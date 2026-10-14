<script setup lang="ts">
// Réglages › Affichage : visibilité, écran, position, style fermé, thème, mode focus.
import { computed, onMounted, ref } from "vue";
import { settings } from "../../notch/settings";
import { display, listMonitors, recenter, setDisplay, type MonitorInfo } from "../../notch/display";
import { t } from "../../notch/i18n";
import Group from "../../ui/Group.vue";
import Row from "../../ui/Row.vue";
import Segmented from "../../ui/Segmented.vue";
import Select from "../../ui/Select.vue";
import { Switch } from "@/sdk/ui/switch";
import { Button } from "@/sdk/ui/button";
import { autoActive, presentation, setPresentation } from "../../notch/presentation";
import { fps, unlockFps } from "../system/gaming";

const unlocking = ref(false);
const unlockResult = ref<string | null>(null);
async function unlock() {
  unlocking.value = true;
  unlockResult.value = await unlockFps();
  unlocking.value = false;
}
const unlockHelp = computed(() => {
  if (fps.status === "relogin" || unlockResult.value === "relogin") return t("settings.fps.relogin");
  if (unlockResult.value === "cancelled") return t("settings.fps.cancelled");
  if (unlockResult.value === "error") return t("settings.fps.error");
  if (unlockResult.value === "ok") return t("settings.fps.ok");
  return t("settings.fps.help");
});

const monitors = ref<MonitorInfo[]>([]);
onMounted(async () => (monitors.value = await listMonitors()));

const monitorOptions = computed(() =>
  monitors.value.map((m, i) => ({
    value: m.primary ? null : m.name,
    label: `${m.primary ? t("settings.monitor.primary") : t("settings.monitor.n", { n: i + 1 })} · ${Math.round(m.width)}×${Math.round(m.height)}`,
  })),
);

const monitorModel = computed({
  get: () => display.monitor,
  set: (v: string | null) => setDisplay({ monitor: v, offset: 0 }),
});
import { testBanner, testPending } from "../notifications/store";
</script>

<template>
  <div>
    <Group :title="t('settings.page.display')" section="display">
      <Row :label="t('settings.visibility')">
        <Segmented
          v-model="settings.visibility"
          size="sm"
          :options="[
            { value: 'always', label: t('settings.visibility.always') },
            { value: 'hideFullscreen', label: t('settings.visibility.hideFullscreen') },
          ]"
        />
      </Row>
      <Row v-if="monitors.length > 1" :label="t('settings.monitor')">
        <Select v-model="monitorModel" :options="monitorOptions" />
      </Row>
      <Row :label="t('settings.position')" :help="display.offset === 0 ? t('settings.position.centered') : t('settings.position.help')">
        <Button v-if="display.offset !== 0" @click="recenter">{{ t("notch.settings.recenter") }}</Button>
      </Row>
      <Row :label="t('settings.idleStyle')" :help="t('settings.idleStyle.help')">
        <Segmented
          v-model="settings.idleStyle"
          size="sm"
          :options="[
            { value: 'notch', label: t('settings.style.notch') },
            { value: 'line', label: t('settings.style.line') },
          ]"
        />
      </Row>
      <Row :label="t('settings.notifStyle')" :help="t('settings.notifStyle.help')">
        <Button size="sm" :disabled="testPending" @click="testBanner">{{ t("settings.notifStyle.test") }}</Button>
        <Segmented
          v-model="settings.notifStyle"
          size="sm"
          :options="[
            { value: 'minimal', label: t('settings.notifStyle.minimal') },
            { value: 'full', label: t('settings.notifStyle.full') },
          ]"
        />
      </Row>
      <Row :label="t('settings.theme')" :help="t('settings.theme.help')">
        <Segmented
          v-model="settings.theme"
          size="sm"
          :options="[
            { value: 'dark', label: t('settings.theme.dark') },
            { value: 'light', label: t('settings.theme.light') },
            { value: 'system', label: t('settings.theme.system') },
          ]"
        />
      </Row>
    </Group>

    <Group :title="t('settings.gaming')" section="gaming">
      <Row :label="t('settings.gaming')" :help="t('settings.gaming.help')">
        <Segmented
          v-model="settings.gaming"
          size="sm"
          :options="[
            { value: 'off', label: t('settings.gaming.off') },
            { value: 'fullscreen', label: t('settings.gaming.fullscreen') },
            { value: 'always', label: t('settings.gaming.always') },
          ]"
        />
      </Row>
      <Row
        v-if="settings.gaming !== 'off' && (fps.status === 'noAdmin' || fps.status === 'relogin' || unlockResult)"
        :label="t('settings.fps.unlock')"
        :help="unlockHelp"
      >
        <Button v-if="fps.status === 'noAdmin'" :loading="unlocking" @click="unlock">{{ t("settings.fps.unlockAction") }}</Button>
      </Row>
    </Group>

    <Group :title="t('settings.privacy')" section="presentation">
      <Row :label="t('settings.privacyDots')" :help="t('settings.privacyDots.help')">
        <Switch v-model="settings.privacyDots" />
      </Row>
      <Row :label="t('settings.presentation')" :help="autoActive ? t('settings.presentation.autoOn') : t('settings.presentation.help')">
        <Switch :model-value="presentation" @update:model-value="setPresentation" />
      </Row>
      <Row :label="t('settings.presentationAuto')" :help="t('settings.presentationAuto.help')">
        <Switch v-model="settings.presentationAuto" />
      </Row>
    </Group>

    <Group :title="t('notch.focusMode')" :note="t('settings.focus.note')">
      <Row :label="t('notch.focusMode')" :help="t('settings.focus.help')">
        <Switch v-model="settings.focusMode" />
      </Row>
      <Row :label="t('settings.focusStyle')">
        <Segmented
          v-model="settings.focusStyle"
          size="sm"
          :options="[
            { value: 'dim', label: t('settings.focusStyle.dim') },
            { value: 'line', label: t('settings.style.line') },
          ]"
        />
      </Row>
    </Group>
  </div>
</template>
