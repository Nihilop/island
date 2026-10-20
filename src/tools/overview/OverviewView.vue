<script setup lang="ts">
// Outil « Vue d'ensemble » (520 × 252) : heure + date, météo, activité en cours (minuteur
// > musique), tuile mode focus du notch, tuile volume principal.
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import {
  PhCheck,
  PhMoon,
  PhMusicNotes,
  PhSpeakerHigh,
  PhSpeakerLow,
  PhSpeakerNone,
  PhSpeakerSlash,
  PhTimer,
} from "@phosphor-icons/vue";
import Equalizer from "../../ui/Equalizer.vue";
import IconButton from "../../ui/IconButton.vue";
import { PhDiscordLogo, PhEye, PhEyeSlash, PhGameController, PhStorefront } from "@phosphor-icons/vue";
import { openStore } from "../../store/open";
import { duration as playDuration, playing } from "../activity/store";
import { presentation, setPresentation } from "../../notch/presentation";
import TrackBar from "../../ui/TrackBar.vue";
import { Button } from "@/sdk/ui/button";
import { Skeleton } from "@/sdk/ui/skeleton";
import { cn } from "@/lib/utils";
import { selectTool, setFocusMode } from "../../notch/state";
import { settings } from "../../notch/settings";
import { np } from "../media/store";
import { discord, inCall, someoneSpeaking } from "../discord/store";
import { fmt, left, ringing, running, timer } from "../timer/store";
import { place, weather, wmo } from "../weather/store";
import { locale } from "../../sdk/i18n";
import { t } from "../../notch/i18n";

// --- Heure (chaque seconde) --------------------------------------------------------
const now = ref(new Date());
let clock: number | undefined;
const time = computed(() => {
  const d = now.value;
  if (settings.clock24) return { hm: `${d.getHours()}:${String(d.getMinutes()).padStart(2, "0")}`, ampm: "" };
  const h = d.getHours() % 12 || 12;
  return { hm: `${h}:${String(d.getMinutes()).padStart(2, "0")}`, ampm: d.getHours() < 12 ? "AM" : "PM" };
});
const date = computed(() =>
  new Intl.DateTimeFormat(locale.value, { weekday: "long", day: "numeric", month: "long" }).format(now.value),
);

// --- Activité : minuteur > musique ------------------------------------------------
const activity = computed<"timer" | "discord" | "game" | "media" | null>(() => {
  if (running.value || timer.finished) return "timer";
  if (inCall.value) return "discord";
  if (playing.value) return "game";
  if (np.active) return "media";
  return null;
});

// --- Mode focus du notch (même chemin que la cible du header) ------------------------
// Le focus Windows (`FocusSessionManager`) n'est pas pilotable : `TryStartFocusSession`
// est une « Limited Access Feature » (jeton Microsoft + identité de package) → accès
// refusé pour une app non packagée. La tuile pilote donc le mode focus du notch.
const focus = computed(() => settings.focusMode);
function toggleFocus() {
  setFocusMode(!settings.focusMode);
}

// --- Volume (repoll toutes les 1 s, sauf pendant le drag) ---------------------------
const volume = ref<{ level: number; muted: boolean } | null>(null);
const dragging = ref(false);
async function readVolume() {
  if (dragging.value) return;
  volume.value = await invoke<{ level: number; muted: boolean } | null>("master_volume").catch(() => null);
}
function setVolume(level: number) {
  if (!volume.value) return;
  volume.value.level = level;
  if (level > 0) volume.value.muted = false;
  invoke("mixer_set", { exe: null, level }).catch(() => {});
}
function toggleMute() {
  if (!volume.value) return;
  volume.value.muted = !volume.value.muted;
  invoke("mixer_set", { exe: null, muted: volume.value.muted }).catch(() => {});
}
const speaker = computed(() => {
  const v = volume.value;
  if (!v || v.muted || v.level === 0) return PhSpeakerSlash;
  return v.level < 0.34 ? PhSpeakerNone : v.level < 0.67 ? PhSpeakerLow : PhSpeakerHigh;
});

let polls: number[] = [];
onMounted(() => {
  clock = window.setInterval(() => (now.value = new Date()), 1000);
  void readVolume();
  polls = [window.setInterval(readVolume, 1000)];
});
onBeforeUnmount(() => {
  window.clearInterval(clock);
  polls.forEach((p) => window.clearInterval(p));
});

const sky = computed(() => (weather.current ? wmo(weather.current.code, weather.current.day) : null));
</script>

<template>
  <div class="flex h-full flex-col gap-2.5 px-4 pb-1">
    <Teleport defer to="#notch-header-actions">
      <IconButton :icon="PhStorefront" :label="t('store.open')" @click="openStore()" />
      <IconButton
        :icon="presentation ? PhEyeSlash : PhEye"
        :active="presentation"
        :label="t('settings.presentation')"
        @click="setPresentation(!presentation)"
      />
    </Teleport>
    <div class="flex items-start justify-between">
      <div>
        <div class="text-[34px] leading-[38px] font-semibold tracking-[-0.03em] tabular-nums">
          {{ time.hm }}<span v-if="time.ampm" class="ml-1 text-[15px] text-label-3">{{ time.ampm }}</span>
        </div>
        <div class="type-footnote capitalize text-label-2">{{ date }}</div>
      </div>
      <button type="button" class="flex flex-col items-end gap-0.5 rounded-[10px] px-1.5 py-1 hover:bg-fill-1" @click="selectTool('weather')">
        <template v-if="weather.current && sky">
          <div class="flex items-center gap-2">
            <component :is="sky.icon" :size="22" weight="duotone" />
            <span class="text-[22px] font-semibold tabular-nums">{{ Math.round(weather.current.temp) }}°</span>
          </div>
          <div class="type-caption max-w-[220px] truncate text-label-3">{{ place.location?.name }} · {{ sky.label }}</div>
        </template>
        <div v-else-if="weather.locating" class="type-caption text-label-3">{{ t("weather.locating") }}</div>
        <Skeleton v-else class="h-8 w-24 animate-[pulse_1.4s_ease-in-out_infinite]" />
      </button>
    </div>

    <button type="button" class="flex h-11 items-center gap-2.5 rounded-md bg-fill-1 px-2.5" @click="activity && selectTool(activity === 'game' ? 'activity' : activity)">
      <template v-if="activity === 'timer'">
        <span :class="cn('grid size-7 place-items-center', timer.mode === 'focus' && !timer.finished ? 'text-sys-orange' : 'text-sys-green')">
          <PhCheck v-if="timer.finished" :size="16" weight="bold" />
          <PhTimer v-else :size="16" weight="fill" />
        </span>
        <div class="min-w-0 flex-1 text-left">
          <div class="type-body truncate">
            {{ timer.finished ? (timer.mode === "focus" ? t("timer.focusDone") : t("timer.pauseDone")) : timer.mode === "focus" ? t("timer.focus") : t("timer.pause") }}
          </div>
          <div class="type-caption text-label-3">
            {{ timer.finished ? t("overview.openToClose") : t("overview.left", { t: fmt(left) }) }}
          </div>
        </div>
        <span v-if="ringing" class="type-caption text-sys-green">●</span>
      </template>
      <template v-else-if="activity === 'discord' && discord.call">
        <img v-if="discord.call.guildIcon" :src="discord.call.guildIcon" alt="" class="size-7 rounded-[6px] object-cover" />
        <span v-else class="grid size-7 place-items-center text-label-2"><PhDiscordLogo :size="16" weight="fill" /></span>
        <div class="min-w-0 flex-1 text-left">
          <div class="type-body truncate">{{ discord.call.channel }}</div>
          <div class="type-caption truncate text-label-3">
            {{ discord.call.guild ?? t("discord.private") }} · {{ t("discord.people", { n: discord.call.members.length }) }}
          </div>
        </div>
        <Equalizer v-if="someoneSpeaking" color="var(--sys-green)" />
      </template>
      <template v-else-if="activity === 'game' && playing">
        <img v-if="playing.art" :src="playing.art" alt="" class="h-7 w-[60px] rounded-[6px] object-cover" />
        <span v-else class="grid size-7 place-items-center text-sys-green"><PhGameController :size="16" weight="fill" /></span>
        <div class="min-w-0 flex-1 text-left">
          <div class="type-body truncate">{{ playing.name }}</div>
          <div class="type-caption truncate text-label-3">{{ t("activity.since", { d: playDuration(playing.elapsed) }) }}</div>
        </div>
      </template>
      <template v-else-if="activity === 'media'">
        <img v-if="np.art" :src="np.art" alt="" class="size-7 rounded-[6px] object-cover" />
        <span v-else class="grid size-7 place-items-center text-tint"><PhMusicNotes :size="16" weight="fill" /></span>
        <div class="min-w-0 flex-1 text-left">
          <div class="type-body truncate">{{ np.title }}</div>
          <div class="type-caption truncate text-label-3">{{ np.artist }}</div>
        </div>
        <Equalizer />
      </template>
      <div v-else class="type-body w-full text-center text-label-3">{{ t("overview.nothing") }}</div>
    </button>

    <div class="flex gap-2">
      <button
        type="button"
        :class="
          cn(
            'flex h-10 w-[150px] items-center gap-2 rounded-md px-2 transition-[background-color,color] duration-200 ease-out',
            focus ? 'bg-label text-notch' : 'bg-fill-1',
          )
        "
        @click="toggleFocus"
      >
        <span
          :class="
            cn(
              'grid size-[26px] place-items-center rounded-full transition-[rotate] duration-300 ease-out',
              focus ? 'rotate-0 bg-notch/10' : '-rotate-30 bg-fill-2',
            )
          "
        >
          <PhMoon :size="14" weight="fill" />
        </span>
        <div class="text-left">
          <div class="type-footnote font-medium">{{ t("overview.focus") }}</div>
          <div class="type-caption opacity-70">{{ focus ? t("overview.on") : t("overview.off") }}</div>
        </div>
      </button>
      <div :class="cn('flex h-10 flex-1 items-center gap-2 rounded-md bg-fill-1 pr-2.5 pl-1.5', !volume && 'pointer-events-none opacity-40')">
        <Button
          variant="ghost"
          size="icon"
          :class="cn('rounded-none hover:bg-transparent', volume?.muted ? 'text-sys-red hover:text-sys-red' : 'text-label-2 hover:text-label-2')"
          @click="toggleMute"
        >
          <component :is="speaker" :size="16" weight="fill" class="size-4" />
        </Button>
        <TrackBar
          size="thick"
          class="flex-1"
          :value="volume && !volume.muted ? volume.level : 0"
          follow
          :wheel-step="0.02"
          :key-step="0.05"
          @drag="dragging = true; setVolume($event)"
          @commit="setVolume($event); dragging = false"
        />
        <span class="type-caption w-9 tabular-nums text-right text-label-2">{{ volume ? Math.round((volume.muted ? 0 : volume.level) * 100) : "–" }}%</span>
      </div>
    </div>
  </div>
</template>
