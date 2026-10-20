<script setup lang="ts">
// Réglages › Extensions : compte du magasin, extensions installées (activation,
// désinstallation, réglages générés depuis le manifeste), magasin (ExtensionStore), mode
// développeur (dossiers ; « Empaqueter » un .island, « Publier » au magasin pour relecture).
// L'installation elle-même se fait dans sa fenêtre (InstallWindow.vue).
import { onMounted, reactive, ref, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import {
  PhArrowClockwise,
  PhFolderPlus,
  PhPuzzlePiece,
  PhStorefront,
  PhPackage,
  PhTrash,
  PhX,
} from "@phosphor-icons/vue";
import Group from "../ui/Group.vue";
import Row from "../ui/Row.vue";
import TextField from "../ui/TextField.vue";
import Select from "../ui/Select.vue";
import Empty from "../ui/Empty.vue";
import { Switch } from "@/sdk/ui/switch";
import { Button } from "@/sdk/ui/button";
import { Badge } from "@/sdk/ui/badge";
import { all, extUrl, installed, type Manifest, type SettingDecl } from "./runtime";
import { readSetting, writeSetting } from "./settings";
import { commandShortcut, setCommandShortcut, shortcutPrefs, shortcutStatus } from "./slots";
import ShortcutField from "../ui/ShortcutField.vue";
import ExtensionStore from "./ExtensionStore.vue";
import StoreAccount from "../tools/wallpaper/StoreAccount.vue";
import { marketError, session, storeStatus, watchAccount } from "../market/api";
import { listen } from "@tauri-apps/api/event";
import {
  addDevFolder,
  pickPackage,
  reloadExtension,
  removeDevFolder,
  setEnabled,
  uninstall,
} from "./install";
import { settings } from "../notch/settings";
import { t } from "../notch/i18n";

// --- Réglages des extensions actives -----------------------------------------------------

/** Valeurs affichées, par `extension|clé` (les secrets ne sont jamais réaffichés). */
const values = reactive(new Map<string, unknown>());
const secretSet = reactive(new Set<string>());
const secretDraft = reactive(new Map<string, string>());
const saving = ref<string | null>(null);

const k = (ext: Manifest, key: string) => `${ext.id}|${key}`;
const active = (id: string) => installed.value.find((m) => m.id === id) ?? null;

onMounted(async () => {
  for (const ext of installed.value) {
    for (const d of ext.contributes.settings) {
      const v = await readSetting(ext, d.key).catch(() => null);
      if (d.type === "secret") {
        if (v) secretSet.add(k(ext, d.key));
      } else values.set(k(ext, d.key), d.type === "list" && Array.isArray(v) ? v.join("\n") : v);
    }
  }
});

async function save(ext: Manifest, d: SettingDecl, value: unknown) {
  const id = k(ext, d.key);
  saving.value = id;
  try {
    const v =
      d.type === "list"
        ? String(value ?? "")
            .split(/[\n,]/)
            .map((s) => s.trim())
            .filter(Boolean)
        : d.type === "number"
          ? Number(value)
          : value;
    await writeSetting(ext, d.key, v);
    if (d.type === "secret") {
      if (value) secretSet.add(id);
      else secretSet.delete(id);
      secretDraft.delete(id);
    } else values.set(id, value);
  } finally {
    saving.value = null;
  }
}

/** Libellé lisible d'une permission. */
function permLabel(p: string): string {
  if (p.startsWith("network:")) return t("ext.perm.network", { host: p.slice(8) });
  if (p.startsWith("images:")) return t("ext.perm.images", { host: p.slice(7) });
  const key = `ext.perm.${p}`;
  const s = t(key);
  return s === key ? p : s;
}

// --- Gestion -------------------------------------------------------------------------------

/** Désinstallation en deux temps (premier clic = confirmer). */
const confirmUninstall = ref<string | null>(null);
const busy = ref<string | null>(null);
const devError = ref<string | null>(null);

async function onUninstall(id: string) {
  if (confirmUninstall.value !== id) {
    confirmUninstall.value = id;
    window.setTimeout(() => confirmUninstall.value === id && (confirmUninstall.value = null), 3000);
    return;
  }
  busy.value = id;
  try {
    await uninstall(id);
  } finally {
    busy.value = null;
    confirmUninstall.value = null;
  }
}

async function onToggle(id: string, on: boolean) {
  busy.value = id;
  try {
    await setEnabled(id, on);
  } finally {
    busy.value = null;
  }
}

async function onReload(id: string) {
  busy.value = id;
  try {
    await reloadExtension(id);
  } finally {
    busy.value = null;
  }
}

async function onAddDev() {
  devError.value = await addDevFolder();
}

// --- Mode développeur : publier au magasin ----------------------------------------------

interface MineItem {
  id: string;
  lastVersion: string | null;
  lastStatus: string | null;
  rejectReason: string | null;
}
/** Dossier → id du manifeste (pour retrouver l'état au magasin). */
const devIds = reactive(new Map<string, string>());
const mine = ref<Map<string, MineItem>>(new Map());
const publishing = ref<string | null>(null);
const publishMsg = reactive(new Map<string, { ok: boolean; text: string }>());

async function loadDevIds() {
  for (const dir of settings.extensions.devFolders) {
    if (devIds.has(dir)) continue;
    const m = await invoke<Manifest>("ext2_dev_inspect", { folder: dir }).catch(() => null);
    if (m) devIds.set(dir, m.id);
  }
}
async function loadMine() {
  if (!session.account) return;
  const list = await invoke<MineItem[]>("market_mine").catch(() => []);
  mine.value = new Map(list.map((x) => [x.id, x]));
}
watch(
  () => [settings.extensions.devMode, settings.extensions.devFolders.length, session.account?.id] as const,
  ([dev]) => {
    if (!dev) return;
    watchAccount();
    void loadDevIds();
    void loadMine();
    void loadPaused();
  },
  { immediate: true },
);

/** État au magasin d'un dossier en développement. */
function storeState(dir: string): string | null {
  const x = mine.value.get(devIds.get(dir) ?? "");
  if (!x?.lastVersion) return null;
  if (x.lastStatus === "pending") return t("ext.publish.pending", { v: x.lastVersion });
  if (x.lastStatus === "approved") return t("ext.publish.approved", { v: x.lastVersion });
  if (x.lastStatus === "rejected") return x.rejectReason === "superseded" ? null : t("ext.publish.rejected", { v: x.lastVersion, why: x.rejectReason ?? "" });
  return null;
}

const packing = ref<string | null>(null);
/** Empaquette le dossier dans un `.island` choisi (comme `island pack`, sans signature). */
async function packDev(dir: string) {
  if (packing.value) return;
  const id = devIds.get(dir) ?? dir.split(/[\\/]/).pop() ?? "extension";
  const { save } = await import("@tauri-apps/plugin-dialog");
  const dest = await save({ defaultPath: `${id}.island`, filters: [{ name: "Island", extensions: ["island"] }] }).catch(() => null);
  if (!dest) return;
  packing.value = dir;
  publishMsg.delete(dir);
  try {
    const r = await invoke<{ files: number; size: number }>("market_pack_extension", { folder: dir, path: dest });
    publishMsg.set(dir, { ok: true, text: t("ext.pack.done", { n: r.files, kb: (r.size / 1024).toFixed(1) }) });
  } catch (e) {
    const code = String(e);
    publishMsg.set(dir, { ok: false, text: code.startsWith("fileType:") ? t("ext.publish.fileType", { f: code.slice(9) }) : marketError(e) });
  } finally {
    packing.value = null;
  }
}

/** « Publier » : la fenêtre de publication (fiche, médias, état au magasin). */
async function publishDev(dir: string) {
  if (publishing.value) return;
  publishing.value = dir;
  publishMsg.delete(dir);
  try {
    await invoke("window_open", { kind: "publish", key: "extension", title: t("ext.publishWin.title"), size: [640, 760], minSize: [480, 520], args: { folder: dir } });
  } finally {
    publishing.value = null;
  }
}

/** Publications suspendues par la modération : « Publier » désactivé, avec son message. */
const paused = ref<string | null>(null);
async function loadPaused() {
  const s = await storeStatus().catch(() => null);
  paused.value = s && !s.publishing.open ? s.publishing.message || t("market.publish.pausedDefault") : null;
}
void listen("market://published", () => {
  void loadMine();
  void loadPaused();
});

const errorText = (e: string) => {
  const key = `ext.err.${e.split(":")[0]}`;
  const s = t(key);
  return s === key ? e : s;
};
</script>

<template>
  <div>
    <!-- Compte du magasin : installer, publier depuis Island -->
    <Group :title="t('ext.account.title')" :note="session.account ? undefined : t('ext.account.note')">
      <div class="px-3 py-2.5"><StoreAccount /></div>
    </Group>

    <!-- Extensions installées -->
    <Group :title="t('ext.installed')" section="extensions">
      <Row :label="t('ext.install')" :help="t('ext.installHelp')">
        <Button @click="pickPackage">{{ t("ext.installAction") }}</Button>
      </Row>
      <div v-if="!all.length" class="border-t border-separator">
        <Empty :icon="PhPuzzlePiece" :title="t('ext.none')" :text="t('ext.noneHelp')" />
      </div>
      <Row v-for="x in all" :key="x.manifest.id" :label="`${x.manifest.name} ${x.manifest.version}`" :help="x.manifest.description ?? x.manifest.id">
        <span
          v-if="x.manifest.icon"
          class="size-4 bg-label-3 [mask-position:center] [mask-repeat:no-repeat] [mask-size:contain]"
          :style="{ maskImage: `url('${extUrl(x.manifest.id, x.manifest.icon)}')` }"
        />
        <Badge v-if="x.dev" variant="secondary">{{ t("ext.dev") }}</Badge>
        <Button v-if="x.dev" size="icon-sm" variant="ghost" :title="t('ext.reload')" :loading="busy === x.manifest.id" @click="onReload(x.manifest.id)">
          <PhArrowClockwise weight="bold" />
        </Button>
        <Button
          v-else
          size="sm"
          :variant="confirmUninstall === x.manifest.id ? 'destructive' : 'ghost'"
          :loading="busy === x.manifest.id"
          :title="t('ext.uninstall')"
          @click="onUninstall(x.manifest.id)"
        >
          <PhTrash weight="bold" /><span v-if="confirmUninstall === x.manifest.id">{{ t("ext.uninstallConfirm") }}</span>
        </Button>
        <Switch
          :model-value="!settings.extensions.disabled.includes(x.manifest.id)"
          :disabled="busy === x.manifest.id"
          @update:model-value="onToggle(x.manifest.id, $event)"
        />
      </Row>
    </Group>

    <ExtensionStore />

    <!-- Réglages de chaque extension active -->
    <template v-for="x in all" :key="`s-${x.manifest.id}`">
      <Group
        v-if="active(x.manifest.id) && (x.manifest.contributes.settings.length || x.manifest.permissions.length || x.manifest.contributes.commands.length)"
        :title="x.manifest.name"
      >
        <Row v-if="x.manifest.permissions.length" :label="t('ext.permissions')">
          <div class="flex max-w-[300px] flex-wrap justify-end gap-1">
            <Badge v-for="p in x.manifest.permissions" :key="p" variant="secondary" :class="p === 'capture' || p === 'activity' ? 'text-sys-red' : ''">{{ permLabel(p) }}</Badge>
          </div>
        </Row>
        <!-- Raccourcis des commandes (modifiables ; défaut = manifeste). -->
        <Row
          v-for="c in x.manifest.contributes.commands"
          :key="`cmd-${c.id}`"
          :label="c.title"
          :help="shortcutStatus.get(`${x.manifest.id}:${c.id}`) === 'taken' ? t('ext.shortcut.taken') : c.shortcut ? t('ext.shortcut.default', { s: c.shortcut }) : undefined"
        >
          <Button
            v-if="`${x.manifest.id}:${c.id}` in shortcutPrefs.map"
            variant="ghost"
            size="sm"
            class="text-label-3"
            @click="setCommandShortcut(x.manifest, c.id, null)"
          >{{ t("ext.shortcut.reset") }}</Button>
          <ShortcutField :model-value="commandShortcut(x.manifest, c)" @update:model-value="setCommandShortcut(x.manifest, c.id, $event)" />
        </Row>
        <Row v-for="d in x.manifest.contributes.settings" :key="d.key" :label="d.label" :help="d.help">
          <Switch v-if="d.type === 'switch'" :model-value="!!values.get(k(x.manifest, d.key))" @update:model-value="save(x.manifest, d, $event)" />
          <Select
            v-else-if="d.type === 'select'"
            :model-value="(values.get(k(x.manifest, d.key)) as string) ?? ''"
            :options="d.options ?? []"
            @update:model-value="save(x.manifest, d, $event)"
          />
          <template v-else-if="d.type === 'secret'">
            <span v-if="secretSet.has(k(x.manifest, d.key)) && !secretDraft.has(k(x.manifest, d.key))" class="type-caption text-label-3">{{ t("ext.secretSaved") }}</span>
            <div v-else class="w-[200px]">
              <TextField
                :model-value="secretDraft.get(k(x.manifest, d.key)) ?? ''"
                secret
                @update:model-value="secretDraft.set(k(x.manifest, d.key), $event)"
                @enter="save(x.manifest, d, secretDraft.get(k(x.manifest, d.key)))"
              />
            </div>
            <Button
              v-if="secretSet.has(k(x.manifest, d.key)) && !secretDraft.has(k(x.manifest, d.key))"
              :loading="saving === k(x.manifest, d.key)"
              @click="save(x.manifest, d, '')"
            >{{ t("ext.clear") }}</Button>
            <Button
              v-else
              variant="default"
              :disabled="!secretDraft.get(k(x.manifest, d.key))"
              :loading="saving === k(x.manifest, d.key)"
              @click="save(x.manifest, d, secretDraft.get(k(x.manifest, d.key)))"
            >{{ t("ext.save") }}</Button>
          </template>
          <div v-else class="w-[220px]">
            <TextField
              :model-value="String(values.get(k(x.manifest, d.key)) ?? '')"
              :type="d.type === 'number' ? 'number' : 'text'"
              :placeholder="d.type === 'list' ? t('ext.listHelp') : ''"
              @update:model-value="values.set(k(x.manifest, d.key), $event)"
              @enter="save(x.manifest, d, values.get(k(x.manifest, d.key)))"
              @blur="save(x.manifest, d, values.get(k(x.manifest, d.key)))"
            />
          </div>
        </Row>
      </Group>
    </template>

    <!-- Mode développeur -->
    <Group :title="t('ext.devMode')" :note="t('ext.devModeNote')">
      <Row :label="t('ext.devMode')" :help="t('ext.devModeHelp')">
        <Switch v-model="settings.extensions.devMode" />
      </Row>
      <template v-if="settings.extensions.devMode">
        <Row
          v-for="dir in settings.extensions.devFolders"
          :key="dir"
          :label="dir.split(/[\\/]/).pop() ?? dir"
          :help="publishMsg.get(dir)?.text ?? storeState(dir) ?? dir"
          :class="publishMsg.get(dir)?.ok === false ? '[&_.type-caption]:text-sys-red' : ''"
        >
          <Button size="sm" :loading="packing === dir" :disabled="!!packing" :title="t('ext.pack.help')" @click="packDev(dir)">
            <PhPackage weight="bold" />{{ t("ext.pack.button") }}
          </Button>
          <Button
            size="sm"
            :loading="publishing === dir"
            :disabled="!!publishing || !session.account || !!paused"
            :title="paused ? `${t('market.publish.paused')} ${paused}` : session.account ? t('ext.publish.help') : t('ext.publish.signIn')"
            @click="publishDev(dir)"
          >
            <PhStorefront weight="bold" />{{ t("ext.publish.button") }}
          </Button>
          <Button size="icon-sm" variant="ghost" :title="t('ext.devRemove')" @click="removeDevFolder(dir)"><PhX weight="bold" /></Button>
        </Row>

        <Row :label="t('ext.devAdd')" :help="devError ? errorText(devError) : t('ext.devAddHelp')">
          <Button @click="onAddDev"><PhFolderPlus weight="bold" />{{ t("ext.devChoose") }}</Button>
        </Row>
      </template>
    </Group>
  </div>
</template>
