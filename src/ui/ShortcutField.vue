<script setup lang="ts">
// Capture d'un raccourci global : clic → « Appuie sur une combinaison… », Échap annule,
// Retour arrière efface. Le notch prend le focus clavier le temps de la capture.
import { onBeforeUnmount, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import Kbd from "./Kbd.vue";
import { accelFromEvent, accelKeys } from "../notch/accel";
import { hold } from "../notch/state";
import { t } from "../notch/i18n";

const model = defineModel<string>({ required: true });
const capturing = ref(false);
let release: (() => void) | null = null;

function onKey(e: KeyboardEvent) {
  e.preventDefault();
  e.stopPropagation();
  if (e.key === "Escape") return stop();
  if (e.key === "Backspace" && !e.ctrlKey && !e.altKey) {
    model.value = "";
    return stop();
  }
  const accel = accelFromEvent(e);
  if (accel) {
    model.value = accel;
    stop();
  }
}

function start() {
  if (capturing.value) return stop();
  capturing.value = true;
  release = hold();
  invoke("overlay_focus").catch(() => {});
  window.addEventListener("keydown", onKey, true);
}

function stop() {
  capturing.value = false;
  window.removeEventListener("keydown", onKey, true);
  release?.();
  release = null;
}

onBeforeUnmount(stop);
</script>

<template>
  <button
    type="button"
    data-no-drag
    :class="[
      'inline-flex h-7 min-w-24 items-center justify-center rounded-sm transition-colors duration-150 hover:bg-fill-2',
      capturing ? 'bg-fill-2 px-2.5 ring-1 ring-inset ring-sys-link' : 'px-1',
    ]"
    @click="start"
  >
    <span v-if="capturing" class="type-caption text-label-2">{{ t("settings.shortcut.press") }}</span>
    <Kbd v-else-if="model" :keys="accelKeys(model)" />
    <span v-else class="type-caption text-label-3">{{ t("settings.shortcut.none") }}</span>
  </button>
</template>
