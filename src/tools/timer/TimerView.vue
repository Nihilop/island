<script setup lang="ts">
// Outil « Minuteur » (520 × 192) : Focus/Pause, puces de durée (+ perso), temps restant
// 40 px teinté (orange focus, vert pause), barre de progression, Réinitialiser, Démarrer.
// Molette sur le temps (non démarré) : ± 1 min, Maj ± 5 min.
import { useDraft } from "../../stores/drafts";
import { computed, nextTick, ref } from "vue";
import { PhAlarm, PhArrowCounterClockwise, PhCheck, PhPause, PhPencilSimple, PhPlay } from "@phosphor-icons/vue";
import Segmented from "../../ui/Segmented.vue";
import { Button } from "@/sdk/ui/button";
import { Input } from "@/sdk/ui/input";
import {
  PRESETS,
  dismiss,
  fmt,
  left,
  pristine,
  progress,
  reset,
  ringing,
  running,
  setMinutes,
  setMode,
  stopAlarm,
  timer,
  toggle,
  type TimerMode,
} from "./store";
import { takeKeyboard } from "../../notch/state";
import { t } from "../../notch/i18n";

const tint = computed(() => (timer.mode === "focus" && !timer.finished ? "var(--sys-orange)" : "var(--sys-green)"));
const minutes = computed(() => Math.round(timer.total / 60));
const chips = computed(() => {
  const list = [...PRESETS[timer.mode]];
  const c = timer.custom[timer.mode];
  if (c && !list.includes(c)) list.push(c);
  return list;
});

const modeModel = computed({
  get: () => timer.mode,
  set: (m: TimerMode) => setMode(m),
});

// Saisie d'une durée perso.
const editing = ref(false);
const draft = useDraft("timer.input", () => "");
const input = ref<{ $el: HTMLInputElement }>();
async function edit() {
  draft.value = String(minutes.value);
  editing.value = true;
  takeKeyboard();
  await nextTick();
  input.value?.$el?.select();
}
function commit() {
  const n = parseInt(draft.value, 10);
  if (n > 0) setMinutes(n);
  editing.value = false;
}
function onKey(e: KeyboardEvent) {
  if (e.key === "Enter") commit();
  else if (e.key === "Escape") {
    e.stopPropagation(); // Échap annule sans fermer le notch
    editing.value = false;
  }
}

function wheel(e: WheelEvent) {
  if (!pristine.value) return;
  e.preventDefault();
  const step = e.shiftKey ? 5 : 1;
  setMinutes(minutes.value + (e.deltaY < 0 ? step : -step));
}
</script>

<template>
  <div class="flex h-full flex-col justify-between px-5 pb-3.5 pt-1" :style="{ '--t': tint }">
    <template v-if="!timer.finished">
      <div class="flex items-center justify-between">
        <Segmented
          v-model="modeModel"
          size="sm"
          :options="[
            { value: 'focus', label: t('timer.focus') },
            { value: 'pause', label: t('timer.pause') },
          ]"
        />
        <div class="flex items-center gap-1">
          <template v-if="!editing">
            <Button
              v-for="m in chips"
              :key="m"
              variant="ghost"
              size="sm"
              class="type-caption min-w-8 px-2 tabular-nums text-label-2 transition-colors duration-160 ease-(--ease-out) active:scale-100 hover:text-label data-[active=true]:bg-fill-2"
              :data-active="m === minutes || undefined"
              @click="setMinutes(m)"
            >
              {{ m }}
            </Button>
            <Button
              variant="ghost"
              size="sm"
              class="min-w-8 px-2 text-label-2 transition-colors duration-160 ease-(--ease-out) active:scale-100 hover:text-label"
              :title="t('timer.custom')"
              @click="edit"
            >
              <PhPencilSimple :size="12" weight="bold" />
            </Button>
          </template>
          <div v-else class="flex h-6 items-center gap-1.5 rounded-full bg-fill-2 pl-2.5 pr-1" data-no-drag>
            <Input
              ref="input"
              v-model="draft"
              class="type-caption h-auto w-7 rounded-none bg-transparent p-0 text-right text-[11px] leading-[14px] tabular-nums text-label focus-visible:ring-0"
              inputmode="numeric"
              maxlength="3"
              @keydown="onKey"
              @blur="editing = false"
              @input="draft = draft.replace(/\D/g, '')"
            />
            <span class="type-caption text-label-3">min</span>
            <Button
              variant="default"
              size="icon-xs"
              class="size-[18px] bg-(--t) text-black hover:bg-(--t) active:scale-100"
              @mousedown.prevent="commit"
            >
              <PhCheck :size="12" weight="bold" />
            </Button>
          </div>
        </div>
      </div>

      <div class="flex items-center gap-3.5">
        <div
          class="min-w-28 text-[40px] font-semibold leading-[44px] tracking-[-0.02em] tabular-nums text-(--t)"
          @wheel="wheel"
        >
          {{ fmt(left) }}
        </div>
        <div class="h-[5px] flex-1 overflow-hidden rounded-full bg-fill-2">
          <div class="h-full origin-left bg-(--t) transition-transform duration-250 ease-linear" :style="{ transform: `scaleX(${progress})` }" />
        </div>
        <Button
          size="icon-lg"
          class="text-label-2 transition-[transform,opacity] duration-[140ms,160ms] ease-(--ease-out) hover:bg-fill-2"
          :disabled="pristine"
          :title="t('timer.reset')"
          @click="reset"
        >
          <PhArrowCounterClockwise :size="18" weight="bold" class="size-[18px]" />
        </Button>
        <Button
          size="icon-lg"
          class="bg-[color-mix(in_srgb,var(--t)_18%,transparent)] text-(--t) transition-[transform,opacity] duration-[140ms,160ms] ease-(--ease-out) hover:bg-[color-mix(in_srgb,var(--t)_18%,transparent)]"
          :title="running ? t('timer.pauseAction') : t('timer.start')"
          @click="toggle"
        >
          <PhPause v-if="running" :size="18" weight="fill" class="size-[18px]" />
          <PhPlay v-else :size="18" weight="fill" class="size-[18px]" />
        </Button>
      </div>
    </template>

    <template v-else>
      <div class="type-footnote flex items-center gap-1.5 font-medium text-sys-green">
        <PhAlarm v-if="ringing" :size="14" weight="fill" class="animate-throb" />
        <PhCheck v-else :size="14" weight="bold" />
        {{ timer.mode === "focus" ? t("timer.focusDone") : t("timer.pauseDone") }} · {{ minutes }} min
      </div>
      <div class="flex items-center gap-3.5">
        <div
          class="min-w-28 text-[40px] font-semibold leading-[44px] tracking-[-0.02em] tabular-nums text-(--t)"
          :class="{ 'animate-throb': ringing }"
        >
          0:00
        </div>
        <div class="h-[5px] flex-1 overflow-hidden rounded-full bg-fill-2">
          <div class="h-full origin-left scale-x-100 bg-(--t) transition-transform duration-250 ease-linear" />
        </div>
        <Button v-if="ringing" variant="default" @click="stopAlarm">{{ t("timer.stopAlarm") }}</Button>
        <Button v-else @click="dismiss">{{ t("timer.close") }}</Button>
      </div>
    </template>
  </div>
</template>
