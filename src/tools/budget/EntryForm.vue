<script setup lang="ts">
// Formulaire du Budget : dépense ponctuelle ou dépense récurrente (ajout / modification).
// Choisir un nom déjà saisi reprend son icône, sa catégorie et son montant. Le brouillon
// vit dans le store (`useDraft`, cf. BudgetView) : il survit à la fermeture du notch.
import { computed, ref } from "vue";
import { PhPlus, PhTrash } from "@phosphor-icons/vue";
import TextField from "../../ui/TextField.vue";
import Segmented from "../../ui/Segmented.vue";
import Select from "../../ui/Select.vue";
import { Button } from "@/sdk/ui/button";
import { history, type Category, categoryColor, categoryLabel, categories, addCategory, CATEGORY_PALETTE } from "./store";
import { categoryIcon, ICONS, iconOf } from "./icons";
import { t } from "../../notch/i18n";

export interface FormState {
  mode: "expense" | "recurring";
  id?: string;
  name: string;
  amount: string;
  icon: string;
  category: Category;
  date: string;
  schedule: { kind: "monthly" | "yearly" | "weekly"; day: string; month: number; weekday: number };
}

const form = defineModel<FormState>({ required: true });
const emit = defineEmits<{ save: []; cancel: []; remove: [] }>();

const iconsOpen = ref(false);
const nameFocused = ref(false);

/** Montant saisi (virgule ou point), ou NaN. */
const amount = computed(() => Number(form.value.amount.replace(/\s/g, "").replace(",", ".")));
const day = computed(() => Number(form.value.schedule.day));
const valid = computed(
  () =>
    form.value.name.trim().length > 0 &&
    amount.value > 0 &&
    (form.value.mode === "expense"
      ? /^\d{4}-\d{2}-\d{2}$/.test(form.value.date)
      : form.value.schedule.kind === "weekly" || (day.value >= 1 && day.value <= 31)),
);

const suggestions = computed(() => {
  const q = form.value.name.trim().toLowerCase();
  if (!q || form.value.id) return [];
  return history.value.filter((e) => e.name.toLowerCase().includes(q) && e.name.toLowerCase() !== q).slice(0, 4);
});

function pick(e: (typeof history.value)[number]) {
  Object.assign(form.value, { name: e.name, icon: e.icon, category: e.category, amount: String(e.amount).replace(".", ",") });
}

function setCategory(c: Category) {
  // L'icône suit la catégorie tant qu'on ne l'a pas choisie soi-même.
  if (form.value.icon === categoryIcon(form.value.category)) form.value.icon = categoryIcon(c);
  form.value.category = c;
}

// Nouvelle catégorie, créée sur place.
const creating = ref(false);
const newName = ref("");
const newColor = ref(CATEGORY_PALETTE[0]);
function createCategory() {
  const name = newName.value.trim();
  if (!name) return;
  setCategory(addCategory(name, newColor.value));
  creating.value = false;
  newName.value = "";
}

const MONTHS = computed(() =>
  Array.from({ length: 12 }, (_, m) => ({ value: m, label: new Intl.DateTimeFormat(undefined, { month: "short" }).format(new Date(2024, m, 1)) })),
);
const WEEKDAYS = computed(() =>
  Array.from({ length: 7 }, (_, d) => ({ value: d, label: new Intl.DateTimeFormat(undefined, { weekday: "narrow" }).format(new Date(2024, 0, 1 + d)) })),
);
</script>

<template>
  <div class="flex h-full flex-col gap-2.5" data-no-drag>
    <div class="flex items-center gap-2">
      <Segmented
        v-if="!form.id"
        v-model="form.mode"
        size="sm"
        :options="[
          { value: 'expense', label: t('budget.form.expense') },
          { value: 'recurring', label: t('budget.form.recurring') },
        ]"
      />
      <span v-else class="type-footnote font-medium">{{ form.mode === "recurring" ? t("budget.form.editRecurring") : t("budget.form.editExpense") }}</span>
    </div>

    <!-- Icône · nom · montant -->
    <div class="flex items-start gap-2">
      <button
        type="button"
        class="grid size-[28px] shrink-0 place-items-center rounded-[8px] transition-colors duration-150"
        :style="{ background: `color-mix(in srgb, ${categoryColor(form.category)} 22%, transparent)`, color: categoryColor(form.category) }"
        :title="t('budget.form.icon')"
        @click="iconsOpen = !iconsOpen"
      >
        <component :is="iconOf(form.icon)" :size="15" weight="bold" />
      </button>
      <div class="relative min-w-0 flex-1">
        <TextField
          v-model="form.name"
          :placeholder="t('budget.form.name')"
          autofocus
          @update:model-value="nameFocused = true"
          @blur="nameFocused = false"
          @enter="valid && emit('save')"
        />
        <div
          v-if="nameFocused && suggestions.length"
          class="absolute inset-x-0 top-[30px] z-10 flex flex-col rounded-[10px] bg-elevated p-1 shadow-[0_8px_24px_rgb(0_0_0/0.4)]"
        >
          <button
            v-for="s in suggestions"
            :key="s.id"
            type="button"
            class="type-footnote flex h-7 items-center gap-2 rounded-[7px] px-2 text-left hover:bg-fill-2"
            @mousedown.prevent="pick(s)"
          >
            <component :is="iconOf(s.icon)" :size="13" weight="bold" :style="{ color: categoryColor(s.category) }" />
            <span class="flex-1 truncate">{{ s.name }}</span>
          </button>
        </div>
      </div>
      <div class="w-[110px] shrink-0">
        <TextField v-model="form.amount" :placeholder="t('budget.form.amount')" inputmode="decimal" @enter="valid && emit('save')" />
      </div>
    </div>

    <!-- Grille d'icônes -->
    <div v-if="iconsOpen" class="grid grid-cols-[repeat(auto-fill,minmax(28px,1fr))] gap-1 rounded-[10px] bg-fill-1 p-1.5">
      <button
        v-for="(icon, key) in ICONS"
        :key="key"
        type="button"
        class="grid size-7 place-items-center rounded-[7px] transition-colors duration-100"
        :class="form.icon === key ? 'bg-fill-3 text-label' : 'text-label-2 hover:bg-fill-2'"
        @click="(form.icon = key), (iconsOpen = false)"
      >
        <component :is="icon" :size="15" weight="bold" />
      </button>
    </div>

    <!-- Catégorie -->
    <div class="flex flex-wrap gap-1">
      <button
        v-for="c in categories"
        :key="c"
        type="button"
        class="type-caption flex h-6 items-center gap-1.5 rounded-full px-2.5 transition-colors duration-120"
        :class="form.category === c ? 'bg-fill-3 text-label' : 'bg-fill-1 text-label-2 hover:bg-fill-2'"
        @click="setCategory(c)"
      >
        <span class="size-1.5 rounded-full" :style="{ background: categoryColor(c) }" />
        {{ categoryLabel(c) }}
      </button>
      <button
        v-if="!creating"
        type="button"
        class="type-caption flex h-6 items-center gap-1 rounded-full px-2.5 text-label-3 transition-colors duration-120 hover:bg-fill-1 hover:text-label-2"
        @click="creating = true"
      >
        <PhPlus :size="10" weight="bold" />{{ t("budget.cat.new") }}
      </button>
    </div>
    <div v-if="creating" class="flex items-center gap-2 rounded-[10px] bg-fill-1 p-1.5">
      <div class="w-[160px]"><TextField v-model="newName" :placeholder="t('budget.cat.newName')" autofocus @enter="createCategory" /></div>
      <div class="flex gap-1">
        <button
          v-for="c in CATEGORY_PALETTE"
          :key="c"
          type="button"
          class="size-5 rounded-full transition-[box-shadow] duration-100"
          :class="newColor === c ? 'shadow-[0_0_0_2px_var(--label)]' : ''"
          :style="{ background: c }"
          @click="newColor = c"
        />
      </div>
      <span class="flex-1" />
      <Button size="sm" @click="creating = false">{{ t("budget.form.cancel") }}</Button>
      <Button size="sm" variant="default" :disabled="!newName.trim()" @click="createCategory">{{ t("budget.cat.create") }}</Button>
    </div>

    <!-- Date (dépense) ou échéance (récurrente) -->
    <div v-if="form.mode === 'expense'" class="flex items-center gap-2">
      <span class="type-footnote text-label-2">{{ t("budget.form.date") }}</span>
      <div class="w-[150px]"><TextField v-model="form.date" type="date" /></div>
    </div>
    <div v-else class="flex flex-wrap items-center gap-2">
      <Segmented
        v-model="form.schedule.kind"
        size="sm"
        :options="[
          { value: 'monthly', label: t('budget.sched.monthly') },
          { value: 'yearly', label: t('budget.sched.yearly') },
          { value: 'weekly', label: t('budget.sched.weekly') },
        ]"
      />
      <template v-if="form.schedule.kind !== 'weekly'">
        <span class="type-footnote text-label-2">{{ t("budget.sched.on") }}</span>
        <div class="w-[52px]"><TextField v-model="form.schedule.day" inputmode="numeric" placeholder="1" /></div>
        <div v-if="form.schedule.kind === 'yearly'" class="w-[120px]">
          <Select v-model="form.schedule.month" :options="MONTHS" />
        </div>
      </template>
      <Segmented v-else v-model="form.schedule.weekday" size="sm" :options="WEEKDAYS" />
    </div>

    <div class="mt-auto flex items-center gap-2">
      <Button v-if="form.id" variant="ghost" size="sm" class="text-sys-red" @click="emit('remove')"><PhTrash weight="bold" />{{ t("budget.form.delete") }}</Button>
      <span class="flex-1" />
      <Button size="sm" @click="emit('cancel')">{{ t("budget.form.cancel") }}</Button>
      <Button size="sm" variant="default" :disabled="!valid" @click="emit('save')">{{ t("budget.form.save") }}</Button>
    </div>
  </div>
</template>
