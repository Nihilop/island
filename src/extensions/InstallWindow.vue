<script setup lang="ts">
// Fenêtre d'installation d'une extension (`install-extension`, fenêtre détachée) : la fiche
// de consentement en grand — nom, éditeur, état de la signature, description, permissions
// en clair (nouvelles en orange, sensibles en rouge), ce que l'extension ajoute. Ouverte
// pour tout paquet : choisi, glissé sur l'île, double-cliqué ou téléchargé du magasin.
// Arguments : `{ path, devMode }`. Après l'installation, `ext2://installed` prévient
// l'overlay, qui active et charge l'extension.
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { emit, listen, type UnlistenFn } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { PhCheckCircle, PhPuzzlePiece, PhSealWarning, PhShieldCheck, PhStorefront, PhWarningOctagon, PhX } from "@phosphor-icons/vue";
import IconButton from "../ui/IconButton.vue";
import { Button } from "@/sdk/ui/button";
import { Badge } from "@/sdk/ui/badge";
import { Spinner } from "@/sdk/ui/spinner";
import type { Inspect } from "./install";
import type { Manifest } from "./runtime";
import { t } from "../notch/i18n";

const win = getCurrentWindow();
const path = ref<string | null>(null);
const devMode = ref(false);
const info = ref<Inspect | null>(null);
const error = ref<string | null>(null);
const loading = ref(false);
const installing = ref(false);
const done = ref<Manifest | null>(null);

function applyTheme(v: "dark" | "light") {
  const root = document.documentElement;
  root.classList.toggle("light", v === "light");
  root.classList.toggle("dark", v === "dark");
  root.style.colorScheme = v;
}

async function open(args: { path?: string; devMode?: boolean } | null) {
  if (!args?.path) return;
  path.value = args.path;
  devMode.value = !!args.devMode;
  info.value = null;
  error.value = null;
  done.value = null;
  loading.value = true;
  try {
    info.value = await invoke<Inspect>("ext2_inspect", { path: args.path });
  } catch (e) {
    error.value = String(e);
  } finally {
    loading.value = false;
  }
}

/** Ni officielle ni vérifiée par le magasin : seulement en mode développeur. */
const untrusted = computed(() => !!info.value && info.value.signature !== "official" && info.value.signature !== "store");
const canInstall = computed(() => !!info.value && info.value.signature !== "invalid" && (!untrusted.value || devMode.value));

const contributions = computed(() => {
  const c = info.value?.manifest.contributes;
  if (!c) return [];
  return [
    ...c.tools.map((x) => t("ext.adds.tool", { name: x.title })),
    ...(c.live.length ? [t("ext.adds.live")] : []),
    ...c.launcher.map((x) => (x.prefix ? t("ext.adds.launcherPrefix", { prefix: x.prefix }) : t("ext.adds.launcher"))),
    ...c.commands.map((x) => t("ext.adds.command", { name: x.title })),
    ...(c.settings.length ? [t("ext.adds.settings")] : []),
  ];
});

function permLabel(p: string): string {
  if (p.startsWith("network:")) return t("ext.perm.network", { host: p.slice(8) });
  if (p.startsWith("images:")) return t("ext.perm.images", { host: p.slice(7) });
  const key = `ext.perm.${p}`;
  const s = t(key);
  return s === key ? p : s;
}
const sensitive = (p: string) => p === "capture" || p === "activity" || p === "terminal";
const errorText = (e: string) => {
  const key = `ext.err.${e.split(":")[0]}`;
  const s = t(key);
  return s === key ? e : s;
};
const size = (b: number) => (b < 1 << 20 ? `${Math.max(1, Math.round(b / 1024))} Ko` : `${(b / (1 << 20)).toFixed(1)} Mo`);

async function install() {
  if (!path.value || !canInstall.value || installing.value) return;
  installing.value = true;
  error.value = null;
  try {
    const m = await invoke<Manifest>("ext2_install", { path: path.value, allowUntrusted: untrusted.value });
    await emit("ext2://installed", m.id);
    done.value = m;
  } catch (e) {
    error.value = String(e);
  } finally {
    installing.value = false;
  }
}

const close = () => win.close();

const unlisten: UnlistenFn[] = [];
onMounted(async () => {
  unlisten.push(await listen<"dark" | "light">("theme://changed", (e) => applyTheme(e.payload)));
  void emit("theme://get");
  unlisten.push(await listen<{ path?: string; devMode?: boolean }>("window://args", (e) => void open(e.payload)));
  await open(await invoke<{ path?: string; devMode?: boolean } | null>("window_args").catch(() => null));
});
onBeforeUnmount(() => unlisten.forEach((u) => u()));
</script>

<template>
  <div class="flex h-screen flex-col overflow-hidden bg-elevated text-label">
    <header class="flex h-10 shrink-0 items-center gap-2 border-b border-separator pr-1 pl-4" data-tauri-drag-region>
      <PhPuzzlePiece :size="14" class="text-label-3" />
      <span class="type-footnote font-medium" data-tauri-drag-region>{{ info?.update ? t("ext.consent.update") : t("ext.consent.title") }}</span>
      <div class="flex-1 self-stretch" data-tauri-drag-region />
      <IconButton :icon="PhX" :label="t('common.close')" class="hover:bg-sys-red! hover:text-white!" @click="close" />
    </header>

    <main class="min-h-0 flex-1 overflow-y-auto px-6 py-5">
      <div v-if="loading" class="flex h-full flex-col items-center justify-center gap-2 text-label-3">
        <Spinner />
        <span class="type-footnote">{{ t("ext.consent.reading") }}</span>
      </div>

      <!-- Installée -->
      <div v-else-if="done" class="flex h-full flex-col items-center justify-center gap-3 text-center">
        <PhCheckCircle :size="40" weight="fill" class="text-sys-green" />
        <div class="type-title font-semibold">{{ t("ext.window.done", { name: done.name }) }}</div>
        <p class="type-footnote max-w-[360px] text-label-3">{{ t("ext.window.doneHelp") }}</p>
      </div>

      <template v-else-if="info">
        <div class="flex items-start gap-3">
          <div class="min-w-0 flex-1">
            <div class="type-title font-semibold">{{ info.manifest.name }}</div>
            <div class="type-footnote text-label-3">
              {{ t("ext.window.version", { v: info.manifest.version }) }}
              <template v-if="info.manifest.author"> · {{ info.manifest.author }}</template>
              · {{ size(info.size) }}
            </div>
            <div class="type-caption mt-0.5 text-label-4">{{ info.manifest.id }}</div>
          </div>
          <Badge v-if="info.signature === 'official'" variant="secondary" class="gap-1 text-sys-green"><PhShieldCheck weight="fill" />{{ t("ext.sig.official") }}</Badge>
          <Badge v-else-if="info.signature === 'store'" variant="secondary" class="gap-1 text-sys-green"><PhStorefront weight="fill" />{{ t("ext.sig.store") }}</Badge>
          <Badge v-else-if="info.signature === 'invalid'" variant="secondary" class="gap-1 text-sys-red"><PhWarningOctagon weight="fill" />{{ t("ext.sig.invalid") }}</Badge>
          <Badge v-else variant="secondary" class="gap-1 text-sys-orange"><PhSealWarning weight="fill" />{{ t(`ext.sig.${info.signature}`) }}</Badge>
        </div>

        <p v-if="info.manifest.description" class="type-body mt-4 text-label-2">{{ info.manifest.description }}</p>

        <div v-if="info.update" class="type-footnote mt-4 rounded-[10px] bg-fill-1 px-3 py-2 text-label-2">
          {{ t("ext.consent.fromVersion", { v: info.update.fromVersion }) }} ·
          {{ info.update.newPermissions.length ? t("ext.consent.newPerms") : t("ext.consent.noNewPerms") }}
        </div>

        <section class="mt-5">
          <h3 class="type-footnote mb-2 font-semibold">{{ t("ext.permissions") }}</h3>
          <p v-if="!info.manifest.permissions.length" class="type-footnote text-label-3">{{ t("ext.consent.noPerms") }}</p>
          <ul v-else class="flex flex-col gap-1.5">
            <li
              v-for="p in info.manifest.permissions"
              :key="p"
              class="type-footnote flex items-center gap-2 rounded-[8px] bg-fill-1 px-3 py-1.5"
              :class="sensitive(p) ? 'text-sys-red' : info.update?.newPermissions.includes(p) ? 'text-sys-orange' : 'text-label-2'"
            >
              <span class="size-1.5 shrink-0 rounded-full bg-current" />
              <span class="flex-1">{{ permLabel(p) }}</span>
              <span v-if="info.update?.newPermissions.includes(p)" class="type-caption font-medium">{{ t("ext.window.newPerm") }}</span>
            </li>
          </ul>
        </section>

        <section v-if="contributions.length" class="mt-5">
          <h3 class="type-footnote mb-2 font-semibold">{{ t("ext.consent.adds") }}</h3>
          <ul class="type-footnote flex flex-col gap-1 text-label-2">
            <li v-for="c in contributions" :key="c">· {{ c }}</li>
          </ul>
        </section>

        <p v-if="untrusted && info.signature !== 'invalid'" class="type-footnote mt-5 rounded-[10px] bg-sys-orange/10 px-3 py-2 text-sys-orange">
          {{ devMode ? t("ext.consent.untrustedDev") : t("ext.consent.untrustedOff") }}
        </p>
      </template>

      <p v-if="error" class="type-footnote mt-4 rounded-[10px] bg-sys-red/10 px-3 py-2 text-sys-red">{{ errorText(error) }}</p>
    </main>

    <footer class="flex shrink-0 justify-end gap-2 border-t border-separator px-4 py-3">
      <template v-if="done">
        <Button variant="default" @click="close">{{ t("common.close") }}</Button>
      </template>
      <template v-else>
        <Button :disabled="installing" @click="close"><PhX weight="bold" />{{ t("ext.consent.cancel") }}</Button>
        <Button v-if="info" variant="default" :disabled="!canInstall || installing" @click="install">
          <Spinner v-if="installing" /><PhCheckCircle v-else weight="fill" />{{ info.update ? t("ext.consent.doUpdate") : t("ext.consent.install") }}
        </Button>
      </template>
    </footer>
  </div>
</template>
