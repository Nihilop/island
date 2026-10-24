<script setup lang="ts">
// Vue « Projets » : liste à gauche (recherche, ajout de dossier), fiche à droite (ouvrir,
// scripts, conteneurs). Affiche tout de suite le dernier état connu, puis rescanne.
import { computed, onMounted, onUnmounted, reactive, ref, watch } from "vue";
import type { Island } from "@island/sdk";
import Icon from "./Icon.vue";
import Detail from "./Detail.vue";
import { scan, type Project } from "../projects";

const props = defineProps<{ island: Island }>();
const island = props.island;
const t = island.t;

const projects = ref<Project[]>([]);
const roots = ref<string[]>([]);
const selected = ref<string | null>(null);
const query = ref("");
const scanning = ref(false);
const adding = ref(false);
const showRoots = ref(false);
const error = ref<string | null>(null);

const filtered = computed(() => {
  const words = query.value.toLowerCase().split(/\s+/).filter(Boolean);
  return projects.value.filter((p) => words.every((w) => `${p.name} ${p.path} ${p.kinds.join(" ")}`.toLowerCase().includes(w)));
});
const current = computed(() => projects.value.find((p) => p.path === selected.value) ?? filtered.value[0] ?? null);

async function rescan() {
  if (scanning.value) return;
  scanning.value = true;
  error.value = null;
  try {
    roots.value = await island.files.folders();
    projects.value = await scan(island);
    await island.storage.set("projects", projects.value);
    await island.events.emit("projects:changed");
  } catch (e) {
    error.value = String((e as Error)?.message ?? e);
  } finally {
    scanning.value = false;
  }
}

async function addFolder() {
  if (adding.value) return;
  adding.value = true;
  try {
    const dir = await island.files.pick(t("add.title"));
    if (dir) await rescan();
  } catch (e) {
    error.value = String((e as Error)?.message ?? e);
  } finally {
    adding.value = false;
  }
}

const removing = reactive(new Set<string>());
async function forget(root: string) {
  if (removing.has(root)) return;
  removing.add(root);
  try {
    await island.files.forget(root);
    await rescan();
  } finally {
    removing.delete(root);
  }
}

watch(selected, (p) => void island.storage.set("selected", p));

const ICON: Record<string, string> = { unreal: "game", node: "code", compose: "box", sln: "file" };
const mainIcon = (p: Project) => ICON[p.kinds[0]] ?? "folder";

onMounted(async () => {
  projects.value = (await island.storage.get<Project[]>("projects")) ?? [];
  selected.value = await island.storage.get<string>("selected");
  void rescan();
});
// Ouverture depuis le lanceur (« Afficher » sur un projet).
island.events.on<string>("projects:select", (path) => {
  query.value = "";
  selected.value = path;
});

// Raccourcis : ↑ ↓ dans la liste.
function onKey(e: KeyboardEvent) {
  if (e.key !== "ArrowDown" && e.key !== "ArrowUp") return;
  const list = filtered.value;
  if (!list.length) return;
  const i = Math.max(0, list.findIndex((p) => p.path === current.value?.path));
  selected.value = list[(i + (e.key === "ArrowDown" ? 1 : list.length - 1)) % list.length].path;
  e.preventDefault();
}
onMounted(() => addEventListener("keydown", onKey));
onUnmounted(() => removeEventListener("keydown", onKey));
</script>

<template>
  <div class="shell">
    <aside>
      <div class="search">
        <Icon name="search" :size="12" />
        <input v-model="query" :placeholder="t('search')" spellcheck="false" />
      </div>
      <div class="list scroll">
        <button v-for="p in filtered" :key="p.path" class="row" :class="{ on: current?.path === p.path }" @click="selected = p.path">
          <span class="kind" :class="p.kinds[0]"><Icon :name="mainIcon(p)" :size="13" /></span>
          <span class="grow">
            <span class="truncate name">{{ p.name }}</span>
            <span class="caption truncate">{{ p.kinds.map((k) => t(`kind.${k}`)).join(" · ") }}</span>
          </span>
        </button>
        <div v-if="!filtered.length && !scanning" class="empty caption">
          {{ projects.length ? t("noMatch") : t("empty") }}
        </div>
        <div v-if="!projects.length && scanning" class="empty caption"><span class="spinner" /> {{ t("scanning") }}</div>
      </div>
      <footer>
        <button class="btn primary" :disabled="adding" @click="addFolder">
          <span v-if="adding" class="spinner" /><Icon v-else name="plus" :size="12" />{{ t("add") }}
        </button>
        <button class="btn icon" :title="t('folders')" @click="showRoots = !showRoots"><Icon name="folder" :size="13" />{{ roots.length }}</button>
        <button class="btn icon" :title="t('refresh')" :disabled="scanning" @click="rescan">
          <span v-if="scanning" class="spinner" /><Icon v-else name="refresh" :size="13" />
        </button>
      </footer>
    </aside>

    <main class="scroll">
      <div v-if="error" class="error">{{ error }} <button class="link" @click="error = null"><Icon name="x" :size="11" /></button></div>
      <section v-if="showRoots" class="roots">
        <h3>{{ t("folders") }}</h3>
        <p class="caption">{{ t("foldersHint") }}</p>
        <div v-for="r in roots" :key="r" class="item">
          <Icon name="folder" :size="13" />
          <span class="grow truncate">{{ r }}</span>
          <button class="btn icon" :title="t('forget')" :disabled="removing.has(r)" @click="forget(r)">
            <span v-if="removing.has(r)" class="spinner" /><Icon v-else name="trash" :size="12" />
          </button>
        </div>
        <div v-if="!roots.length" class="caption">{{ t("noFolders") }}</div>
      </section>
      <Detail v-else-if="current" :key="current.path" :island="island" :project="current" @error="error = $event" />
      <div v-else class="welcome">
        <Icon name="folder" :size="28" />
        <p>{{ t("welcome") }}</p>
        <button class="btn primary" :disabled="adding" @click="addFolder"><span v-if="adding" class="spinner" />{{ t("add") }}</button>
      </div>
    </main>
  </div>
</template>
