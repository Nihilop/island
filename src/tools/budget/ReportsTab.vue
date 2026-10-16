<script setup lang="ts">
// Onglet « Rapports » : revenus et dépenses mois par mois, chiffres clés, répartition par
// catégorie et plus grosses dépenses, sur la période choisie (en-tête).
import { computed } from "vue";
import { money, parse, categoryColor, categoryLabel } from "./store";
import { report, type Range } from "./reports";
import { iconOf } from "./icons";
import { presentation } from "../../notch/presentation";
import { locale } from "../../sdk/i18n";
import { t } from "../../notch/i18n";

const props = defineProps<{ range: Range }>();

const r = computed(() => report(props.range));
/** Échelle du graphique : le plus grand revenu ou dépense de la période. */
const scale = computed(() => Math.max(1, ...r.value.months.map((x) => Math.max(x.income, x.recurring + x.expenses))));
const monthName = (y: number, m: number, style: "narrow" | "long") =>
  new Intl.DateTimeFormat(locale.value, style === "long" ? { month: "long", year: "numeric" } : { month: "short" }).format(new Date(y, m, 1));
const pct = (v: number | null) => (v === null ? "–" : `${Math.round(v * 100)} %`);
const day = (date: string) => new Intl.DateTimeFormat(locale.value, { day: "numeric", month: "short" }).format(parse(date));
</script>

<template>
  <div class="h-full overflow-y-auto pr-1 [scrollbar-color:var(--fill-3)_transparent] [scrollbar-width:thin]">
    <div v-if="!r.months.length" class="type-body grid h-full place-items-center text-label-3">{{ t("budget.reports.empty") }}</div>
    <div v-else class="flex flex-col gap-3 pb-1">
      <!-- Revenus / dépenses par mois -->
      <div class="flex h-[112px] items-end gap-1.5">
        <div
          v-for="x in r.months"
          :key="x.ym"
          class="flex h-full min-w-0 flex-1 flex-col items-center gap-1"
          :title="presentation ? monthName(x.y, x.m, 'long') : `${monthName(x.y, x.m, 'long')} · ${t('budget.salary')} ${money(x.income)} · ${t('budget.reports.spent')} ${money(x.recurring + x.expenses)}`"
        >
          <div class="flex w-full flex-1 items-end justify-center gap-[3px]">
            <div class="w-[9px] rounded-t-[3px] bg-fill-3" :style="{ height: `${(x.income / scale) * 100}%` }" />
            <div
              class="w-[9px] rounded-t-[3px]"
              :class="x.recurring + x.expenses > x.income ? 'bg-sys-red' : 'bg-sys-green'"
              :style="{ height: `${((x.recurring + x.expenses) / scale) * 100}%` }"
            />
          </div>
          <span class="type-caption truncate text-label-3">{{ monthName(x.y, x.m, "narrow") }}</span>
        </div>
      </div>
      <div class="type-caption -mt-1 flex items-center gap-3 text-label-3">
        <span class="flex items-center gap-1"><span class="size-2 rounded-[2px] bg-fill-3" />{{ t("budget.salary") }}</span>
        <span class="flex items-center gap-1"><span class="size-2 rounded-[2px] bg-sys-green" />{{ t("budget.reports.spent") }}</span>
      </div>

      <!-- Chiffres clés -->
      <div class="grid grid-cols-4 gap-1.5">
        <div class="rounded-[10px] bg-fill-1 px-2.5 py-2">
          <div class="type-caption text-label-3">{{ t("budget.reports.average") }}</div>
          <div class="type-footnote font-semibold tabular-nums">{{ money(r.average) }}</div>
        </div>
        <div class="rounded-[10px] bg-fill-1 px-2.5 py-2">
          <div class="type-caption text-label-3">{{ t("budget.reports.savings") }}</div>
          <div class="type-footnote font-semibold tabular-nums" :class="(r.savingsRate ?? 0) < 0 ? 'text-sys-red' : 'text-sys-green'">{{ pct(r.savingsRate) }}</div>
        </div>
        <div class="rounded-[10px] bg-fill-1 px-2.5 py-2">
          <div class="type-caption text-label-3">{{ t("budget.reports.recurringShare") }}</div>
          <div class="type-footnote font-semibold tabular-nums">{{ pct(r.recurringShare) }}</div>
        </div>
        <div class="rounded-[10px] bg-fill-1 px-2.5 py-2">
          <div class="type-caption text-label-3">{{ t("budget.reports.peak") }}</div>
          <div class="type-footnote truncate font-semibold first-letter:uppercase">{{ r.peak ? monthName(r.peak.y, r.peak.m, "long") : "–" }}</div>
        </div>
      </div>

      <div class="grid grid-cols-2 gap-4">
        <!-- Catégories -->
        <div class="flex flex-col gap-1.5">
          <div class="type-caption text-label-3">{{ t("budget.reports.byCategory") }}</div>
          <div v-for="c in r.byCategory" :key="c.category" class="flex flex-col gap-0.5">
            <div class="type-caption flex items-center gap-1.5">
              <span class="size-1.5 rounded-full" :style="{ background: categoryColor(c.category) }" />
              <span class="flex-1 text-label-2">{{ categoryLabel(c.category) }}</span>
              <span class="tabular-nums">{{ money(c.amount) }}</span>
            </div>
            <div class="h-1 overflow-hidden rounded-full bg-fill-1">
              <div class="h-full rounded-full" :style="{ width: `${(c.amount / r.spent) * 100}%`, background: categoryColor(c.category) }" />
            </div>
          </div>
          <div v-if="!r.byCategory.length" class="type-caption text-label-3">–</div>
        </div>
        <!-- Plus grosses dépenses -->
        <div class="flex flex-col gap-1">
          <div class="type-caption text-label-3">{{ t("budget.reports.top") }}</div>
          <div v-for="l in r.top" :key="l.key" class="flex h-7 items-center gap-2">
            <component :is="iconOf(l.icon)" :size="13" weight="bold" :style="{ color: categoryColor(l.category) }" />
            <span class="type-footnote min-w-0 flex-1 truncate">{{ l.name }}</span>
            <span class="type-caption text-label-3">{{ day(l.date) }}</span>
            <span class="type-footnote tabular-nums">{{ money(l.amount) }}</span>
          </div>
          <div v-if="!r.top.length" class="type-caption text-label-3">{{ t("budget.reports.noTop") }}</div>
        </div>
      </div>
    </div>
  </div>
</template>
