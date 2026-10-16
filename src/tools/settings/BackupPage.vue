<script setup lang="ts">
// Réglages › Sauvegarde : export / import des réglages et des données des outils (un
// fichier JSON, cf. src-tauri/src/backup.rs). Ni extensions, ni secrets (Windows Hello,
// Bitwarden). Les sections verrouillées par Windows Hello (notes, budget) demandent une
// confirmation avant export comme avant import. L'import remplace les sections choisies
// après une copie de sécurité de l'état actuel, puis Island redémarre.
import { computed, onMounted, reactive, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { open as openDialog, save } from "@tauri-apps/plugin-dialog";
import { PhCheckCircle, PhFileArrowDown, PhFileArrowUp, PhFolderOpen, PhWarning } from "@phosphor-icons/vue";
import Group from "../../ui/Group.vue";
import Row from "../../ui/Row.vue";
import { Switch } from "@/sdk/ui/switch";
import { Button } from "@/sdk/ui/button";
import { settings } from "../../notch/settings";
import { helloError, unlockScopes, type HelloScope } from "../../notch/hello";
import { locale } from "../../sdk/i18n";
import { t } from "../../notch/i18n";

interface Summary {
  app: string;
  created: number;
  sections: Record<string, number>;
}

/** Ordre d'affichage (celui du Rust). */
const SECTIONS = ["settings", "notes", "todos", "budget", "markets", "weather", "timer", "shelf", "launcher"];
/** Sections protégées par Windows Hello (si le verrou de l'outil est activé). */
const LOCKS: Partial<Record<string, HelloScope>> = { notes: "notes", budget: "budget" };

const current = ref<Summary | null>(null);
const exportSel = reactive<Record<string, boolean>>(Object.fromEntries(SECTIONS.map((s) => [s, true])));
const busy = ref<"export" | "pick" | "import" | null>(null);
const message = ref<{ ok: boolean; text: string } | null>(null);

onMounted(async () => {
  current.value = await invoke<Summary>("backup_current").catch(() => null);
});

const help = (id: string, n: number | undefined) => {
  if (id === "notes") return t("backup.count.notes", { n: n ?? 0 });
  if (id === "todos") return t("backup.count.todos", { n: n ?? 0 });
  return n ? t(`backup.help.${id}`) : t("backup.empty");
};
const has = (id: string) => (current.value?.sections[id] ?? 0) > 0;
const chosen = (sel: Record<string, boolean>, available: (id: string) => boolean) => SECTIONS.filter((s) => sel[s] && available(s));
const exportList = computed(() => chosen(exportSel, has));

/** Confirme Windows Hello pour les sections verrouillées choisies. */
async function confirmLocks(ids: string[]) {
  const scopes = ids.map((s) => LOCKS[s]).filter((x): x is HelloScope => !!x && settings.hello[x]);
  await unlockScopes(scopes);
}

function fail(e: unknown) {
  const hello = helloError(t, e);
  const code = String(e);
  const key = `backup.err.${code}`;
  message.value = { ok: false, text: hello && code.startsWith("hello") ? hello : t(key) === key ? t("backup.err.generic", { e: code }) : t(key) };
}

async function doExport() {
  if (busy.value || !exportList.value.length) return;
  busy.value = "export";
  message.value = null;
  try {
    await confirmLocks(exportList.value);
    const path = await save({
      defaultPath: `island-sauvegarde-${new Date().toISOString().slice(0, 10)}.json`,
      filters: [{ name: t("backup.fileType"), extensions: ["json"] }],
    });
    if (!path) return;
    const s = await invoke<Summary>("backup_export", { path, sections: exportList.value });
    message.value = { ok: true, text: t("backup.exported", { n: Object.keys(s.sections).length }) };
  } catch (e) {
    if (helloError(t, e) !== null || !String(e).startsWith("hello")) fail(e);
  } finally {
    busy.value = null;
  }
}

// --- Import ---------------------------------------------------------------------------------

const file = ref<{ path: string; summary: Summary } | null>(null);
const importSel = reactive<Record<string, boolean>>({});
const importList = computed(() => (file.value ? chosen(importSel, (s) => s in file.value!.summary.sections) : []));
const confirming = ref(false);

async function pick() {
  if (busy.value) return;
  busy.value = "pick";
  message.value = null;
  confirming.value = false;
  try {
    const path = await openDialog({ multiple: false, directory: false, filters: [{ name: t("backup.fileType"), extensions: ["json"] }] });
    if (!path || Array.isArray(path)) return;
    const summary = await invoke<Summary>("backup_inspect", { path });
    file.value = { path, summary };
    for (const s of SECTIONS) importSel[s] = s in summary.sections;
  } catch (e) {
    fail(e);
  } finally {
    busy.value = null;
  }
}

async function doImport() {
  if (busy.value || !file.value || !importList.value.length) return;
  if (!confirming.value) {
    confirming.value = true;
    window.setTimeout(() => (confirming.value = false), 4000);
    return;
  }
  busy.value = "import";
  message.value = null;
  try {
    await confirmLocks(importList.value);
    // Redémarre l'app en cas de succès : on ne revient pas ici.
    await invoke("backup_import", { path: file.value.path, sections: importList.value });
  } catch (e) {
    if (helloError(t, e) !== null || !String(e).startsWith("hello")) fail(e);
    busy.value = null;
    confirming.value = false;
  }
}

const date = (ms: number) => new Intl.DateTimeFormat(locale.value, { dateStyle: "long", timeStyle: "short" }).format(new Date(ms));
const fileName = computed(() => file.value?.path.split(/[\\/]/).pop() ?? "");
</script>

<template>
  <div>
    <Group :title="t('backup.export')" section="backup" :note="t('backup.scope')">
      <Row v-for="id in SECTIONS" :key="id" :label="t(`backup.section.${id}`)" :help="help(id, current?.sections[id])">
        <Switch v-model="exportSel[id]" :disabled="!has(id)" />
      </Row>
      <div class="flex items-center justify-end gap-2 border-t border-separator px-3 py-2">
        <Button :loading="busy === 'export'" :disabled="!!busy || !exportList.length" @click="doExport">
          <PhFileArrowDown weight="bold" />{{ t("backup.exportBtn") }}
        </Button>
      </div>
    </Group>

    <Group :title="t('backup.import')" section="restore" :note="t('backup.importNote')">
      <Row :label="file ? fileName : t('backup.noFile')" :help="file ? t('backup.fileInfo', { date: date(file.summary.created), v: file.summary.app }) : t('backup.pickHelp')">
        <Button variant="secondary" :loading="busy === 'pick'" :disabled="!!busy" @click="pick"><PhFileArrowUp weight="bold" />{{ t("backup.pick") }}</Button>
      </Row>
      <template v-if="file">
        <Row
          v-for="id in SECTIONS.filter((s) => s in file!.summary.sections)"
          :key="id"
          :label="t(`backup.section.${id}`)"
          :help="help(id, file.summary.sections[id])"
        >
          <Switch v-model="importSel[id]" />
        </Row>
        <div class="flex items-center justify-end gap-2 border-t border-separator px-3 py-2">
          <span v-if="confirming" class="type-caption mr-auto flex items-center gap-1.5 text-sys-orange">
            <PhWarning :size="13" weight="fill" />{{ t("backup.confirm", { n: importList.length }) }}
          </span>
          <Button
            :variant="confirming ? 'destructive' : 'default'"
            :loading="busy === 'import'"
            :disabled="!!busy || !importList.length"
            @click="doImport"
          >
            {{ confirming ? t("backup.importConfirm") : t("backup.importBtn") }}
          </Button>
        </div>
      </template>
    </Group>

    <div
      v-if="message"
      class="type-footnote mt-3 flex items-start gap-2 rounded-md px-3 py-2"
      :class="message.ok ? 'bg-sys-green/12 text-sys-green' : 'bg-sys-red/12 text-sys-red'"
    >
      <component :is="message.ok ? PhCheckCircle : PhWarning" :size="15" weight="fill" class="mt-px shrink-0" />{{ message.text }}
    </div>

    <button
      type="button"
      class="type-caption mt-3 flex items-center gap-1.5 px-3 text-label-3 transition-colors duration-100 hover:text-label-2"
      @click="invoke('backup_open_folder')"
    >
      <PhFolderOpen :size="13" weight="bold" />{{ t("backup.safetyFolder") }}
    </button>
  </div>
</template>
