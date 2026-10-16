<script setup lang="ts">
// Outil « Notes » (680 × 420) : dossiers à gauche (Toutes, Épinglées, dossiers, Corbeille),
// liste à droite avec recherche plein texte. Double-clic = ouvrir la note dans sa propre
// fenêtre. Glisser une note sur un dossier = la déplacer. Verrou Windows Hello en option
// (Réglages › Outils › Verrouillage) : tout reste masqué jusqu'à la confirmation, et les
// fenêtres de notes se ferment au verrouillage (cf. notch/hello.ts).
import { computed, onMounted, ref, watch } from "vue";
import {
  PhArrowCounterClockwise,
  PhFingerprint,
  PhFolder,
  PhFolderPlus,
  PhLockSimple,
  PhLockSimpleOpen,
  PhNotePencil,
  PhNotebook,
  PhPencilSimple,
  PhPushPin,
  PhTrash,
  PhX,
} from "@phosphor-icons/vue";
import IconButton from "../../ui/IconButton.vue";
import TextField from "../../ui/TextField.vue";
import Empty from "../../ui/Empty.vue";
import { Button } from "@/sdk/ui/button";
import {
  createFolder,
  createNote,
  deleteFolder,
  folderOf,
  folders,
  initNotesList,
  inScope,
  moveFolder,
  moveNote,
  nameOf,
  notes,
  openNote,
  pinNote,
  purgeNote,
  searchNotes,
  trashNote,
  type Note,
  type Scope,
} from "./store";
import { useDraft } from "../../stores/drafts";
import { settings } from "../../notch/settings";
import { hello, helloError, isLocked, lockScope, touchScope, unlockScope } from "../../notch/hello";
import { locale } from "../../sdk/i18n";
import { t } from "../../notch/i18n";

const scope = useDraft<Scope>("notes.scope", () => "all");
const query = useDraft("notes.query", () => "");
const selected = ref<string | null>(null);
const busy = ref<string | null>(null);

onMounted(initNotesList);

// --- Recherche plein texte (Rust), débouncée ----------------------------------------------

const hits = ref<Set<string> | null>(null);
let searchTimer: number | undefined;
watch(
  query,
  (q) => {
    window.clearTimeout(searchTimer);
    if (!q.trim()) return void (hits.value = null);
    searchTimer = window.setTimeout(async () => {
      hits.value = new Set(await searchNotes(q).catch(() => []));
    }, 200);
  },
  { immediate: true },
);

const visible = computed<Note[]>(() => {
  if (hits.value) return notes.list.filter((n) => !n.trashed && hits.value!.has(n.id)).sort((a, b) => b.updated - a.updated);
  return inScope(scope.value);
});

const currentFolder = computed(() => (scope.value.startsWith("folder:") ? scope.value.slice(7) : ""));
const count = (s: Scope) => inScope(s).length;
const depth = (f: string) => f.split("/").length - 1;

function age(ms: number): string {
  const m = Math.floor((Date.now() - ms) / 60_000);
  if (m < 1) return t("clip.now");
  if (m < 60) return t("clip.min", { n: m });
  const h = Math.floor(m / 60);
  if (h < 24) return t("clip.hour", { n: h });
  return new Intl.DateTimeFormat(locale.value, { day: "numeric", month: "short" }).format(new Date(ms));
}

/** Exécute une action en bloquant les clics répétés (`key` = ce qui tourne). */
async function act(key: string, fn: () => Promise<unknown>) {
  if (busy.value) return;
  busy.value = key;
  touchScope("notes");
  try {
    await fn();
  } finally {
    busy.value = null;
  }
}

// --- Notes --------------------------------------------------------------------------------

function newNote() {
  void act("new", async () => {
    const n = await createNote(currentFolder.value, t("notes.untitled"));
    selected.value = n.id;
    await openNote(n);
  });
}

const open = (n: Note) => act(`open:${n.id}`, () => openNote(n));

const renaming = ref<{ id: string; name: string } | null>(null);
function startRename(n: Note) {
  renaming.value = { id: n.id, name: nameOf(n.path) };
}
function commitRename(n: Note) {
  const r = renaming.value;
  renaming.value = null;
  if (!r?.name.trim() || r.name.trim() === nameOf(n.path)) return;
  const folder = folderOf(n.path);
  void act(`rename:${n.id}`, () => moveNote(n.id, folder ? `${folder}/${r.name.trim()}` : r.name.trim()));
}

// --- Dossiers -----------------------------------------------------------------------------

const newFolder = ref<string | null>(null);
function commitFolder() {
  const name = newFolder.value?.trim();
  newFolder.value = null;
  if (!name) return;
  const path = currentFolder.value ? `${currentFolder.value}/${name}` : name;
  void act("folder", async () => {
    const p = await createFolder(path);
    scope.value = `folder:${p}`;
  });
}

const renamingFolder = ref<{ path: string; name: string } | null>(null);
function commitFolderRename() {
  const r = renamingFolder.value;
  renamingFolder.value = null;
  if (!r?.name.trim()) return;
  const parent = folderOf(r.path);
  const to = parent ? `${parent}/${r.name.trim()}` : r.name.trim();
  if (to === r.path) return;
  void act(`folder:${r.path}`, async () => {
    await moveFolder(r.path, to);
    if (scope.value === `folder:${r.path}`) scope.value = `folder:${to}`;
  });
}

/** Suppression de dossier en deux clics (ses notes vont à la corbeille). */
const confirmFolder = ref<string | null>(null);
function removeFolder(path: string) {
  if (confirmFolder.value !== path) {
    confirmFolder.value = path;
    window.setTimeout(() => confirmFolder.value === path && (confirmFolder.value = null), 3000);
    return;
  }
  confirmFolder.value = null;
  void act(`folder:${path}`, async () => {
    await deleteFolder(path);
    if (scope.value.startsWith(`folder:${path}`)) scope.value = "all";
  });
}

// --- Glisser une note sur un dossier ------------------------------------------------------

const dropTarget = ref<string | null>(null);
function onDragStart(e: DragEvent, n: Note) {
  e.dataTransfer?.setData("application/x-island-note", n.id);
  if (e.dataTransfer) e.dataTransfer.effectAllowed = "move";
}
function onDrop(e: DragEvent, folder: string) {
  dropTarget.value = null;
  const id = e.dataTransfer?.getData("application/x-island-note");
  const n = notes.list.find((x) => x.id === id);
  if (!n || folderOf(n.path) === folder) return;
  void act(`move:${n.id}`, () => moveNote(n.id, folder ? `${folder}/${nameOf(n.path)}` : nameOf(n.path)));
}
const isNoteDrag = (e: DragEvent) => !!e.dataTransfer?.types.includes("application/x-island-note");

// --- Verrou -----------------------------------------------------------------------------

const locked = computed(() => isLocked("notes"));
const lockError = ref<string | null>(null);
async function unlock() {
  if (hello.prompting) return;
  lockError.value = null;
  try {
    await unlockScope("notes");
  } catch (e) {
    lockError.value = helloError(t, e);
  }
}
onMounted(() => {
  if (locked.value) void unlock();
});

const item = "type-footnote flex h-7 w-full items-center gap-2 rounded-[8px] px-2 text-left transition-colors duration-100";
</script>

<template>
  <div class="flex h-full flex-col px-4 pb-1" @pointerdown="touchScope('notes')">
    <Teleport defer to="#notch-header-actions">
      <template v-if="!locked">
        <IconButton :icon="PhFolderPlus" :label="t('notes.newFolder')" @click="newFolder = ''" />
        <IconButton :icon="PhNotePencil" :label="t('notes.new')" :loading="busy === 'new'" :disabled="!!busy" @click="newNote" />
      </template>
      <IconButton v-if="settings.hello.notes && !locked" :icon="PhLockSimpleOpen" active :label="t('notes.lock')" @click="lockScope('notes')" />
    </Teleport>

    <Empty v-if="locked" :icon="PhLockSimple" :title="t('notes.locked')" :text="lockError ?? t('notes.lockedHelp')">
      <Button variant="default" :loading="hello.prompting" @click="unlock"><PhFingerprint weight="bold" />{{ t("hello.unlock") }}</Button>
    </Empty>

    <div v-else class="flex min-h-0 flex-1 gap-3" data-no-drag>
      <!-- Dossiers -->
      <nav class="flex w-[170px] shrink-0 flex-col gap-px overflow-y-auto [scrollbar-width:none]">
        <button
          type="button"
          :class="[item, scope === 'all' && !query ? 'bg-fill-2 text-label' : 'text-label-2 hover:bg-fill-1', dropTarget === '' ? 'ring-1 ring-sys-link' : '']"
          @click="(scope = 'all'), (query = '')"
          @dragover.prevent="isNoteDrag($event) && (dropTarget = '')"
          @dragleave="dropTarget = null"
          @drop.prevent="onDrop($event, '')"
        >
          <PhNotebook :size="14" weight="bold" /><span class="flex-1 truncate">{{ t("notes.all") }}</span>
          <span class="type-caption text-label-3">{{ count("all") }}</span>
        </button>
        <button type="button" :class="[item, scope === 'pinned' && !query ? 'bg-fill-2 text-label' : 'text-label-2 hover:bg-fill-1']" @click="(scope = 'pinned'), (query = '')">
          <PhPushPin :size="14" weight="bold" /><span class="flex-1 truncate">{{ t("notes.pinned") }}</span>
          <span class="type-caption text-label-3">{{ count("pinned") }}</span>
        </button>

        <div class="type-caption mt-2 mb-0.5 px-2 text-label-3">{{ t("notes.folders") }}</div>
        <template v-for="f in folders" :key="f">
          <div v-if="renamingFolder?.path === f" class="py-0.5" :style="{ paddingLeft: `${depth(f) * 12}px` }">
            <TextField v-model="renamingFolder.name" autofocus @enter="commitFolderRename" @blur="commitFolderRename" />
          </div>
          <div
            v-else
            class="group/f relative"
            @dragover.prevent="isNoteDrag($event) && (dropTarget = f)"
            @dragleave="dropTarget === f && (dropTarget = null)"
            @drop.prevent="onDrop($event, f)"
          >
            <button
              type="button"
              :class="[item, scope === `folder:${f}` && !query ? 'bg-fill-2 text-label' : 'text-label-2 hover:bg-fill-1', dropTarget === f ? 'ring-1 ring-sys-link' : '']"
              :style="{ paddingLeft: `${8 + depth(f) * 12}px` }"
              @click="(scope = `folder:${f}`), (query = '')"
            >
              <PhFolder :size="14" :weight="scope === `folder:${f}` ? 'fill' : 'bold'" class="shrink-0" />
              <span class="flex-1 truncate">{{ nameOf(f) }}</span>
            </button>
            <div class="absolute top-0.5 right-0.5 flex gap-px opacity-0 group-hover/f:opacity-100" :class="{ 'opacity-100': confirmFolder === f }">
              <Button size="icon-xs" variant="ghost" class="text-label-3" :title="t('notes.rename')" @click="renamingFolder = { path: f, name: nameOf(f) }">
                <PhPencilSimple weight="bold" />
              </Button>
              <Button
                size="icon-xs"
                :variant="confirmFolder === f ? 'destructive' : 'ghost'"
                :class="confirmFolder === f ? '' : 'text-label-3'"
                :title="t('notes.deleteFolder')"
                @click="removeFolder(f)"
              >
                <PhTrash weight="bold" />
              </Button>
            </div>
          </div>
        </template>
        <div v-if="newFolder !== null" class="py-0.5">
          <TextField v-model="newFolder" autofocus :placeholder="t('notes.folderName')" @enter="commitFolder" @blur="commitFolder" />
        </div>
        <div v-if="!folders.length && newFolder === null" class="type-caption px-2 text-label-4">{{ t("notes.noFolder") }}</div>

        <button type="button" :class="[item, 'mt-auto', scope === 'trash' && !query ? 'bg-fill-2 text-label' : 'text-label-3 hover:bg-fill-1']" @click="(scope = 'trash'), (query = '')">
          <PhTrash :size="14" weight="bold" /><span class="flex-1 truncate">{{ t("notes.trash") }}</span>
          <span class="type-caption">{{ count("trash") }}</span>
        </button>
      </nav>

      <!-- Liste -->
      <div class="flex min-w-0 flex-1 flex-col gap-1.5">
        <TextField v-model="query" :placeholder="t('notes.search')" />
        <div class="min-h-0 flex-1 overflow-y-auto pr-1 [scrollbar-color:var(--fill-3)_transparent] [scrollbar-width:thin]">
          <div v-if="!notes.loaded" class="type-caption py-6 text-center text-label-3">…</div>
          <Empty
            v-else-if="!visible.length"
            :icon="PhNotebook"
            :title="query ? t('notes.noResult') : scope === 'trash' ? t('notes.trashEmpty') : t('notes.empty')"
            :text="query || scope === 'trash' ? undefined : t('notes.emptyHelp')"
          />
          <div
            v-for="n in visible"
            :key="n.id"
            class="group/n relative flex cursor-default flex-col gap-0.5 rounded-[10px] px-2.5 py-2 transition-colors duration-100"
            :class="selected === n.id ? 'bg-fill-2' : 'hover:bg-fill-1'"
            :draggable="!n.trashed && renaming?.id !== n.id"
            @dragstart="onDragStart($event, n)"
            @click="selected = n.id"
            @dblclick="!n.trashed && open(n)"
          >
            <div class="flex items-center gap-1.5">
              <PhPushPin v-if="n.pinned && !n.trashed" :size="11" weight="fill" class="shrink-0 text-sys-orange" />
              <div v-if="renaming?.id === n.id" class="min-w-0 flex-1" @click.stop @dblclick.stop>
                <TextField v-model="renaming.name" autofocus @enter="commitRename(n)" @blur="commitRename(n)" />
              </div>
              <span v-else class="type-footnote min-w-0 flex-1 truncate font-medium">{{ nameOf(n.path) }}</span>
              <span class="type-caption shrink-0 text-label-3 group-hover/n:opacity-0">{{ age(n.updated) }}</span>
            </div>
            <div class="type-caption truncate text-label-3">
              <span v-if="(query || scope === 'all' || scope === 'pinned') && folderOf(n.path)" class="text-label-2">{{ folderOf(n.path) }} · </span>{{ n.preview || t("notes.blank") }}
            </div>
            <!-- Actions -->
            <div class="absolute top-1.5 right-1.5 flex gap-px opacity-0 transition-opacity duration-100 group-hover/n:opacity-100" @click.stop @dblclick.stop>
              <template v-if="!n.trashed">
                <Button size="icon-xs" variant="ghost" class="text-label-3" :title="n.pinned ? t('notes.unpin') : t('notes.pin')" @click="act(`pin:${n.id}`, () => pinNote(n.id, !n.pinned))">
                  <PhPushPin :weight="n.pinned ? 'fill' : 'bold'" />
                </Button>
                <Button size="icon-xs" variant="ghost" class="text-label-3" :title="t('notes.rename')" @click="startRename(n)"><PhPencilSimple weight="bold" /></Button>
                <Button size="icon-xs" variant="ghost" class="text-label-3" :title="t('notes.delete')" @click="act(`trash:${n.id}`, () => trashNote(n.id, true))">
                  <PhTrash weight="bold" />
                </Button>
              </template>
              <template v-else>
                <Button size="icon-xs" variant="ghost" class="text-label-3" :title="t('notes.restore')" @click="act(`restore:${n.id}`, () => trashNote(n.id, false))">
                  <PhArrowCounterClockwise weight="bold" />
                </Button>
                <Button size="icon-xs" variant="ghost" class="text-sys-red" :title="t('notes.purge')" @click="act(`purge:${n.id}`, () => purgeNote(n.id))">
                  <PhX weight="bold" />
                </Button>
              </template>
            </div>
          </div>
        </div>
        <div class="type-caption text-label-4">{{ scope === "trash" ? t("notes.trashHelp") : t("notes.openHelp") }}</div>
      </div>
    </div>
  </div>
</template>
