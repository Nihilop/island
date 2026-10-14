<script setup lang="ts">
// Lignes du mixer (34 px) : bouton muet = icône du programme (grisée + badge haut-parleur
// barré si muet), nom (medium si c'est l'app qui joue), barre de niveau, pourcentage. Les
// lignes glissent à leur nouvelle place quand l'app source change ; apparition échelonnée.
import { PhSpeakerHigh, PhSpeakerSlash, PhAppWindow } from "@phosphor-icons/vue";
import { mixerDragging, rows, setLevel, toggleMute } from "./mixer";
import { hold } from "../../notch/state";
import TrackBar from "../../ui/TrackBar.vue";
import { t } from "../../notch/i18n";
import { Button } from "@/sdk/ui/button";
import { cn } from "@/lib/utils";

let release: (() => void) | null = null;
function onDrag(exe: string | null, v: number) {
  if (!mixerDragging.value) {
    mixerDragging.value = true;
    release = hold();
  }
  void setLevel(exe, v);
}
function onCommit(exe: string | null, v: number) {
  void setLevel(exe, v);
  mixerDragging.value = false;
  release?.();
  release = null;
}
</script>

<template>
  <div class="mt-2.5 max-h-[170px] overflow-y-auto [scrollbar-color:var(--fill-3)_transparent] [scrollbar-width:thin]">
    <div v-if="rows.length <= 1" class="type-caption py-2 text-center text-label-3">{{ t("media.mixer.empty") }}</div>
    <TransitionGroup
      tag="div"
      appear
      enter-active-class="transition-[opacity,translate] duration-240 ease-out delay-(--delay)"
      enter-from-class="-translate-y-1 opacity-0"
      leave-active-class="hidden"
      move-class="transition-transform duration-360 ease-out"
    >
      <div
        v-for="(r, i) in rows"
        :key="r.kind === 'system' ? '__system' : r.app.exe"
        class="flex h-[34px] items-center gap-2.5"
        :style="{ '--delay': `${80 + i * 40}ms` }"
      >
        <Button
          variant="ghost"
          size="icon-sm"
          class="relative rounded-none text-label-2 hover:bg-transparent hover:text-label-2"
          data-no-drag
          @click="toggleMute(r.kind === 'system' ? null : r.app.exe)"
        >
          <template v-if="r.kind === 'system'">
            <PhSpeakerSlash v-if="r.app.muted" :size="16" weight="fill" class="size-4 text-sys-red" />
            <PhSpeakerHigh v-else :size="16" weight="fill" class="size-4" />
          </template>
          <template v-else>
            <img
              v-if="r.app.icon"
              :src="r.app.icon"
              alt=""
              :class="cn('size-[18px] transition-[opacity,filter] duration-160 ease-out', r.app.muted && 'opacity-40 grayscale')"
            />
            <PhAppWindow
              v-else
              :size="18"
              weight="duotone"
              :class="cn('size-[18px] transition-[opacity,filter] duration-160 ease-out', r.app.muted && 'opacity-40 grayscale')"
            />
            <span
              v-if="r.app.muted"
              class="absolute -right-0.5 -bottom-0.5 grid size-3.5 place-items-center rounded-full bg-sys-red text-white"
            >
              <PhSpeakerSlash :size="9" weight="fill" class="size-[9px]" />
            </span>
          </template>
        </Button>
        <span :class="cn('type-footnote w-32 truncate text-label-2', r.kind === 'app' && r.playing && 'font-medium text-label')">
          {{ r.kind === "system" ? t("media.mixer.system") : r.app.name }}
        </span>
        <TrackBar
          class="flex-1"
          :value="r.app.muted ? 0 : r.app.level"
          follow
          :wheel-step="0.02"
          :key-step="0.05"
          @drag="onDrag(r.kind === 'system' ? null : r.app.exe, $event)"
          @commit="onCommit(r.kind === 'system' ? null : r.app.exe, $event)"
        />
        <span class="w-[34px] text-right text-[11px] tabular-nums text-label-3">{{ Math.round((r.app.muted ? 0 : r.app.level) * 100) }}%</span>
      </div>
    </TransitionGroup>
  </div>
</template>
