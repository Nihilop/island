<script setup lang="ts">
// Fenêtre « Publier une extension » (`publish-extension`, mode développeur) : le dossier tel
// qu'il partira au magasin (version, permissions, médias `store/`, taille) et sa fiche
// (résumé, description, étiquettes, liens, licence, nouveautés), préremplie depuis la
// version en ligne, le manifeste et le README. L'envoi passe par le Rust (paquet + jeton) ;
// la version part en relecture. Publications suspendues par la modération : bouton
// désactivé, avec son message. Arguments : `{ folder }`.
import { computed, onBeforeUnmount, onMounted, reactive, ref } from "vue";
import { emit, listen, type UnlistenFn } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { PhCheckCircle, PhImage, PhStorefront, PhWarning, PhX } from "@phosphor-icons/vue";
import IconButton from "../ui/IconButton.vue";
import StoreAccount from "../tools/wallpaper/StoreAccount.vue";
import { Button } from "@/sdk/ui/button";
import { Input } from "@/sdk/ui/input";
import { Textarea } from "@/sdk/ui/textarea";
import { Badge } from "@/sdk/ui/badge";
import { Spinner } from "@/sdk/ui/spinner";
import { cleanTags, item, marketError, session, storeStatus, watchAccount, type Detail } from "../market/api";
import type { Manifest } from "./runtime";
import { t } from "../notch/i18n";

interface DevInfo {
  /** Champs de la fiche à la racine du manifeste (`summary`, `tags`, liens, licence). */
  manifest: Manifest & { summary?: string; tags?: string[]; homepage?: string; repository?: string; license?: string };
  media: string[];
  readme: string | null;
  files: number;
  size: number;
}
interface MineItem {
  id: string;
  lastVersion: string | null;
  lastStatus: string | null;
  rejectReason: string | null;
}

const win = getCurrentWindow();
const folder = ref<string | null>(null);
const info = ref<DevInfo | null>(null);
const online = ref<Detail | null>(null);
const mine = ref<MineItem | null>(null);
const paused = ref<string | null>(null);
const loading = ref(false);
const sending = ref(false);
const error = ref<string | null>(null);
const sent = ref<{ version: string } | null>(null);

const form = reactive({ summary: "", description: "", tags: "", homepage: "", repository: "", license: "", changelog: "" });
const tags = computed(() => cleanTags(form.tags));

function applyTheme(v: "dark" | "light") {
  const root = document.documentElement;
  root.classList.toggle("light", v === "light");
  root.classList.toggle("dark", v === "dark");
  root.style.colorScheme = v;
}

/** `a` > `b` (« 1.10.0 » > « 1.9.2 »), comme le magasin. */
function newer(a: string, b: string): boolean {
  const parts = (v: string) => {
    const [core, pre] = v.trim().split("-", 2);
    return { n: core.split(".").map((x) => Number.parseInt(x, 10) || 0), pre: pre !== undefined };
  };
  const x = parts(a);
  const y = parts(b);
  for (let i = 0; i < Math.max(x.n.length, y.n.length); i++) {
    const d = (x.n[i] ?? 0) - (y.n[i] ?? 0);
    if (d) return d > 0;
  }
  return y.pre && !x.pre;
}

/** Dernière version déposée (en ligne ou en attente) : la nouvelle doit la dépasser. */
const last = computed(() => {
  const versions = [online.value?.version, mine.value?.lastStatus !== "rejected" ? mine.value?.lastVersion : null].filter((v): v is string => !!v);
  return versions.sort((a, b) => (newer(a, b) ? -1 : 1))[0] ?? null;
});
const tooOld = computed(() => !!info.value && !!last.value && !newer(info.value.manifest.version, last.value));
const canSend = computed(() => !!info.value && !!session.account && !paused.value && !tooOld.value && !sending.value);

async function open(args: { folder?: string } | null) {
  if (!args?.folder) return;
  folder.value = args.folder;
  Object.assign(form, { summary: "", description: "", tags: "", homepage: "", repository: "", license: "", changelog: "" });
  info.value = null;
  online.value = null;
  mine.value = null;
  sent.value = null;
  error.value = null;
  loading.value = true;
  try {
    const [i, status] = await Promise.all([invoke<DevInfo>("market_dev_info", { folder: args.folder }), storeStatus().catch(() => null)]);
    info.value = i;
    paused.value = status && !status.publishing.open ? status.publishing.message || t("market.publish.pausedDefault") : null;
    const id = i.manifest.id;
    const [o, list] = await Promise.all([item(id).catch(() => null), session.account ? invoke<MineItem[]>("market_mine").catch(() => []) : Promise.resolve([])]);
    online.value = o;
    mine.value = list.find((x) => x.id === id) ?? null;
    // Préremplissage : la fiche en ligne, sinon le manifeste et le README.
    const s = i.manifest;
    form.summary = o?.summary || s.summary || i.manifest.description || "";
    form.description = o?.description || i.readme || "";
    form.tags = (o?.tags?.length ? o.tags : (s.tags ?? [])).join(", ");
    form.homepage = o?.homepage || s.homepage || "";
    form.repository = o?.repository || s.repository || "";
    form.license = o?.license || s.license || "";
  } catch (e) {
    error.value = marketError(e);
  } finally {
    loading.value = false;
  }
}

async function send() {
  if (!folder.value || !canSend.value) return;
  sending.value = true;
  error.value = null;
  try {
    const r = await invoke<{ version: string }>("market_publish_extension", {
      folder: folder.value,
      listing: { ...form, tags: tags.value },
    });
    sent.value = r;
  } catch (e) {
    const code = String(e);
    error.value = code.startsWith("fileType:") ? t("ext.publish.fileType", { f: code.slice(9) }) : marketError(e);
    if (code === "publishingPaused") paused.value = t("market.publish.pausedDefault");
  } finally {
    sending.value = false;
  }
}

const size = (b: number) => (b < 1 << 20 ? `${Math.max(1, Math.round(b / 1024))} Ko` : `${(b / (1 << 20)).toFixed(1)} Mo`);
const close = () => win.close();

const unlisten: UnlistenFn[] = [];
onMounted(async () => {
  watchAccount();
  unlisten.push(await listen<"dark" | "light">("theme://changed", (e) => applyTheme(e.payload)));
  void emit("theme://get");
  unlisten.push(await listen<{ folder?: string }>("window://args", (e) => void open(e.payload)));
  // Connexion faite dans cette fenêtre : on recharge l'état au magasin.
  unlisten.push(await listen("market://account", () => folder.value && void open({ folder: folder.value })));
  await open(await invoke<{ folder?: string } | null>("window_args").catch(() => null));
});
onBeforeUnmount(() => unlisten.forEach((u) => u()));
</script>

<template>
  <div class="flex h-screen flex-col overflow-hidden bg-elevated text-label">
    <header class="flex h-10 shrink-0 items-center gap-2 border-b border-separator pr-1 pl-4" data-tauri-drag-region>
      <PhStorefront :size="14" class="text-label-3" />
      <span class="type-footnote font-medium" data-tauri-drag-region>{{ t("ext.publishWin.title") }}</span>
      <div class="flex-1 self-stretch" data-tauri-drag-region />
      <IconButton :icon="PhX" :label="t('common.close')" class="hover:bg-sys-red! hover:text-white!" @click="close" />
    </header>

    <main class="min-h-0 flex-1 overflow-y-auto px-6 py-5">
      <div v-if="loading" class="flex h-full items-center justify-center"><Spinner /></div>

      <div v-else-if="sent" class="flex h-full flex-col items-center justify-center gap-3 text-center">
        <PhCheckCircle :size="40" weight="fill" class="text-sys-green" />
        <div class="type-title font-semibold">{{ t("ext.publishWin.sent", { v: sent.version }) }}</div>
        <p class="type-footnote max-w-[380px] text-label-3">{{ t("ext.publishWin.sentHelp") }}</p>
      </div>

      <template v-else-if="info">
        <!-- Ce qui part -->
        <div class="flex items-start gap-3">
          <div class="min-w-0 flex-1">
            <div class="type-title font-semibold">{{ info.manifest.name }} <span class="text-label-3">v{{ info.manifest.version }}</span></div>
            <div class="type-caption text-label-4">{{ info.manifest.id }} · {{ t("ext.publishWin.files", { n: info.files }) }} · {{ size(info.size) }}</div>
          </div>
          <Badge v-if="online" variant="secondary">{{ t("ext.publishWin.online", { v: online.version ?? "?" }) }}</Badge>
          <Badge v-else variant="secondary" class="text-sys-link">{{ t("ext.publishWin.new") }}</Badge>
        </div>
        <p v-if="mine?.lastStatus === 'pending'" class="type-footnote mt-3 rounded-[10px] bg-fill-1 px-3 py-2 text-label-2">
          {{ t("ext.publishWin.pendingNote", { v: mine.lastVersion ?? "" }) }}
        </p>
        <p v-if="mine?.lastStatus === 'rejected' && mine.rejectReason !== 'superseded'" class="type-footnote mt-3 rounded-[10px] bg-sys-red/10 px-3 py-2 text-sys-red">
          {{ t("ext.publish.rejected", { v: mine.lastVersion ?? "", why: mine.rejectReason ?? "" }) }}
        </p>
        <p v-if="tooOld" class="type-footnote mt-3 flex gap-1.5 rounded-[10px] bg-sys-orange/10 px-3 py-2 text-sys-orange">
          <PhWarning :size="14" weight="bold" class="mt-0.5 shrink-0" />{{ t("ext.publishWin.bump", { v: last ?? "" }) }}
        </p>
        <p v-if="paused" class="type-footnote mt-3 flex gap-1.5 rounded-[10px] bg-sys-orange/10 px-3 py-2 text-sys-orange">
          <PhWarning :size="14" weight="bold" class="mt-0.5 shrink-0" />{{ t("market.publish.paused") }} {{ paused }}
        </p>

        <section class="mt-4 grid grid-cols-2 gap-3">
          <div>
            <h3 class="type-caption mb-1 font-semibold text-label-2">{{ t("ext.permissions") }}</h3>
            <div class="flex flex-wrap gap-1">
              <Badge v-for="p in info.manifest.permissions" :key="p" variant="secondary">{{ p }}</Badge>
              <span v-if="!info.manifest.permissions.length" class="type-caption text-label-3">{{ t("ext.consent.noPerms") }}</span>
            </div>
          </div>
          <div>
            <h3 class="type-caption mb-1 font-semibold text-label-2">{{ t("ext.publishWin.media") }}</h3>
            <ul v-if="info.media.length" class="type-caption flex flex-col text-label-2">
              <li v-for="m in info.media" :key="m" class="flex items-center gap-1"><PhImage :size="11" />{{ m }}</li>
            </ul>
            <p v-else class="type-caption text-label-3">{{ t("ext.publishWin.noMedia") }}</p>
          </div>
        </section>

        <!-- Fiche -->
        <section class="mt-5 flex flex-col gap-3">
          <label class="flex flex-col gap-1">
            <span class="type-caption font-medium text-label-2">{{ t("market.publish.summary") }}</span>
            <Input v-model="form.summary" maxlength="200" :placeholder="t('market.publish.summaryHint')" />
          </label>
          <label class="flex flex-col gap-1">
            <span class="type-caption font-medium text-label-2">{{ t("ext.publishWin.description") }}</span>
            <Textarea v-model="form.description" rows="6" maxlength="5000" :placeholder="t('ext.publishWin.descriptionHint')" />
          </label>
          <label class="flex flex-col gap-1">
            <span class="type-caption font-medium text-label-2">{{ t("market.publish.tags") }}</span>
            <Input v-model="form.tags" :placeholder="t('ext.publishWin.tagsHint')" />
            <span v-if="tags.length" class="flex flex-wrap gap-1">
              <span v-for="tg in tags" :key="tg" class="type-caption rounded-full bg-fill-2 px-1.5 text-label-2">{{ tg }}</span>
            </span>
          </label>
          <div class="grid grid-cols-2 gap-3">
            <label class="flex flex-col gap-1">
              <span class="type-caption font-medium text-label-2">{{ t("market.homepage") }}</span>
              <Input v-model="form.homepage" placeholder="https://" />
            </label>
            <label class="flex flex-col gap-1">
              <span class="type-caption font-medium text-label-2">{{ t("market.repository") }}</span>
              <Input v-model="form.repository" placeholder="https://github.com/…" />
            </label>
          </div>
          <label class="flex flex-col gap-1">
            <span class="type-caption font-medium text-label-2">{{ t("ext.publishWin.license") }}</span>
            <Input v-model="form.license" maxlength="60" placeholder="MIT" />
          </label>
          <label v-if="online || mine" class="flex flex-col gap-1">
            <span class="type-caption font-medium text-label-2">{{ t("market.publish.changelog") }}</span>
            <Textarea v-model="form.changelog" rows="3" maxlength="2000" :placeholder="t('market.publish.changelogHint')" />
          </label>
          <p class="type-caption text-label-4">{{ t("ext.publishWin.notice") }}</p>
        </section>

        <section v-if="!session.account" class="mt-4 rounded-[10px] bg-fill-1 px-3 py-2.5">
          <p class="type-caption mb-1.5 text-label-3">{{ t("market.publish.signInFirst") }}</p>
          <StoreAccount />
        </section>
      </template>

      <p v-if="error" class="type-footnote mt-4 rounded-[10px] bg-sys-red/10 px-3 py-2 text-sys-red">{{ error }}</p>
    </main>

    <footer class="flex shrink-0 justify-end gap-2 border-t border-separator px-4 py-3">
      <Button v-if="sent" variant="default" @click="close">{{ t("common.close") }}</Button>
      <template v-else>
        <Button :disabled="sending" @click="close">{{ t("ext.consent.cancel") }}</Button>
        <Button v-if="info" variant="default" :disabled="!canSend" @click="send">
          <Spinner v-if="sending" /><PhStorefront v-else weight="fill" />{{ sending ? t("market.publish.sending") : t("ext.publishWin.send") }}
        </Button>
      </template>
    </footer>
  </div>
</template>
