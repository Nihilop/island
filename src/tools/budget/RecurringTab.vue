<script setup lang="ts">
// Onglet « Récurrentes » : abonnements, loyer, factures… avec la prochaine échéance (les
// rappels partent la veille et le jour même). Interrupteur = suspendre sans supprimer.
import { computed } from "vue";
import { PhArrowsClockwise } from "@phosphor-icons/vue";
import { Switch } from "@/sdk/ui/switch";
import { budget, money, nextDue, parse, today, type Recurring, categoryColor } from "./store";
import { iconOf } from "./icons";
import { locale } from "../../sdk/i18n";
import { t } from "../../notch/i18n";

const emit = defineEmits<{ edit: [r: Recurring] }>();

const rows = computed(() =>
  budget.recurring
    .map((r) => ({ r, due: nextDue(r) }))
    .sort((a, b) => Number(b.r.active) - Number(a.r.active) || (a.due ?? "9").localeCompare(b.due ?? "9")),
);

/** Coût mensuel équivalent (hebdo × 52/12, annuel / 12). */
const monthly = computed(() =>
  budget.recurring
    .filter((r) => r.active)
    .reduce((s, r) => s + (r.schedule.kind === "monthly" ? r.amount : r.schedule.kind === "yearly" ? r.amount / 12 : (r.amount * 52) / 12), 0),
);

function schedule(r: Recurring): string {
  const s = r.schedule;
  if (s.kind === "monthly") return t("budget.sched.everyMonth", { day: s.day });
  if (s.kind === "weekly") {
    return t("budget.sched.everyWeek", { day: new Intl.DateTimeFormat(locale.value, { weekday: "long" }).format(new Date(2024, 0, 1 + s.weekday)) });
  }
  return t("budget.sched.everyYear", { date: new Intl.DateTimeFormat(locale.value, { day: "numeric", month: "long" }).format(new Date(2024, s.month, s.day)) });
}

function dueLabel(due: string | null): string {
  if (!due) return "";
  const days = Math.round((parse(due).getTime() - parse(today()).getTime()) / 86_400_000);
  if (days === 0) return t("budget.due.today");
  if (days === 1) return t("budget.due.tomorrow");
  return t("budget.due.in", { n: days });
}
</script>

<template>
  <div class="flex h-full flex-col gap-2">
    <div class="type-caption flex items-center gap-2 px-2 text-label-3">
      <PhArrowsClockwise :size="11" weight="bold" />
      <span class="flex-1">{{ t("budget.recurringCount", { n: budget.recurring.filter((r) => r.active).length }) }}</span>
      <span>{{ t("budget.perMonth", { amount: money(monthly) }) }}</span>
    </div>
    <div class="min-h-0 flex-1 overflow-y-auto [scrollbar-width:thin]">
      <div v-if="!rows.length" class="type-body grid h-full place-items-center px-6 text-center text-label-3">{{ t("budget.emptyRecurring") }}</div>
      <div
        v-for="{ r, due } in rows"
        :key="r.id"
        class="flex h-11 cursor-default items-center gap-2.5 rounded-[10px] px-2 hover:bg-fill-1"
        :class="{ 'opacity-50': !r.active }"
        @click="emit('edit', r)"
      >
        <span
          class="grid size-7 shrink-0 place-items-center rounded-[8px]"
          :style="{ background: `color-mix(in srgb, ${categoryColor(r.category)} 20%, transparent)`, color: categoryColor(r.category) }"
        >
          <component :is="iconOf(r.icon)" :size="14" weight="bold" />
        </span>
        <div class="min-w-0 flex-1">
          <div class="type-footnote truncate font-medium">{{ r.name }}</div>
          <div class="type-caption truncate text-label-3">{{ schedule(r) }}</div>
        </div>
        <span v-if="r.active && due" class="type-caption shrink-0" :class="due === today() ? 'text-sys-orange' : 'text-label-3'">{{ dueLabel(due) }}</span>
        <span class="type-footnote w-[72px] shrink-0 text-right font-medium tabular-nums">{{ money(r.amount) }}</span>
        <Switch :model-value="r.active" @click.stop @update:model-value="r.active = $event" />
      </div>
    </div>
  </div>
</template>
