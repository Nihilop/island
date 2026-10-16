<script setup lang="ts">
// Onglet « Calendrier » : grille du mois (semaine du lundi), points colorés par dépense,
// jour de paie marqué. Clic sur un jour = son détail à droite, avec « Ajouter ce jour ».
import { computed } from "vue";
import { PhArrowsClockwise, PhCurrencyEur, PhPlus } from "@phosphor-icons/vue";
import { Button } from "@/sdk/ui/button";
import { budget, daysIn, linesOf, money, parse, today, type Line, categoryColor } from "./store";
import { iconOf } from "./icons";
import { locale } from "../../sdk/i18n";
import { t } from "../../notch/i18n";

const props = defineProps<{ y: number; m: number }>();
const selected = defineModel<string>("selected", { required: true });
const emit = defineEmits<{ add: [date: string]; edit: [line: Line] }>();

const pad = (n: number) => String(n).padStart(2, "0");
const lines = computed(() => linesOf(props.y, props.m));
const byDay = computed(() => {
  const map = new Map<string, Line[]>();
  for (const l of lines.value) map.set(l.date, [...(map.get(l.date) ?? []), l]);
  return map;
});

/** Cases de la grille : vides avant le 1er (lundi en premier), puis les jours. */
const cells = computed(() => {
  const lead = (new Date(props.y, props.m, 1).getDay() + 6) % 7;
  const n = daysIn(props.y, props.m);
  return [
    ...Array.from({ length: lead }, () => null),
    ...Array.from({ length: n }, (_, i) => `${props.y}-${pad(props.m + 1)}-${pad(i + 1)}`),
  ];
});
const payday = computed(() => `${props.y}-${pad(props.m + 1)}-${pad(Math.min(budget.payday, daysIn(props.y, props.m)))}`);
const weekdays = computed(() =>
  Array.from({ length: 7 }, (_, d) => new Intl.DateTimeFormat(locale.value, { weekday: "narrow" }).format(new Date(2024, 0, 1 + d))),
);

const dayLines = computed(() => byDay.value.get(selected.value) ?? []);
const dayTotal = computed(() => dayLines.value.reduce((s, l) => s + l.amount, 0));
const selectedLabel = computed(() =>
  selected.value.startsWith(`${props.y}-${pad(props.m + 1)}`)
    ? new Intl.DateTimeFormat(locale.value, { weekday: "long", day: "numeric", month: "long" }).format(parse(selected.value))
    : "",
);
</script>

<template>
  <div class="flex h-full gap-4">
    <div class="flex w-[300px] shrink-0 flex-col">
      <div class="type-caption grid grid-cols-7 pb-1 text-center text-label-3">
        <span v-for="(w, i) in weekdays" :key="i">{{ w }}</span>
      </div>
      <div class="grid flex-1 grid-cols-7 grid-rows-6 gap-0.5">
        <template v-for="(date, i) in cells" :key="i">
          <span v-if="!date" />
          <button
            v-else
            type="button"
            class="relative flex flex-col items-center justify-start rounded-[8px] pt-1 transition-colors duration-100"
            :class="date === selected ? 'bg-fill-3' : 'hover:bg-fill-1'"
            @click="selected = date"
          >
            <span
              class="type-caption grid size-5 place-items-center rounded-full tabular-nums"
              :class="date === today() ? 'bg-sys-red font-semibold text-white' : date > today() ? 'text-label-2' : 'text-label'"
            >{{ Number(date.slice(8)) }}</span>
            <span class="mt-0.5 flex h-1.5 items-center gap-[3px]">
              <PhCurrencyEur v-if="date === payday" :size="9" weight="bold" class="text-sys-green" />
              <span
                v-for="l in (byDay.get(date) ?? []).slice(0, 3)"
                :key="l.key"
                class="size-1.5 rounded-full"
                :style="{ background: categoryColor(l.category) }"
              />
            </span>
          </button>
        </template>
      </div>
    </div>

    <!-- Détail du jour -->
    <div class="flex min-w-0 flex-1 flex-col gap-1.5">
      <div class="flex items-center gap-2">
        <div class="min-w-0 flex-1">
          <div class="type-footnote truncate font-medium first-letter:uppercase">{{ selectedLabel }}</div>
          <div class="type-caption text-label-3">{{ dayLines.length ? money(-dayTotal) : t("budget.noExpense") }}</div>
        </div>
        <Button size="sm" @click="emit('add', selected)"><PhPlus weight="bold" />{{ t("budget.addThisDay") }}</Button>
      </div>
      <div class="min-h-0 flex-1 overflow-y-auto [scrollbar-width:thin]">
        <div v-if="selected === payday" class="type-footnote flex h-9 items-center gap-2.5 px-2 text-sys-green">
          <PhCurrencyEur :size="14" weight="bold" />
          <span class="flex-1">{{ t("budget.payday") }}</span>
        </div>
        <div
          v-for="l in dayLines"
          :key="l.key"
          class="flex h-9 cursor-default items-center gap-2.5 rounded-[10px] px-2 hover:bg-fill-1"
          @click="emit('edit', l)"
        >
          <component :is="iconOf(l.icon)" :size="14" weight="bold" :style="{ color: categoryColor(l.category) }" />
          <span class="type-footnote flex-1 truncate">{{ l.name }}</span>
          <PhArrowsClockwise v-if="l.kind === 'recurring'" :size="10" weight="bold" class="text-label-3" />
          <span class="type-footnote tabular-nums">{{ money(-l.amount) }}</span>
        </div>
      </div>
    </div>
  </div>
</template>
