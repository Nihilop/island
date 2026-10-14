<script setup lang="ts">
// Réglages › Général : langue, horloge, ouverture, démarrage, raccourci, mises à jour.
import { computed, ref } from "vue";
import { useAppStore } from "../../stores/appStore";
import { storeToRefs } from "pinia";
import { settings } from "../../notch/settings";
import { shortcutOk } from "../../notch/state";
import { syncTray } from "../../notch/tray";
import { t } from "../../notch/i18n";
import { applyUpdate, checkForUpdate, updateState } from "../../composables/updater";
import { changelog, currentVersion, freshVersions, markSeen, unseen } from "../../composables/changelog";
import { locale } from "../../sdk/i18n";
import Group from "../../ui/Group.vue";
import Row from "../../ui/Row.vue";
import Segmented from "../../ui/Segmented.vue";
import { Switch } from "@/sdk/ui/switch";
import { Button } from "@/sdk/ui/button";
import ShortcutField from "../../ui/ShortcutField.vue";
import { place, searching } from "../weather/store";
import { selectTool } from "../../notch/state";

function openWeatherSearch() {
  searching.value = true;
  selectTool("weather");
}

const app = useAppStore();
const { lang, autostart } = storeToRefs(app);

const langModel = computed({
  get: () => lang.value,
  set: (v: string) => {
    lang.value = v;
    app.onLang();
  },
});

const autostartModel = computed({
  get: () => autostart.value,
  set: async (v: boolean) => {
    autostart.value = v;
    await app.onAutostart();
    syncTray();
  },
});

const statusText = computed(() => {
  switch (updateState.status) {
    case "checking":
      return t("updates.checking");
    case "current":
      return t("updates.current");
    case "available":
      return t("updates.available", { v: updateState.version });
    case "downloading":
      return t("updates.downloading");
    case "error":
      return t("updates.error");
    default:
      return "";
  }
});

const showLog = ref(false);
function toggleLog() {
  showLog.value = !showLog.value;
  if (showLog.value) markSeen();
}
const items = (e: (typeof changelog)[number]) => e.items[locale.value] ?? e.items.en ?? [];
</script>

<template>
  <div>
    <Group :title="t('settings.page.general')" section="general">
      <Row :label="t('settings.lang')">
        <Segmented
          v-model="langModel"
          size="sm"
          :options="[
            { value: 'fr', label: 'Français' },
            { value: 'en', label: 'English' },
          ]"
        />
      </Row>
      <Row :label="t('settings.clock')">
        <Segmented
          v-model="settings.clock24"
          size="sm"
          :options="[
            { value: true, label: '24 h' },
            { value: false, label: '12 h' },
          ]"
        />
      </Row>
      <Row :label="t('settings.openMode')">
        <Segmented
          v-model="settings.openMode"
          size="sm"
          :options="[
            { value: 'hover', label: t('settings.openMode.hover') },
            { value: 'click', label: t('settings.openMode.click') },
          ]"
        />
      </Row>
      <Row v-if="settings.openMode === 'hover'" :label="t('settings.hoverDelay')">
        <Segmented
          v-model="settings.hoverDelay"
          size="sm"
          :options="[
            { value: 'fast', label: t('settings.hoverDelay.fast') },
            { value: 'normal', label: t('settings.hoverDelay.normal') },
            { value: 'patient', label: t('settings.hoverDelay.patient') },
          ]"
        />
      </Row>
      <Row :label="t('tray.autostart')">
        <Switch v-model="autostartModel" />
      </Row>
      <Row :label="t('settings.shortcut')" :help="shortcutOk ? t('settings.shortcut.help') : t('settings.shortcut.taken')">
        <ShortcutField v-model="settings.shortcut" />
      </Row>
    </Group>

    <Group :title="t('tool.weather')" section="weather">
      <Row :label="t('weather.location')" :help="place.location ? `${place.location.name}${place.auto ? ' · ' + t('weather.auto') : ''}` : t('weather.noLocation')">
        <Button @click="openWeatherSearch">{{ t("weather.change") }}</Button>
      </Row>
      <Row :label="t('weather.unit')">
        <Segmented
          v-model="settings.tempUnit"
          size="sm"
          :options="[
            { value: 'c', label: '°C' },
            { value: 'f', label: '°F' },
          ]"
        />
      </Row>
    </Group>

    <Group :title="t('settings.updates')" section="updates">
      <Row :label="t('settings.version', { v: currentVersion })" :help="statusText">
        <Button :disabled="updateState.status === 'checking' || updateState.status === 'downloading'" @click="checkForUpdate(false)">
          {{ t("updates.check") }}
        </Button>
      </Row>
      <div
        v-if="updateState.status === 'available' || updateState.status === 'downloading'"
        class="border-t border-separator px-3 py-2 [&+*]:border-t [&+*]:border-separator"
      >
        <div class="flex items-center justify-between gap-4">
          <div class="type-body">{{ t("updates.available", { v: updateState.version }) }}</div>
          <div v-if="updateState.status === 'downloading'" class="h-1.5 w-[112px] overflow-hidden rounded-full bg-fill-2">
            <div
              class="h-full origin-left bg-label-2 transition-transform duration-200 ease-out"
              :style="{ transform: `scaleX(${updateState.progress})` }"
            />
          </div>
          <Button v-else variant="default" @click="applyUpdate">{{ t("updates.install") }}</Button>
        </div>
        <p v-if="updateState.notes" class="type-caption mt-1 whitespace-pre-wrap text-label-3">{{ updateState.notes }}</p>
      </div>
      <button type="button" class="flex min-h-9 w-full items-center gap-2 border-t border-separator px-3 py-1.5 text-left" @click="toggleLog">
        <span class="type-body">{{ t("settings.whatsNew") }}</span>
        <span v-if="unseen.length" class="size-[7px] rounded-full bg-sys-link" />
      </button>
      <div
        v-if="showLog"
        class="flex flex-col gap-2.5 border-t border-separator px-3 pt-1 pb-2.5 [&+*]:border-t [&+*]:border-separator"
      >
        <div v-for="e in changelog" :key="e.version">
          <div class="flex items-center gap-2">
            <span class="type-footnote font-semibold">{{ e.version }}</span>
            <span
              v-if="freshVersions.has(e.version)"
              class="type-caption rounded-full bg-[color-mix(in_srgb,var(--sys-link)_22%,transparent)] px-1.5 font-medium text-sys-link"
            >{{ t("settings.new") }}</span>
            <span class="type-caption text-label-3">{{ e.date }}</span>
          </div>
          <ul class="type-caption mt-1 list-disc pl-4 text-label-2" :class="{ 'font-semibold': e.highlight }">
            <li v-for="(it, i) in items(e)" :key="i">{{ it }}</li>
          </ul>
        </div>
      </div>
      <Row :label="t('settings.autoUpdate')" :help="t('settings.autoUpdate.help')">
        <Switch v-model="settings.autoUpdate" />
      </Row>
    </Group>
  </div>
</template>
