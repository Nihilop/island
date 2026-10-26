<script setup lang="ts">
// Fiche d'un projet : ouvrir (éditeur, explorateur, terminal), Unreal (.uproject, .sln),
// scripts npm, conteneurs Docker Compose (suivis toutes les 3 s tant que la fiche est
// affichée).
import { computed, onMounted, onUnmounted, reactive, ref } from "vue";
import type { Island } from "@island/sdk";
import Icon from "./Icon.vue";
import { CARGO, cargo, compose, fileName, launchUnreal, openEditor, openTerminal, packageManager, runScript, runnables, type Project } from "../projects";
import { act, containers, type Container } from "../docker";

const props = defineProps<{ island: Island; project: Project }>();
const emit = defineEmits<{ error: [string] }>();
const island = props.island;
const t = island.t;
const p = computed(() => props.project);

// Actions en cours (une clé par bouton : spinner, pas de double clic).
const busy = reactive(new Set<string>());
async function run(key: string, fn: () => Promise<unknown>) {
  if (busy.has(key)) return;
  busy.add(key);
  try {
    await fn();
  } catch (e) {
    emit("error", explain(e));
  } finally {
    // Les lancements rendent la main tout de suite : on garde le retour visuel un instant.
    setTimeout(() => busy.delete(key), 600);
  }
}
const explain = (e: unknown) => {
  const msg = String((e as Error)?.message ?? e);
  const key = `err.${msg.split(":")[0]}`;
  const s = t(key);
  return s === key ? msg : s;
};

const pm = ref("npm");
onMounted(async () => (pm.value = await packageManager(island, p.value)));

// --- Docker ---------------------------------------------------------------------------------

const list = ref<Container[]>([]);
const docker = ref<"loading" | "ok" | "unavailable" | string>("loading");
let timer: number | undefined;

async function refreshContainers() {
  const r = await containers(island, p.value);
  if (r.ok) {
    list.value = r.list;
    docker.value = "ok";
  } else docker.value = r.error;
}
onMounted(() => {
  if (!p.value.compose.length) return;
  void refreshContainers();
  timer = window.setInterval(() => document.visibilityState === "visible" && void refreshContainers(), 3000);
});
onUnmounted(() => clearInterval(timer));

const container = (c: Container, action: "start" | "stop" | "restart") =>
  run(`${c.id}:${action}`, async () => {
    await act(island, c.id, action);
    await refreshContainers();
  });
const running = computed(() => list.value.filter((c) => c.state === "running").length);
</script>

<template>
  <div class="detail">
    <header>
      <div class="grow">
        <h2 class="truncate">{{ p.name }}</h2>
        <div class="caption truncate" :title="p.path">{{ p.path }}</div>
      </div>
      <button class="btn" :disabled="busy.has('editor')" @click="run('editor', () => openEditor(island, p))">
        <span v-if="busy.has('editor')" class="spinner" /><Icon v-else name="code" />{{ t("editor") }}
      </button>
      <button class="btn icon" :title="t('explorer')" :disabled="busy.has('reveal')" @click="run('reveal', () => island.files.open(p.path))">
        <span v-if="busy.has('reveal')" class="spinner" /><Icon v-else name="folder" />
      </button>
      <button class="btn icon" :title="t('terminal')" :disabled="busy.has('term')" @click="run('term', () => openTerminal(island, p))">
        <span v-if="busy.has('term')" class="spinner" /><Icon v-else name="terminal" />
      </button>
    </header>

    <section v-if="p.uprojects.length || p.solutions.length">
      <h3>{{ p.uprojects.length ? t("kind.unreal") : t("kind.sln") }}</h3>
      <div class="grid">
        <button v-for="u in p.uprojects" :key="`launch:${u}`" class="tile launch" :disabled="busy.has(`launch:${u}`)" @click="run(`launch:${u}`, () => launchUnreal(island, p, u))">
          <span v-if="busy.has(`launch:${u}`)" class="spinner" /><Icon v-else name="play" :size="16" />
          <span class="grow">
            <span class="truncate">{{ t("unreal.launchShort") }}</span>
            <span class="caption">{{ t("unreal.launch") }}</span>
          </span>
        </button>
        <button v-for="f in [...p.uprojects, ...p.solutions]" :key="f" class="tile" :disabled="busy.has(f)" @click="run(f, () => island.files.open(f))">
          <span v-if="busy.has(f)" class="spinner" /><Icon v-else :name="f.toLowerCase().endsWith('.uproject') ? 'game' : 'file'" :size="16" />
          <span class="grow">
            <span class="truncate">{{ fileName(f) }}</span>
            <span class="caption">{{ f.toLowerCase().endsWith(".uproject") ? t("open.uproject") : t("open.sln") }}</span>
          </span>
        </button>
      </div>
    </section>

    <section v-if="p.kinds.includes('node')">
      <h3>{{ t("scripts") }} <span class="caption">{{ pm }}</span></h3>
      <div v-if="p.scripts.length" class="chips">
        <button v-for="s in p.scripts" :key="s" class="chip" :disabled="busy.has(`s:${s}`)" :title="`${pm} run ${s}`" @click="run(`s:${s}`, () => runScript(island, p, s))">
          <span v-if="busy.has(`s:${s}`)" class="spinner" /><Icon v-else name="play" :size="10" />{{ s }}
        </button>
      </div>
      <p v-else class="caption">{{ t("noScripts") }}</p>
    </section>

    <section v-if="p.kinds.includes('rust')">
      <h3>Rust <span class="caption">{{ t("rust.crates", { n: p.crates.length }) }}</span></h3>
      <div class="chips">
        <button
          v-for="r in runnables(p)"
          :key="r.command"
          class="chip"
          :disabled="busy.has(`r:${r.command}`)"
          :title="r.command"
          @click="run(`r:${r.command}`, () => cargo(island, p, r.command, `run ${r.label}`))"
        >
          <span v-if="busy.has(`r:${r.command}`)" class="spinner" /><Icon v-else name="play" :size="10" />run {{ r.label }}
        </button>
        <button v-for="c in CARGO" :key="c" class="chip plain" :disabled="busy.has(`c:${c}`)" :title="`cargo ${c}`" @click="run(`c:${c}`, () => cargo(island, p, `cargo ${c}`, c))">
          <span v-if="busy.has(`c:${c}`)" class="spinner" /><Icon v-else name="terminal" :size="10" />{{ c }}
        </button>
      </div>
    </section>

    <section v-if="p.compose.length">
      <h3>
        Docker <span v-if="docker === 'ok'" class="caption">{{ t("docker.count", { on: running, all: list.length }) }}</span>
        <span class="spacer" />
        <button class="btn small" :disabled="busy.has('up')" @click="run('up', () => compose(island, p, 'up -d'))">
          <span v-if="busy.has('up')" class="spinner" /><Icon v-else name="up" :size="11" />up
        </button>
        <button class="btn small" :disabled="busy.has('down')" @click="run('down', () => compose(island, p, 'down'))">
          <span v-if="busy.has('down')" class="spinner" /><Icon v-else name="down" :size="11" />down
        </button>
      </h3>
      <p v-if="docker === 'loading'" class="caption"><span class="spinner" /> {{ t("docker.loading") }}</p>
      <p v-else-if="docker === 'unavailable'" class="caption">{{ t("docker.off") }}</p>
      <p v-else-if="docker !== 'ok'" class="caption">{{ docker }}</p>
      <p v-else-if="!list.length" class="caption">{{ t("docker.none") }}</p>
      <div v-else class="list">
        <div v-for="c in list" :key="c.id" class="item">
          <span class="dot" :class="c.state" />
          <span class="grow">
            <span class="truncate">{{ c.service || c.name }}</span>
            <span class="caption truncate">{{ c.status }} · {{ c.image }}</span>
          </span>
          <button v-if="c.state !== 'running'" class="btn icon" :title="t('docker.start')" :disabled="busy.has(`${c.id}:start`)" @click="container(c, 'start')">
            <span v-if="busy.has(`${c.id}:start`)" class="spinner" /><Icon v-else name="play" :size="11" />
          </button>
          <template v-else>
            <button class="btn icon" :title="t('docker.restart')" :disabled="busy.has(`${c.id}:restart`)" @click="container(c, 'restart')">
              <span v-if="busy.has(`${c.id}:restart`)" class="spinner" /><Icon v-else name="restart" :size="11" />
            </button>
            <button class="btn icon" :title="t('docker.stop')" :disabled="busy.has(`${c.id}:stop`)" @click="container(c, 'stop')">
              <span v-if="busy.has(`${c.id}:stop`)" class="spinner" /><Icon v-else name="stop" :size="11" />
            </button>
          </template>
        </div>
      </div>
      <div class="caption files">{{ p.compose.map(fileName).join(", ") }}</div>
    </section>
  </div>
</template>
