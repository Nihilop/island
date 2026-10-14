<script setup lang="ts">
// Outil « Lecture » (520 × 232) : pochette + titre (bouton qui ramène l'app source),
// égaliseur, progression, contrôles, flèche du mixer.
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { PhArrowUpRight, PhCaretDown, PhMusicNotes, PhPause, PhPlay, PhSkipBack, PhSkipForward } from "@phosphor-icons/vue";
import { appLabel, fmtTime, next, np, openSource, position, prev, seek, togglePlay, trackKey } from "./store";
import { mixerOpen, toggleMixer } from "./mixer";
import Equalizer from "../../ui/Equalizer.vue";
import TrackBar from "../../ui/TrackBar.vue";
import Empty from "../../ui/Empty.vue";
import Mixer from "./Mixer.vue";
import { Button } from "@/sdk/ui/button";
import { cn } from "@/lib/utils";
import { t } from "../../notch/i18n";

// Horloge locale pour la position extrapolée (4 Hz suffisent pour m:ss).
const now = ref(0);
let timer: number | undefined;
onMounted(() => (timer = window.setInterval(() => (now.value = performance.now()), 250)));
onBeforeUnmount(() => window.clearInterval(timer));

const dragTarget = ref<number | null>(null);
const pos = computed(() => {
  void now.value;
  return dragTarget.value ?? position();
});
const ratio = computed(() => (np.duration > 0 ? pos.value / np.duration : 0));

function onDrag(v: number) {
  dragTarget.value = v * np.duration;
}
function onCommit(v: number) {
  dragTarget.value = null;
  seek(v * np.duration);
}
</script>

<template>
  <div class="flex h-full flex-col px-5 pt-1">
    <Empty v-if="!np.active" :icon="PhMusicNotes" :title="t('media.empty')" />
    <template v-else>
      <div class="flex items-center gap-3">
        <button
          type="button"
          class="group/source flex min-w-0 items-center gap-3"
          :title="t('media.openIn', { app: appLabel(np.appId) })"
          data-no-drag
          @click="openSource"
        >
          <div
            :class="
              cn(
                'relative size-14 shrink-0 overflow-hidden rounded-md transition-transform duration-240 ease-[cubic-bezier(0.34,1.4,0.64,1)]',
                !np.playing && 'scale-90',
              )
            "
          >
            <Transition
              enter-active-class="transition-opacity duration-450 ease-in-out"
              enter-from-class="opacity-0"
              leave-active-class="transition-opacity duration-350 ease-in-out delay-200"
              leave-to-class="opacity-0"
            >
              <img v-if="np.art" :key="np.art" :src="np.art" alt="" class="absolute inset-0 size-full object-cover" />
              <div v-else class="absolute inset-0 grid size-full place-items-center bg-fill-2 text-tint">
                <PhMusicNotes :size="22" weight="fill" />
              </div>
            </Transition>
            <div
              :class="
                cn(
                  'absolute inset-0 bg-scrim transition-opacity duration-250 ease-out group-hover/source:opacity-45',
                  np.playing ? 'opacity-0' : 'opacity-35',
                )
              "
            />
            <div
              class="absolute inset-0 grid place-items-center opacity-0 transition-opacity duration-160 ease-out group-hover/source:opacity-100"
            >
              <PhArrowUpRight :size="18" weight="bold" />
            </div>
          </div>
          <Transition
            enter-active-class="transition-[opacity,filter] duration-200 ease-out"
            enter-from-class="opacity-0 blur-[4px]"
            leave-active-class="transition-[opacity,filter] duration-200 ease-out"
            leave-to-class="opacity-0 blur-[4px]"
            mode="out-in"
          >
            <div :key="trackKey" class="min-w-0 text-left">
              <div class="type-title truncate font-semibold">{{ np.title }}</div>
              <div class="type-body truncate text-label-2">{{ np.artist }}</div>
            </div>
          </Transition>
        </button>
        <Equalizer :height="16" class="ml-auto shrink-0" />
      </div>

      <div class="mt-3.5 flex h-4 items-center gap-2">
        <template v-if="np.duration > 0">
          <span class="w-9 text-[11px] tabular-nums text-label-3">{{ fmtTime(pos) }}</span>
          <TrackBar :value="ratio" :interactive="np.canSeek" class="flex-1" @drag="onDrag" @commit="onCommit" />
          <span class="w-9 text-right text-[11px] tabular-nums text-label-3">−{{ fmtTime(np.duration - pos) }}</span>
        </template>
      </div>

      <div class="mt-1.5 grid grid-cols-[1fr_auto_1fr] items-center">
        <span />
        <div class="flex items-center gap-6">
          <Button variant="ghost" size="icon-lg" class="text-label hover:text-label" data-no-drag @click="prev">
            <PhSkipBack :size="20" weight="fill" class="size-5" />
          </Button>
          <Button variant="ghost" size="icon-lg" class="text-label hover:text-label" data-no-drag @click="togglePlay">
            <Transition
              enter-active-class="transition-[opacity,scale,filter] duration-160 ease-out"
              enter-from-class="scale-[0.6] opacity-0 blur-[3px]"
              leave-active-class="transition-[opacity,scale,filter] duration-160 ease-out"
              leave-to-class="scale-[0.6] opacity-0 blur-[3px]"
              mode="out-in"
            >
              <PhPause v-if="np.playing" key="pause" :size="26" weight="fill" class="size-[26px]" />
              <PhPlay v-else key="play" :size="26" weight="fill" class="size-[26px]" />
            </Transition>
          </Button>
          <Button variant="ghost" size="icon-lg" class="text-label hover:text-label" data-no-drag @click="next">
            <PhSkipForward :size="20" weight="fill" class="size-5" />
          </Button>
        </div>
        <Button
          variant="ghost"
          size="icon"
          :class="
            cn(
              'justify-self-end transition-[rotate,scale,color] duration-240 hover:bg-transparent',
              mixerOpen ? 'rotate-180 text-label hover:text-label' : 'hover:text-label-3',
            )
          "
          data-no-drag
          @click="toggleMixer"
        >
          <PhCaretDown :size="14" weight="bold" />
        </Button>
      </div>
    </template>
    <Mixer v-if="mixerOpen" />
  </div>
</template>
