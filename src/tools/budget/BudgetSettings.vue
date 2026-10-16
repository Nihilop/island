<script setup lang="ts">
// Réglages › Outils › Budget : salaire par défaut, jour de paie, rappels des récurrentes.
import { ref, watch } from "vue";
import Group from "../../ui/Group.vue";
import Row from "../../ui/Row.vue";
import TextField from "../../ui/TextField.vue";
import { Switch } from "@/sdk/ui/switch";
import { budget, removeCategory } from "./store";
import { Button } from "@/sdk/ui/button";
import { PhTrash } from "@phosphor-icons/vue";
import { t } from "../../notch/i18n";

const salary = ref(budget.salary === null ? "" : String(budget.salary).replace(".", ","));
const payday = ref(String(budget.payday));
watch(
  () => [budget.salary, budget.payday] as const,
  ([s, p]) => {
    salary.value = s === null ? "" : String(s).replace(".", ",");
    payday.value = String(p);
  },
);

function saveSalary() {
  const v = Number(salary.value.replace(/\s/g, "").replace(",", "."));
  budget.salary = salary.value.trim() && v >= 0 ? v : null;
}

function savePayday() {
  const d = Number(payday.value);
  if (d >= 1 && d <= 31) budget.payday = d;
  else payday.value = String(budget.payday);
}
</script>

<template>
  <Group :title="t('tool.budget')" section="budget" :note="t('budget.settings.note')">
    <Row :label="t('budget.settings.salary')" :help="t('budget.settings.salaryHelp')">
      <div class="w-[120px]"><TextField v-model="salary" inputmode="decimal" @enter="saveSalary" @blur="saveSalary" /></div>
    </Row>
    <Row :label="t('budget.settings.payday')" :help="t('budget.settings.paydayHelp')">
      <div class="w-[64px]"><TextField v-model="payday" inputmode="numeric" @enter="savePayday" @blur="savePayday" /></div>
    </Row>
    <Row :label="t('budget.settings.reminders')" :help="t('budget.settings.remindersHelp')">
      <Switch v-model="budget.reminders" />
    </Row>
    <Row v-for="c in budget.categories" :key="c.id" :label="c.name" :help="t('budget.settings.categoryHelp')">
      <span class="size-2.5 rounded-full" :style="{ background: c.color }" />
      <Button size="icon-sm" variant="ghost" :title="t('budget.form.delete')" @click="removeCategory(c.id)"><PhTrash weight="bold" /></Button>
    </Row>
  </Group>
</template>
