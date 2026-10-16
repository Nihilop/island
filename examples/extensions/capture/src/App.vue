<script setup lang="ts">
// Outil Capture : mode (capture d'écran / vidéo), cible en un clic (zone, fenêtre, écran),
// son du PC et micro pour la vidéo, dernières captures (clic = afficher dans l'Explorateur).
// Pendant un enregistrement : chrono et bouton Arrêter.
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import type { Island, RecordingState } from "@island/sdk";
import { PhAppWindow, PhImage, PhMicrophone, PhMonitor, PhRecord, PhSelection, PhSpeakerHigh, PhStop, PhVideoCamera } from "@phosphor-icons/vue";
import { clock, fileName, recent as loadRecent, type Mode, type Recent } from "./core";

const props = defineProps<{ island: Island }>();
const island = props.island;
const t = island.t;

type Kind = "shot" | "video";
const kind = ref<Kind>("shot");
const busy = ref<Mode | "stop" | null>(null);
const error = ref<string | null>(null);
const rec = ref<RecordingState>({ recording: false, startedAt: null, target: null });
const now = ref(Date.now());
const recents = ref<Recent[]>([]);
/** Son pour la vidéo : choix rapide ici, réglages de l'extension par défaut. */
const audio = ref({ systemAudio: true, mic: false });

const TARGETS: { id: Mode; icon: typeof PhSelection }[] = [
  { id: "region", icon: PhSelection },
  { id: "window", icon: PhAppWindow },
  { id: "screen", icon: PhMonitor },
];

let tick: ReturnType<typeof setInterval> | undefined;
onMounted(async () => {
  const [state, list, saved, sys, mic] = await Promise.all([
    island.capture.state().catch(() => rec.value),
    loadRecent(island),
    island.storage.get<{ systemAudio: boolean; mic: boolean }>("audio"),
    island.settings.get<boolean>("systemAudio"),
    island.settings.get<boolean>("mic"),
  ]);
  rec.value = state;
  recents.value = list;
  audio.value = saved ?? { systemAudio: sys !== false, mic: !!mic };
  if (state.recording) kind.value = "video";
  island.capture.onRecording((s) => (rec.value = s));
  island.events.on<Recent[]>("recent", (l) => (recents.value = l));
  tick = setInterval(() => (now.value = Date.now()), 500);
});
onBeforeUnmount(() => clearInterval(tick));

async function toggleAudio(key: "systemAudio" | "mic") {
  audio.value = { ...audio.value, [key]: !audio.value[key] };
  await island.storage.set("audio", audio.value);
}

// L'action est confiée à la logique de fond (main.ts) : cette vue disparaît dès que l'île se
// ferme, c'est-à-dire dès que la sélection s'affiche.
async function go(mode: Mode) {
  if (busy.value) return;
  busy.value = mode;
  error.value = null;
  // Objet simple : un proxy réactif de Vue ne peut pas traverser le canal (postMessage).
  const action = kind.value === "shot" ? { kind: "shot", mode } : { kind: "video", mode, audio: { ...audio.value } };
  await island.events.emit("action", action).catch((e) => (error.value = String(e)));
  setTimeout(() => (busy.value = null), 800);
}

async function stop() {
  if (busy.value) return;
  busy.value = "stop";
  await island.events.emit("action", { kind: "stop" });
  setTimeout(() => (busy.value = null), 800);
}

const elapsed = computed(() => (rec.value.startedAt ? clock(now.value - rec.value.startedAt) : "0:00"));
const ago = (at: number) => {
  const m = Math.floor((Date.now() - at) / 60000);
  if (m < 1) return t("ago.now");
  if (m < 60) return t("ago.min", { n: m });
  const h = Math.floor(m / 60);
  return h < 24 ? t("ago.hour", { n: h }) : new Date(at).toLocaleDateString();
};
const reveal = (r: Recent) => void island.capture.reveal(r.path).catch(() => {});
</script>

<template>
  <div class="root">
    <!-- Enregistrement en cours -->
    <section v-if="rec.recording" class="recording">
      <span class="dot" />
      <div class="grow">
        <div class="time">{{ elapsed }}</div>
        <div class="caption">{{ t("rec.running") }}</div>
      </div>
      <button type="button" class="btn stop" :disabled="busy === 'stop'" @click="stop">
        <PhStop :size="15" weight="fill" />{{ t("rec.stop") }}
      </button>
    </section>

    <template v-else>
      <div class="head">
        <div class="segmented">
          <button type="button" :class="{ on: kind === 'shot' }" @click="kind = 'shot'"><PhImage :size="14" weight="bold" />{{ t("kind.shot") }}</button>
          <button type="button" :class="{ on: kind === 'video' }" @click="kind = 'video'"><PhVideoCamera :size="14" weight="bold" />{{ t("kind.video") }}</button>
        </div>
        <div v-if="kind === 'video'" class="chips">
          <button type="button" class="chip" :class="{ on: audio.systemAudio }" :title="t('audio.system')" @click="toggleAudio('systemAudio')">
            <PhSpeakerHigh :size="14" :weight="audio.systemAudio ? 'fill' : 'bold'" />{{ t("audio.system") }}
          </button>
          <button type="button" class="chip" :class="{ on: audio.mic }" :title="t('audio.mic')" @click="toggleAudio('mic')">
            <PhMicrophone :size="14" :weight="audio.mic ? 'fill' : 'bold'" />{{ t("audio.mic") }}
          </button>
        </div>
      </div>

      <div class="targets">
        <button v-for="x in TARGETS" :key="x.id" type="button" class="target" :class="{ video: kind === 'video' }" :disabled="!!busy" @click="go(x.id)">
          <span class="icon">
            <component :is="x.icon" :size="22" weight="duotone" />
            <PhRecord v-if="kind === 'video'" :size="11" weight="fill" class="badge-rec" />
          </span>
          <span class="label">{{ t(`target.${x.id}`) }}</span>
          <span class="caption">{{ t(`target.${x.id}.help`) }}</span>
        </button>
      </div>
    </template>

    <div v-if="error" class="error caption">{{ error }}</div>

    <!-- Dernières captures -->
    <div v-if="recents.length" class="recents">
      <button v-for="r in recents.slice(0, 4)" :key="r.path" type="button" class="recent" :title="r.path" @click="reveal(r)">
        <component :is="r.kind === 'video' ? PhVideoCamera : PhImage" :size="14" weight="bold" class="muted" />
        <span class="truncate">{{ fileName(r.path) }}</span>
        <span class="caption">{{ r.kind === "video" && r.durationMs ? clock(r.durationMs) : ago(r.at) }}</span>
      </button>
    </div>
    <div v-else class="caption hint">{{ t("hint") }}</div>
  </div>
</template>

<style scoped>
.root {
  display: flex;
  flex-direction: column;
  gap: 10px;
  height: 100%;
  padding: 2px 16px 10px;
}
.head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
}
.segmented {
  display: inline-flex;
  gap: 2px;
  padding: 2px;
  border-radius: 9px;
  background: var(--fill-1);
}
.segmented button,
.chip {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  height: 26px;
  padding: 0 10px;
  border: 0;
  border-radius: 7px;
  background: transparent;
  color: var(--label-2);
  font: inherit;
  font-size: 12px;
  font-weight: 500;
  cursor: pointer;
  transition: background 120ms ease-out, color 120ms ease-out;
}
.segmented button.on {
  background: var(--fill-3);
  color: var(--label);
}
.chips {
  display: flex;
  gap: 4px;
}
.chip {
  border-radius: 999px;
  background: var(--fill-1);
}
.chip.on {
  background: var(--fill-3);
  color: var(--label);
}
.targets {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: 8px;
}
.target {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 3px;
  padding: 14px 8px 12px;
  border: 0;
  border-radius: 14px;
  background: var(--fill-1);
  color: var(--label);
  font: inherit;
  cursor: pointer;
  transition: background 120ms ease-out, scale 120ms ease-out;
}
.target:hover {
  background: var(--fill-2);
}
.target:active {
  scale: 0.98;
}
.target:disabled {
  opacity: 0.5;
  cursor: default;
}
.target .icon {
  position: relative;
  display: grid;
  place-items: center;
  width: 40px;
  height: 40px;
  margin-bottom: 4px;
  border-radius: 12px;
  background: var(--fill-2);
}
.target.video .icon {
  color: var(--red);
}
.badge-rec {
  position: absolute;
  right: -3px;
  bottom: -3px;
  color: var(--red);
}
.target .label {
  font-weight: 600;
}
.recording {
  display: flex;
  align-items: center;
  gap: 14px;
  padding: 18px 18px;
  border-radius: 16px;
  background: color-mix(in srgb, var(--red) 14%, transparent);
}
.recording .dot {
  width: 12px;
  height: 12px;
  border-radius: 50%;
  background: var(--red);
  box-shadow: 0 0 0 0 color-mix(in srgb, var(--red) 60%, transparent);
  animation: pulse 1.4s ease-out infinite;
}
@keyframes pulse {
  to {
    box-shadow: 0 0 0 10px transparent;
  }
}
.recording .grow {
  flex: 1;
  min-width: 0;
}
.recording .time {
  font-size: 26px;
  font-weight: 300;
  font-variant-numeric: tabular-nums;
  letter-spacing: -0.02em;
}
.btn.stop {
  height: 32px;
  background: var(--red);
  color: #fff;
}
.recents {
  display: grid;
  grid-template-columns: repeat(2, 1fr);
  gap: 4px;
  margin-top: auto;
}
.recent {
  display: flex;
  align-items: center;
  gap: 8px;
  min-width: 0;
  padding: 6px 10px;
  border: 0;
  border-radius: 9px;
  background: transparent;
  color: var(--label);
  font: inherit;
  font-size: 12px;
  text-align: left;
  cursor: pointer;
}
.recent:hover {
  background: var(--fill-1);
}
.recent .truncate {
  flex: 1;
  min-width: 0;
}
.hint {
  margin-top: auto;
  text-align: center;
}
.error {
  color: var(--red);
}
</style>
