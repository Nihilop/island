<script setup lang="ts">
// Outil « Budget » (640 × 420) : onglets Mois / Calendrier / Récurrentes, ajout rapide,
// encart « salaire du mois » pendant la semaine de paie. Verrou Windows Hello en option
// (Réglages › Outils › Verrouillage) : tout reste masqué jusqu'à la confirmation.
import { computed, onMounted, ref } from "vue";
import { PhCaretLeft, PhCaretRight, PhCurrencyEur, PhDownloadSimple, PhFingerprint, PhLockSimple, PhLockSimpleOpen, PhPlus, PhX } from "@phosphor-icons/vue";
import Segmented from "../../ui/Segmented.vue";
import IconButton from "../../ui/IconButton.vue";
import TextField from "../../ui/TextField.vue";
import Empty from "../../ui/Empty.vue";
import { Button } from "@/sdk/ui/button";
import EntryForm, { type FormState } from "./EntryForm.vue";
import MonthTab from "./MonthTab.vue";
import CalendarTab from "./CalendarTab.vue";
import RecurringTab from "./RecurringTab.vue";
import ReportsTab from "./ReportsTab.vue";
import { exportCsv, type Range } from "./reports";
import {
  addExpense,
  budget,
  dismissSalary,
  removeExpense,
  removeRecurring,
  salaryPrompt,
  saveRecurring,
  setMonthSalary,
  today,
  type Line,
  type Recurring,
} from "./store";
import { CATEGORY_ICON } from "./icons";
import { useDraft } from "../../stores/drafts";
import { settings } from "../../notch/settings";
import { hello, helloError, isLocked, lockScope, touchScope, unlockScope } from "../../notch/hello";
import { locale } from "../../sdk/i18n";
import { t } from "../../notch/i18n";

type Tab = "month" | "calendar" | "recurring" | "reports";
const tab = useDraft<Tab>("budget.tab", () => "month");
/** Mois affiché (année, mois 0–11). */
const cursor = useDraft("budget.cursor", () => ({ y: new Date().getFullYear(), m: new Date().getMonth() }));
const selectedDay = useDraft("budget.day", () => today());
const range = useDraft<Range>("budget.range", () => "6m");

const exporting = ref(false);
const exportError = ref(false);
async function onExport() {
  if (exporting.value) return;
  exporting.value = true;
  exportError.value = false;
  try {
    await exportCsv();
  } catch {
    exportError.value = true;
    window.setTimeout(() => (exportError.value = false), 2600);
  } finally {
    exporting.value = false;
  }
}
/** Formulaire ouvert (brouillon conservé si le notch se ferme). */
const form = useDraft<FormState | null>("budget.form", () => null);

const monthLabel = computed(() =>
  new Intl.DateTimeFormat(locale.value, { month: "long", year: "numeric" }).format(new Date(cursor.value.y, cursor.value.m, 1)),
);

function shift(delta: number) {
  const d = new Date(cursor.value.y, cursor.value.m + delta, 1);
  cursor.value = { y: d.getFullYear(), m: d.getMonth() };
}

// --- Formulaire -------------------------------------------------------------------------

function blank(mode: FormState["mode"], date = today()): FormState {
  return {
    mode,
    name: "",
    amount: "",
    icon: CATEGORY_ICON.other,
    category: "other",
    date,
    schedule: { kind: "monthly", day: String(Number(date.slice(8))), month: Number(date.slice(5, 7)) - 1, weekday: 0 },
  };
}

function openAdd(date?: string) {
  form.value = blank(tab.value === "recurring" ? "recurring" : "expense", date ?? (tab.value === "calendar" ? selectedDay.value : today()));
}

function editRecurring(r: Recurring) {
  form.value = {
    mode: "recurring",
    id: r.id,
    name: r.name,
    amount: String(r.amount).replace(".", ","),
    icon: r.icon,
    category: r.category,
    date: today(),
    schedule: {
      kind: r.schedule.kind,
      day: "day" in r.schedule ? String(r.schedule.day) : "1",
      month: r.schedule.kind === "yearly" ? r.schedule.month : 0,
      weekday: r.schedule.kind === "weekly" ? r.schedule.weekday : 0,
    },
  };
}

function editLine(l: Line) {
  if (l.kind === "recurring") {
    const r = budget.recurring.find((x) => x.id === l.id);
    if (r) editRecurring(r);
    return;
  }
  const e = budget.expenses.find((x) => x.id === l.id);
  if (!e) return;
  form.value = { ...blank("expense", e.date), id: e.id, name: e.name, amount: String(e.amount).replace(".", ","), icon: e.icon, category: e.category };
}

function save() {
  const f = form.value;
  if (!f) return;
  const amount = Number(f.amount.replace(/\s/g, "").replace(",", "."));
  const base = { name: f.name.trim(), icon: f.icon, amount, category: f.category };
  if (f.mode === "expense") {
    if (f.id) {
      const e = budget.expenses.find((x) => x.id === f.id);
      if (e) Object.assign(e, base, { date: f.date });
    } else addExpense({ ...base, date: f.date });
  } else {
    const day = Math.max(1, Math.min(31, Number(f.schedule.day) || 1));
    const schedule =
      f.schedule.kind === "monthly"
        ? ({ kind: "monthly", day } as const)
        : f.schedule.kind === "yearly"
          ? ({ kind: "yearly", month: f.schedule.month, day } as const)
          : ({ kind: "weekly", weekday: f.schedule.weekday } as const);
    saveRecurring({ ...base, id: f.id, schedule });
  }
  touchScope("budget");
  form.value = null;
}

function remove() {
  const f = form.value;
  if (!f?.id) return;
  if (f.mode === "expense") removeExpense(f.id);
  else removeRecurring(f.id);
  form.value = null;
}

// --- Salaire ------------------------------------------------------------------------------

const salaryDraft = useDraft("budget.salaryDraft", () => "");
const promptLabel = computed(() =>
  salaryPrompt.value ? new Intl.DateTimeFormat(locale.value, { month: "long" }).format(new Date(`${salaryPrompt.value}-01T12:00`)) : "",
);

function confirmSalary() {
  const ym = salaryPrompt.value;
  const v = Number(salaryDraft.value.replace(/\s/g, "").replace(",", "."));
  if (!ym || !(v >= 0) || !salaryDraft.value.trim()) return;
  setMonthSalary(ym, v);
  salaryDraft.value = "";
}

// Premier lancement : salaire par défaut + jour de paie.
const setup = ref({ salary: "", payday: "1" });
const setupValid = computed(() => Number(setup.value.salary.replace(/\s/g, "").replace(",", ".")) > 0 && Number(setup.value.payday) >= 1 && Number(setup.value.payday) <= 31);
function finishSetup() {
  if (!setupValid.value) return;
  budget.salary = Number(setup.value.salary.replace(/\s/g, "").replace(",", "."));
  budget.payday = Number(setup.value.payday);
}

// --- Verrou -----------------------------------------------------------------------------

const locked = computed(() => isLocked("budget"));
const lockError = ref<string | null>(null);
async function unlock() {
  if (hello.prompting) return;
  lockError.value = null;
  try {
    await unlockScope("budget");
  } catch (e) {
    lockError.value = helloError(t, e);
  }
}
onMounted(() => {
  if (locked.value) void unlock();
});
</script>

<template>
  <div class="flex h-full flex-col gap-2 px-4 pb-1" @pointerdown="touchScope('budget')">
    <Teleport defer to="#notch-header-actions">
      <template v-if="!locked && budget.salary !== null && !form">
        <template v-if="tab === 'month' || tab === 'calendar'">
          <IconButton :icon="PhCaretLeft" :label="t('budget.prevMonth')" @click="shift(-1)" />
          <span class="type-footnote min-w-[104px] text-center font-medium first-letter:uppercase">{{ monthLabel }}</span>
          <IconButton :icon="PhCaretRight" :label="t('budget.nextMonth')" @click="shift(1)" />
        </template>
        <Segmented
          v-model="tab"
          size="sm"
          :options="[
            { value: 'month', label: t('budget.tab.month') },
            { value: 'calendar', label: t('budget.tab.calendar') },
            { value: 'recurring', label: t('budget.tab.recurring') },
            { value: 'reports', label: t('budget.tab.reports') },
          ]"
        />
        <template v-if="tab === 'reports'">
          <Segmented
            v-model="range"
            size="sm"
            :options="[
              { value: '6m', label: t('budget.range.6m') },
              { value: '12m', label: t('budget.range.12m') },
              { value: 'year', label: t('budget.range.year') },
            ]"
          />
          <IconButton :icon="PhDownloadSimple" :label="t('budget.reports.export')" :loading="exporting" :disabled="exporting" @click="onExport" />
        </template>
        <IconButton v-else :icon="PhPlus" :label="t('budget.add')" @click="openAdd()" />
      </template>
      <IconButton v-if="settings.hello.budget && !locked" :icon="PhLockSimpleOpen" active :label="t('budget.lock')" @click="lockScope('budget')" />
    </Teleport>

    <!-- Verrouillé -->
    <Empty v-if="locked" :icon="PhLockSimple" :title="t('budget.locked')" :text="lockError ?? t('budget.lockedHelp')">
      <Button variant="default" :loading="hello.prompting" @click="unlock"><PhFingerprint weight="bold" />{{ t("hello.unlock") }}</Button>
    </Empty>

    <!-- Premier lancement -->
    <div v-else-if="budget.salary === null" class="flex h-full flex-col items-center justify-center gap-3" data-no-drag>
      <span class="grid size-10 place-items-center rounded-full bg-fill-2 text-sys-green"><PhCurrencyEur :size="18" weight="bold" /></span>
      <div class="text-center">
        <div class="type-body font-medium">{{ t("budget.setup.title") }}</div>
        <div class="type-caption text-label-3">{{ t("budget.setup.help") }}</div>
      </div>
      <div class="flex items-end gap-2">
        <label class="flex w-[150px] flex-col gap-1">
          <span class="type-caption text-label-3">{{ t("budget.setup.salary") }}</span>
          <TextField v-model="setup.salary" inputmode="decimal" autofocus @enter="finishSetup" />
        </label>
        <label class="flex w-[100px] flex-col gap-1">
          <span class="type-caption text-label-3">{{ t("budget.setup.payday") }}</span>
          <TextField v-model="setup.payday" inputmode="numeric" @enter="finishSetup" />
        </label>
        <Button variant="default" :disabled="!setupValid" @click="finishSetup">{{ t("budget.setup.start") }}</Button>
      </div>
    </div>

    <template v-else>
      <!-- Salaire du mois (semaine de paie) -->
      <div v-if="salaryPrompt && !form" class="flex shrink-0 items-center gap-2.5 rounded-[12px] bg-fill-1 py-1.5 pr-1.5 pl-3" data-no-drag>
        <PhCurrencyEur :size="15" weight="bold" class="shrink-0 text-sys-green" />
        <div class="min-w-0 flex-1">
          <div class="type-footnote font-medium">{{ t("budget.prompt.title", { month: promptLabel }) }}</div>
          <div class="type-caption truncate text-label-3">{{ t("budget.prompt.help") }}</div>
        </div>
        <div class="w-[110px]"><TextField v-model="salaryDraft" inputmode="decimal" :placeholder="String(budget.salary)" @enter="confirmSalary" /></div>
        <Button size="sm" variant="default" :disabled="!salaryDraft.trim()" @click="confirmSalary">{{ t("budget.prompt.save") }}</Button>
        <Button size="icon-sm" variant="ghost" :title="t('budget.prompt.dismiss')" @click="dismissSalary(salaryPrompt)"><PhX weight="bold" /></Button>
      </div>

      <div v-if="exportError" class="type-caption shrink-0 text-sys-red">{{ t("budget.reports.exportFailed") }}</div>
      <div class="min-h-0 flex-1">
        <EntryForm v-if="form" v-model="form" @save="save" @cancel="form = null" @remove="remove" />
        <MonthTab v-else-if="tab === 'month'" :y="cursor.y" :m="cursor.m" @edit="editLine" />
        <CalendarTab v-else-if="tab === 'calendar'" v-model:selected="selectedDay" :y="cursor.y" :m="cursor.m" @add="openAdd" @edit="editLine" />
        <RecurringTab v-else-if="tab === 'recurring'" @edit="editRecurring" />
        <ReportsTab v-else :range="range" />
      </div>
    </template>
  </div>
</template>
