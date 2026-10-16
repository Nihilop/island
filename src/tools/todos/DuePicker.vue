<script setup lang="ts">
// Sélecteur d'échéance compact (popover) : raccourcis Aujourd'hui / Demain / Semaine
// prochaine, champ date, heure facultative, retrait. Le slot par défaut est le déclencheur
// (enveloppé dans une ancre stable). Chaque changement s'applique tout de suite au modèle.
import { onBeforeUnmount, ref, watch } from "vue";
import { Popover, PopoverContent, PopoverTrigger } from "@/sdk/ui/popover";
import { Button } from "@/sdk/ui/button";
import TextField from "../../ui/TextField.vue";
import { hold } from "../../notch/state";
import { t } from "../../notch/i18n";
import { addDays, nextWeek, today, type Due } from "./store";

const model = defineModel<Due | null>({ required: true });
const emit = defineEmits<{ close: [] }>();

// Le popover ouvert tient le notch ouvert (le curseur passe dessus, hors de la forme).
const open = ref(false);
let release: (() => void) | null = null;
watch(open, (o) => {
  if (o) release = hold();
  else {
    release?.();
    release = null;
    emit("close");
  }
});
onBeforeUnmount(() => release?.());

const set = (due: string) => (model.value = { due, time: model.value?.time });
const date = ref("");
const time = ref("");
watch(
  model,
  (m) => {
    date.value = m?.due ?? "";
    time.value = m?.time ?? "";
  },
  { immediate: true },
);
watch(date, (d) => {
  if (d === (model.value?.due ?? "")) return;
  model.value = d ? { due: d, time: model.value?.time } : null;
});
// Une heure sans jour : aujourd'hui.
watch(time, (h) => {
  if (h === (model.value?.time ?? "")) return;
  model.value = { due: model.value?.due ?? today(), time: h || undefined };
});

function clear() {
  model.value = null;
  open.value = false;
}

const shortcuts = [
  { label: "todos.due.today", due: () => today() },
  { label: "todos.due.tomorrow", due: () => addDays(today(), 1) },
  { label: "todos.due.nextWeek", due: nextWeek },
];
</script>

<template>
  <Popover v-model:open="open">
    <!-- Ancre stable : le contenu du slot change d'élément quand une date est posée (icône →
         pastille) ; ancré directement dessus, le popover garderait l'élément retiré du DOM
         et se placerait en haut à gauche de l'écran. -->
    <PopoverTrigger as-child>
      <span class="inline-flex shrink-0"><slot :open="open" /></span>
    </PopoverTrigger>
    <PopoverContent side="bottom" align="end" :side-offset="6" class="flex w-72 flex-col gap-2 p-2" data-no-drag>
      <div class="flex gap-1">
        <Button
          v-for="s in shortcuts"
          :key="s.label"
          size="sm"
          :variant="model?.due === s.due() ? 'default' : 'secondary'"
          class="type-caption h-6 grow px-2"
          @click="set(s.due())"
        >
          {{ t(s.label) }}
        </Button>
      </div>
      <div class="flex gap-1.5">
        <TextField v-model="date" type="date" class="flex-1 [&_input]:[color-scheme:dark] [.light_&_input]:[color-scheme:light]" @enter="open = false" />
        <TextField v-model="time" type="time" class="w-[84px] [&_input]:[color-scheme:dark] [.light_&_input]:[color-scheme:light]" @enter="open = false" />
      </div>
      <div class="flex items-center justify-between">
        <Button v-if="model" variant="ghost" size="sm" class="type-caption h-6 px-1.5 text-sys-red" @click="clear">
          {{ t("todos.due.remove") }}
        </Button>
        <span v-else />
        <Button size="sm" class="type-caption h-6 px-2.5" @click="open = false">{{ t("todos.due.done") }}</Button>
      </div>
    </PopoverContent>
  </Popover>
</template>
