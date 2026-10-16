<script setup lang="ts">
// Réglages › Extensions : fiche de consentement (paquet en attente), extensions installées
// (activation, désinstallation, réglages générés depuis le manifeste), mode développeur.
import { computed, onMounted, reactive, ref } from "vue";
import {
  PhArrowClockwise,
  PhCheckCircle,
  PhFolderPlus,
  PhPuzzlePiece,
  PhSealWarning,
  PhShieldCheck,
  PhTrash,
  PhWarningOctagon,
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
import {
  addDevFolder,
  cancelInstall,
  confirmInstall,
  pending,
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

// --- Fiche de consentement ---------------------------------------------------------------

const info = computed(() => pending.info);
/** Paquet non officiel : installable seulement en mode développeur, après avertissement. */
const untrusted = computed(() => info.value && info.value.signature !== "official");
const canInstall = computed(() => !!info.value && info.value.signature !== "invalid" && (!untrusted.value || settings.extensions.devMode));
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
const errorText = (e: string) => {
  const key = `ext.err.${e.split(":")[0]}`;
  const s = t(key);
  return s === key ? e : s;
};
</script>

<template>
  <div>
    <!-- Fiche de consentement -->
    <Group v-if="pending.path" :title="info?.update ? t('ext.consent.update') : t('ext.consent.title')">
      <div v-if="pending.busy && !info" class="type-footnote px-3 py-3 text-label-3">{{ t("ext.consent.reading") }}</div>
      <template v-else-if="info">
        <Row :label="`${info.manifest.name} ${info.manifest.version}`" :help="info.manifest.author ?? info.manifest.id">
          <Badge v-if="info.signature === 'official'" variant="secondary" class="gap-1 text-sys-green"><PhShieldCheck weight="fill" />{{ t("ext.sig.official") }}</Badge>
          <Badge v-else-if="info.signature === 'invalid'" variant="secondary" class="gap-1 text-sys-red"><PhWarningOctagon weight="fill" />{{ t("ext.sig.invalid") }}</Badge>
          <Badge v-else variant="secondary" class="gap-1 text-sys-orange"><PhSealWarning weight="fill" />{{ t(`ext.sig.${info.signature}`) }}</Badge>
        </Row>
        <div v-if="info.manifest.description" class="type-footnote border-t border-separator px-3 py-2 text-label-2">{{ info.manifest.description }}</div>
        <Row v-if="info.update" :label="t('ext.consent.fromVersion', { v: info.update.fromVersion })">
          <span class="type-caption text-label-3">{{ info.update.newPermissions.length ? t("ext.consent.newPerms") : t("ext.consent.noNewPerms") }}</span>
        </Row>
        <Row :label="t('ext.permissions')" :help="info.manifest.permissions.length ? undefined : t('ext.consent.noPerms')">
          <div class="flex max-w-[300px] flex-wrap justify-end gap-1">
            <Badge
              v-for="p in info.manifest.permissions"
              :key="p"
              variant="secondary"
              :class="info.update?.newPermissions.includes(p) ? 'text-sys-orange' : ''"
            >{{ permLabel(p) }}</Badge>
          </div>
        </Row>
        <Row v-if="contributions.length" :label="t('ext.consent.adds')">
          <div class="type-caption flex max-w-[300px] flex-col items-end text-label-2">
            <span v-for="c in contributions" :key="c">{{ c }}</span>
          </div>
        </Row>
        <div v-if="untrusted && info.signature !== 'invalid'" class="type-footnote border-t border-separator px-3 py-2 text-sys-orange">
          {{ settings.extensions.devMode ? t("ext.consent.untrustedDev") : t("ext.consent.untrustedOff") }}
        </div>
      </template>
      <div v-if="pending.error" class="type-footnote border-t border-separator px-3 py-2 text-sys-red">{{ errorText(pending.error) }}</div>
      <div class="flex justify-end gap-2 border-t border-separator px-3 py-2">
        <Button :disabled="pending.busy && !!info" @click="cancelInstall"><PhX weight="bold" />{{ t("ext.consent.cancel") }}</Button>
        <Button v-if="info" variant="default" :disabled="!canInstall" :loading="pending.busy" @click="confirmInstall(!!untrusted)">
          <PhCheckCircle weight="fill" />{{ info.update ? t("ext.consent.doUpdate") : t("ext.consent.install") }}
        </Button>
      </div>
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

    <!-- Réglages de chaque extension active -->
    <template v-for="x in all" :key="`s-${x.manifest.id}`">
      <Group v-if="active(x.manifest.id) && (x.manifest.contributes.settings.length || x.manifest.permissions.length)" :title="x.manifest.name">
        <Row v-if="x.manifest.permissions.length" :label="t('ext.permissions')">
          <div class="flex max-w-[300px] flex-wrap justify-end gap-1">
            <Badge v-for="p in x.manifest.permissions" :key="p" variant="secondary">{{ permLabel(p) }}</Badge>
          </div>
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
        <Row v-for="dir in settings.extensions.devFolders" :key="dir" :label="dir.split(/[\\/]/).pop() ?? dir" :help="dir">
          <Button size="icon-sm" variant="ghost" :title="t('ext.devRemove')" @click="removeDevFolder(dir)"><PhX weight="bold" /></Button>
        </Row>
        <Row :label="t('ext.devAdd')" :help="devError ? errorText(devError) : t('ext.devAddHelp')">
          <Button @click="onAddDev"><PhFolderPlus weight="bold" />{{ t("ext.devChoose") }}</Button>
        </Row>
      </template>
    </Group>
  </div>
</template>
