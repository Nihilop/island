<script setup lang="ts">
// Réglages › Minuteur : son d'alarme (aperçu au changement), volume, durée.
import { onBeforeUnmount, ref, watch } from "vue";
import { PhPlay, PhStop } from "@phosphor-icons/vue";
import Group from "../../ui/Group.vue";
import Row from "../../ui/Row.vue";
import Select from "../../ui/Select.vue";
import Segmented from "../../ui/Segmented.vue";
import TrackBar from "../../ui/TrackBar.vue";
import { Button } from "@/sdk/ui/button";
import { settings } from "../../notch/settings";
import { SOUNDS, playSound, stopSound, type SoundId } from "../../design/sound";
import { t } from "../../notch/i18n";

const previewing = ref(false);
let resetTimer: number | undefined;

function preview() {
  if (previewing.value) {
    stopSound();
    previewing.value = false;
    return;
  }
  playSound(settings.alarm.sound, settings.alarm.volume, 0);
  previewing.value = true;
  window.clearTimeout(resetTimer);
  resetTimer = window.setTimeout(() => (previewing.value = false), 2500);
}

// Changer de son le joue.
watch(
  () => settings.alarm.sound,
  () => {
    previewing.value = false;
    preview();
  },
);

onBeforeUnmount(() => {
  stopSound();
  window.clearTimeout(resetTimer);
});

const volumeStep = (v: number) => Math.round(Math.min(1, Math.max(0.05, v)) * 20) / 20;
</script>

<template>
  <div>
    <Group :title="t('settings.page.timer')" section="alarm">
      <Row :label="t('timer.sound')">
        <Button size="icon-sm" class="text-label-2" @click="preview">
          <PhStop v-if="previewing" :size="12" weight="fill" />
          <PhPlay v-else :size="12" weight="fill" />
        </Button>
        <Select
          v-model="settings.alarm.sound"
          :options="SOUNDS.map((s: SoundId) => ({ value: s, label: t(`timer.sound.${s}`) }))"
        />
      </Row>
      <Row :label="t('timer.volume')">
        <TrackBar
          class="w-[160px]"
          :value="settings.alarm.volume"
          follow
          :wheel-step="0.05"
          :key-step="0.05"
          @drag="settings.alarm.volume = volumeStep($event)"
          @commit="settings.alarm.volume = volumeStep($event)"
        />
        <span class="type-caption w-9 text-right text-label-2 tabular-nums">{{ Math.round(settings.alarm.volume * 100) }}%</span>
      </Row>
      <Row :label="t('timer.duration')">
        <Segmented
          v-model="settings.alarm.duration"
          size="sm"
          :options="[
            { value: 0, label: '1×' },
            { value: 5, label: '5 s' },
            { value: 15, label: '15 s' },
            { value: 30, label: '30 s' },
            { value: -1, label: t('timer.untilStop') },
          ]"
        />
      </Row>
    </Group>
  </div>
</template>
