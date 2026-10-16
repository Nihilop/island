<script setup lang="ts">
// Onglet « Mois » : ce qui reste (salaire − récurrentes − dépenses), le détail, la
// répartition par catégorie, puis les opérations du mois. Clic sur une ligne = modifier ;
// clic sur le salaire = saisir le salaire réel du mois.
import { computed, ref } from "vue";
import { PhArrowsClockwise, PhPencilSimple, PhX } from "@phosphor-icons/vue";
import TextField from "../../ui/TextField.vue";
import { Button } from "@/sdk/ui/button";
import { budget, linesOf, money, monthKey, parse, removeExpense, setMonthSalary, summaryOf, today, type Line, categoryColor, categoryLabel } from "./store";
import { iconOf } from "./icons";
import { locale } from "../../sdk/i18n";
import { t } from "../../notch/i18n";

const props = defineProps<{ y: number; m: number }>();
const emit = defineEmits<{ edit: [line: Line] }>();

const summary = computed(() => summaryOf(props.y, props.m));
const lines = computed(() => linesOf(props.y, props.m));
const ym = computed(() => monthKey(props.y, props.m));
const custom = computed(() => budget.salaries[ym.value] !== undefined);
const spentPct = computed(() => {
  const s = summary.value;
  return s.income > 0 ? Math.min(100, ((s.recurring + s.expenses) / s.income) * 100) : 0;
});

const editingSalary = ref(false);
const salaryDraft = ref("");
function editSalary() {
  salaryDraft.value = String(summary.value.income).replace(".", ",");
  editingSalary.value = true;
}
function saveSalary() {
  const v = Number(salaryDraft.value.replace(/\s/g, "").replace(",", "."));
  if (v >= 0) setMonthSalary(ym.value, v);
  editingSalary.value = false;
}

const dayLabel = (date: string) =>
  date === today() ? t("budget.today") : new Intl.DateTimeFormat(locale.value, { day: "numeric", month: "short" }).format(parse(date));
</script>

<template>
  <div class="flex h-full gap-4">
    <!-- Résumé -->
    <div class="flex w-[210px] shrink-0 flex-col gap-2.5">
      <div>
        <div class="type-caption text-label-3">{{ t("budget.remaining") }}</div>
        <div
          class="text-[28px] leading-[34px] font-semibold tracking-[-0.025em] tabular-nums"
          :class="summary.remaining < 0 ? 'text-sys-red' : 'text-label'"
        >{{ money(summary.remaining) }}</div>
        <div class="mt-1.5 h-1.5 overflow-hidden rounded-full bg-fill-2">
          <div
            class="h-full rounded-full transition-[width] duration-300 ease-out"
            :class="spentPct >= 100 ? 'bg-sys-red' : spentPct > 85 ? 'bg-sys-orange' : 'bg-sys-green'"
            :style="{ width: `${spentPct}%` }"
          />
        </div>
      </div>

      <div class="flex flex-col gap-1">
        <div class="type-footnote flex h-7 items-center gap-2">
          <span class="flex-1 text-label-2">{{ t("budget.salary") }}<span v-if="!custom" class="text-label-3"> · {{ t("budget.default") }}</span></span>
          <template v-if="editingSalary">
            <div class="w-[96px]"><TextField v-model="salaryDraft" inputmode="decimal" autofocus @enter="saveSalary" /></div>
            <Button size="icon-xs" variant="ghost" @click="editingSalary = false"><PhX weight="bold" /></Button>
          </template>
          <button v-else type="button" class="group/sal flex items-center gap-1 font-medium tabular-nums text-sys-green" @click="editSalary">
            {{ money(summary.income, { sign: true }) }}
            <PhPencilSimple :size="11" weight="bold" class="text-label-3 opacity-0 transition-opacity group-hover/sal:opacity-100" />
          </button>
        </div>
        <div class="type-footnote flex h-6 items-center">
          <span class="flex-1 text-label-2">{{ t("budget.recurringTotal") }}</span>
          <span class="tabular-nums">{{ money(-summary.recurring) }}</span>
        </div>
        <div class="type-footnote flex h-6 items-center">
          <span class="flex-1 text-label-2">{{ t("budget.expensesTotal") }}</span>
          <span class="tabular-nums">{{ money(-summary.expenses) }}</span>
        </div>
      </div>

      <!-- Répartition -->
      <div v-if="summary.byCategory.length" class="flex flex-col gap-1.5">
        <div class="flex h-2 overflow-hidden rounded-full bg-fill-1">
          <div
            v-for="c in summary.byCategory"
            :key="c.category"
            class="h-full"
            :style="{ width: `${(c.amount / (summary.recurring + summary.expenses)) * 100}%`, background: categoryColor(c.category) }"
          />
        </div>
        <div class="type-caption grid grid-cols-2 gap-x-2 gap-y-0.5">
          <span v-for="c in summary.byCategory.slice(0, 6)" :key="c.category" class="flex min-w-0 items-center gap-1.5 text-label-2">
            <span class="size-1.5 shrink-0 rounded-full" :style="{ background: categoryColor(c.category) }" />
            <span class="truncate">{{ categoryLabel(c.category) }}</span>
          </span>
        </div>
      </div>
    </div>

    <!-- Opérations -->
    <div class="min-h-0 min-w-0 flex-1 overflow-y-auto pr-1 [scrollbar-color:var(--fill-3)_transparent] [scrollbar-width:thin]">
      <div v-if="!lines.length" class="type-body grid h-full place-items-center text-center text-label-3">{{ t("budget.emptyMonth") }}</div>
      <div
        v-for="l in lines"
        :key="l.key"
        class="group/line flex h-10 cursor-default items-center gap-2.5 rounded-[10px] px-2 hover:bg-fill-1"
        @click="emit('edit', l)"
      >
        <span
          class="grid size-7 shrink-0 place-items-center rounded-[8px]"
          :style="{ background: `color-mix(in srgb, ${categoryColor(l.category)} 20%, transparent)`, color: categoryColor(l.category) }"
        >
          <component :is="iconOf(l.icon)" :size="14" weight="bold" />
        </span>
        <div class="min-w-0 flex-1">
          <div class="type-footnote truncate font-medium">{{ l.name }}</div>
          <div class="type-caption flex items-center gap-1 text-label-3">
            {{ dayLabel(l.date) }}
            <PhArrowsClockwise v-if="l.kind === 'recurring'" :size="10" weight="bold" />
          </div>
        </div>
        <span class="type-footnote font-medium tabular-nums" :class="l.date > today() ? 'text-label-3' : ''">{{ money(-l.amount) }}</span>
        <Button
          v-if="l.kind === 'expense'"
          size="icon-xs"
          variant="ghost"
          class="text-label-3 opacity-0 group-hover/line:opacity-100"
          :title="t('budget.form.delete')"
          @click.stop="removeExpense(l.id)"
        >
          <PhX weight="bold" />
        </Button>
      </div>
    </div>
  </div>
</template>
