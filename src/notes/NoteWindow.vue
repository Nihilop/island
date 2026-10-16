<script setup lang="ts">
// Fenêtre détachée d'une note (libellé `note-<id>`, cf. src-tauri/src/detached.rs) : barre
// de titre maison (fenêtre sans décoration), éditeur Markdown, sauvegarde automatique.
// Le thème suit le notch (`theme://changed`) ; basculer ici le demande au notch
// (`theme://set`), seul à écrire les réglages. Toute activité repousse le verrou Hello
// des notes (`hello://touch`) ; au verrouillage, le notch ferme ces fenêtres.
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { emit, listen, type UnlistenFn } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import {
  PhArrowClockwise,
  PhArrowCounterClockwise,
  PhCheck,
  PhWarning,
  PhCornersIn,
  PhCornersOut,
  PhMinus,
  PhMoon,
  PhPushPin,
  PhSun,
  PhX,
} from "@phosphor-icons/vue";
import IconButton from "../ui/IconButton.vue";
import NoteEditor from "./editor/NoteEditor.vue";
import { t } from "../notch/i18n";

interface Meta {
  id: string;
  path: string;
  created: number;
  updated: number;
  pinned: boolean;
  trashed: number | null;
}

const win = getCurrentWindow();
const id = win.label.slice("note-".length);

const meta = ref<Meta | null>(null);
const content = ref("");
const loaded = ref(false);
const error = ref<string | null>(null);
const editor = ref<InstanceType<typeof NoteEditor> | null>(null);
const maximized = ref(false);

const name = computed(() => meta.value?.path.split("/").pop() ?? "");
const folder = computed(() => (meta.value?.path.includes("/") ? meta.value.path.slice(0, meta.value.path.lastIndexOf("/")) : ""));
const words = computed(() => content.value.trim().split(/\s+/).filter(Boolean).length);

// --- Thème ----------------------------------------------------------------------------------

const theme = ref<"dark" | "light">("dark");
function applyTheme(v: "dark" | "light") {
  theme.value = v;
  const root = document.documentElement;
  root.classList.toggle("light", v === "light");
  root.classList.toggle("dark", v === "dark");
  root.style.colorScheme = v;
}
const toggleTheme = () => void emit("theme://set", theme.value === "light" ? "dark" : "light");

// --- Chargement / sauvegarde ----------------------------------------------------------------

/** Dernier contenu écrit (ou lu) sur disque : évite de réécrire ce qu'on vient de recevoir. */
let persisted = "";
let saveTimer: number | undefined;
const saving = ref(false);
const savedAt = ref<number | null>(null);

async function load() {
  try {
    const [m, text] = await Promise.all([invoke<Meta>("notes_meta", { id }), invoke<string>("notes_read", { id })]);
    meta.value = m;
    persisted = text;
    content.value = text;
    void win.setTitle(m.path.split("/").pop() ?? "Note");
  } catch (e) {
    error.value = String(e);
  } finally {
    loaded.value = true;
  }
}

async function save() {
  window.clearTimeout(saveTimer);
  if (content.value === persisted) return;
  const text = content.value;
  saving.value = true;
  try {
    meta.value = await invoke<Meta>("notes_write", { id, content: text });
    persisted = text;
    savedAt.value = Date.now();
  } catch (e) {
    error.value = String(e);
  } finally {
    saving.value = false;
  }
}

watch(content, () => {
  if (!loaded.value) return;
  touch();
  window.clearTimeout(saveTimer);
  saveTimer = window.setTimeout(save, 600);
});

// --- Renommage ------------------------------------------------------------------------------

const renaming = ref<string | null>(null);
async function commitRename() {
  const v = renaming.value?.trim();
  renaming.value = null;
  if (!v || !meta.value || v === name.value) return;
  try {
    meta.value = await invoke<Meta>("notes_move", { id, path: folder.value ? `${folder.value}/${v}` : v });
    void win.setTitle(v);
  } catch (e) {
    error.value = String(e);
  }
}

async function togglePin() {
  if (!meta.value) return;
  const pinned = !meta.value.pinned;
  await invoke("notes_pin", { id, pinned }).catch(() => {});
  meta.value = { ...meta.value, pinned };
}

// --- Fenêtre --------------------------------------------------------------------------------

async function close() {
  await save();
  await win.close();
}
const minimize = () => void win.minimize();
const toggleMaximize = () => void win.toggleMaximize();

let lastTouch = 0;
function touch() {
  const now = Date.now();
  if (now - lastTouch < 10_000) return;
  lastTouch = now;
  void emit("hello://touch", "notes");
}

const unlisten: UnlistenFn[] = [];
function onKey(e: KeyboardEvent) {
  if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "s") {
    e.preventDefault();
    void save();
  } else if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "w") {
    e.preventDefault();
    void close();
  }
}

onMounted(async () => {
  window.addEventListener("keydown", onKey);
  window.addEventListener("pointerdown", touch);
  unlisten.push(await listen<"dark" | "light">("theme://changed", (e) => applyTheme(e.payload)));
  void emit("theme://get");

  // Modifiée ailleurs (liste du notch, autre fenêtre…) : métadonnées à jour, contenu relu
  // seulement si on n'a rien en attente ; supprimée ou mise à la corbeille : on ferme.
  unlisten.push(
    await listen<{ id?: string; source?: string }>("notes://changed", async (e) => {
      if (e.payload?.source === win.label || (e.payload?.id && e.payload.id !== id)) return;
      const m = await invoke<Meta>("notes_meta", { id }).catch(() => null);
      if (!m || m.trashed) return void win.close();
      meta.value = m;
      void win.setTitle(m.path.split("/").pop() ?? "Note");
      if (content.value !== persisted) return;
      const text = await invoke<string>("notes_read", { id }).catch(() => null);
      if (text !== null && text !== persisted) {
        persisted = text;
        content.value = text;
      }
    }),
  );
  unlisten.push(await win.onResized(async () => (maximized.value = await win.isMaximized())));
  unlisten.push(await win.onCloseRequested(() => save()));

  await load();
  touch();
});

onBeforeUnmount(() => {
  window.removeEventListener("keydown", onKey);
  window.removeEventListener("pointerdown", touch);
  unlisten.forEach((u) => u());
});
</script>

<template>
  <div class="flex h-screen flex-col overflow-hidden bg-elevated text-label">
    <!-- Barre de titre (zone de déplacement) -->
    <header class="flex h-10 shrink-0 items-center gap-1 border-b border-separator pr-1 pl-3" data-tauri-drag-region @dblclick.self="toggleMaximize">
      <div class="flex min-w-0 flex-1 items-center gap-1.5" data-tauri-drag-region>
        <span v-if="folder" class="type-caption shrink-0 truncate text-label-3" data-tauri-drag-region>{{ folder }} /</span>
        <input
          v-if="renaming !== null"
          v-model="renaming"
          class="type-footnote min-w-0 flex-1 rounded-[6px] bg-fill-1 px-1.5 py-0.5 font-medium outline-none"
          autofocus
          @keydown.enter="commitRename"
          @keydown.escape="renaming = null"
          @blur="commitRename"
        />
        <button v-else type="button" class="type-footnote min-w-0 truncate rounded-[6px] px-1 py-0.5 font-medium hover:bg-fill-1" :title="t('notes.rename')" @click="renaming = name">
          {{ name || "…" }}
        </button>
        <span class="type-caption flex shrink-0 items-center gap-1 text-label-4" data-tauri-drag-region>
          <template v-if="saving">…</template>
          <template v-else-if="savedAt"><PhCheck :size="10" weight="bold" />{{ t("notes.saved") }}</template>
        </span>
      </div>
      <IconButton :icon="PhArrowCounterClockwise" :label="t('notes.undo')" :disabled="!editor?.canUndo" @click="editor?.undo()" />
      <IconButton :icon="PhArrowClockwise" :label="t('notes.redo')" :disabled="!editor?.canRedo" @click="editor?.redo()" />
      <IconButton :icon="PhPushPin" :active="meta?.pinned" :label="meta?.pinned ? t('notes.unpin') : t('notes.pin')" @click="togglePin" />
      <IconButton :icon="theme === 'light' ? PhMoon : PhSun" :label="t('settings.theme')" @click="toggleTheme" />
      <div class="mx-1 h-4 w-px bg-separator" />
      <IconButton :icon="PhMinus" :label="t('notes.minimize')" @click="minimize" />
      <IconButton :icon="maximized ? PhCornersIn : PhCornersOut" :label="t('notes.maximize')" @click="toggleMaximize" />
      <IconButton :icon="PhX" :label="t('common.close')" class="hover:bg-sys-red! hover:text-white!" @click="close" />
    </header>

    <!-- Éditeur -->
    <main class="flex min-h-0 flex-1 flex-col">
      <div v-if="error" class="type-footnote m-4 flex items-center gap-2 rounded-[10px] bg-sys-red/10 px-3 py-2 text-sys-red">
        <PhWarning :size="14" weight="bold" />{{ t("notes.error", { e: error }) }}
      </div>
      <!-- L'éditeur gère son défilement et ses marges (la gouttière de la poignée de bloc). -->
      <NoteEditor v-if="loaded && !error" ref="editor" v-model="content" class="min-h-0 flex-1" :placeholder="t('notes.editorPlaceholder')" autofocus />
    </main>

    <footer class="type-caption flex h-6 shrink-0 items-center justify-end px-3 text-label-4">
      {{ words === 1 ? t("notes.word", { n: words }) : t("notes.words", { n: words }) }}
    </footer>
  </div>
</template>
