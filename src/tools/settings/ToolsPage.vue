<script setup lang="ts">
// Réglages › Outils : liste réordonnable par glisser (poignée), activation par outil.
// Impossible de désactiver le dernier outil actif.
import { computed, ref } from "vue";
import { PhDotsSixVertical } from "@phosphor-icons/vue";
import { settings } from "../../notch/settings";
import { hold } from "../../notch/state";
import { tools } from "../registry";
import { t } from "../../notch/i18n";
import Group from "../../ui/Group.vue";
import { Switch } from "@/sdk/ui/switch";
import VaultSettings from "../vault/VaultSettings.vue";
import HelloSettings from "./HelloSettings.vue";

const ROW_H = 40;

const rows = computed(() =>
  settings.tools.map((p) => ({ pref: p, tool: tools.value.find((x) => x.id === p.id) })).filter((r) => r.tool),
);
const activeCount = computed(() => settings.tools.filter((p) => p.enabled).length);

function setEnabled(id: string, on: boolean) {
  const p = settings.tools.find((x) => x.id === id);
  if (!p || (!on && activeCount.value <= 1)) return;
  p.enabled = on;
}

// --- Réordonnancement : pendant le drag l'ordre ne bouge pas ; la ligne tenue suit le
// curseur (sans transition), les autres glissent d'une hauteur pour lui faire place, et
// l'ordre n'est enregistré qu'au relâchement (une seule mise à jour, aucun saut).
const drag = ref<{ id: string; from: number; dy: number } | null>(null);
let startY = 0;
let release: (() => void) | null = null;

/** Emplacement visé par la ligne tenue. */
const target = computed(() => {
  const d = drag.value;
  if (!d) return -1;
  return Math.max(0, Math.min(rows.value.length - 1, d.from + Math.round(d.dy / ROW_H)));
});

/** Décalage vertical d'une ligne non tenue pour libérer l'emplacement visé. */
function shift(i: number): number {
  const d = drag.value;
  if (!d) return 0;
  const to = target.value;
  if (d.from < to && i > d.from && i <= to) return -ROW_H;
  if (to < d.from && i >= to && i < d.from) return ROW_H;
  return 0;
}

function onHandleDown(e: PointerEvent, id: string) {
  drag.value = { id, from: rows.value.findIndex((r) => r.pref.id === id), dy: 0 };
  startY = e.clientY;
  release = hold();
  (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
}

function onHandleMove(e: PointerEvent) {
  if (!drag.value) return;
  // Bornée à la liste : la ligne ne sort pas du groupe.
  const max = (rows.value.length - 1 - drag.value.from) * ROW_H;
  const min = -drag.value.from * ROW_H;
  drag.value.dy = Math.max(min, Math.min(max, e.clientY - startY));
}

function onHandleUp() {
  const d = drag.value;
  if (d && target.value !== d.from) {
    const list = settings.tools;
    const [item] = list.splice(d.from, 1);
    list.splice(target.value, 0, item);
  }
  drag.value = null;
  release?.();
  release = null;
}
</script>

<template>
  <div>
    <Group :title="t('settings.page.tools')" section="tools" :note="t('settings.tools.note')">
      <div class="relative">
        <div
          v-for="(r, i) in rows"
          :key="r.pref.id"
          class="relative flex h-10 items-center gap-2.5 pr-3 pl-1.5 not-first:border-t"
          :class="
            drag?.id === r.pref.id
              ? 'z-10 rounded-[10px] border-transparent bg-elevated shadow-[0_6px_18px_rgb(0_0_0/0.45)]'
              : drag
                ? 'border-separator transition-transform duration-200 ease-out'
                : 'border-separator'
          "
          :style="
            drag?.id === r.pref.id
              ? { transform: `translateY(${drag.dy}px) scale(1.02)` }
              : drag
                ? { transform: `translateY(${shift(i)}px)` }
                : undefined
          "
        >
          <button
            type="button"
            class="grid h-7 w-5 touch-none place-items-center text-label-3"
            :class="drag?.id === r.pref.id ? 'cursor-grabbing' : 'cursor-grab'"
            data-no-drag
            @pointerdown="onHandleDown($event, r.pref.id)"
            @pointermove="onHandleMove"
            @pointerup="onHandleUp"
            @pointercancel="onHandleUp"
          >
            <PhDotsSixVertical :size="14" weight="bold" />
          </button>
          <component :is="r.tool!.icon" :size="15" weight="bold" class="text-label-2" />
          <span class="type-body flex-1">{{ r.tool!.title() }}</span>
          <Switch
            :model-value="r.pref.enabled"
            :disabled="r.pref.enabled && activeCount <= 1"
            @update:model-value="setEnabled(r.pref.id, $event)"
          />
        </div>
      </div>
    </Group>

    <HelloSettings />

    <VaultSettings />

  </div>
</template>
