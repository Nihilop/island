<script setup lang="ts">
// Outil « To-dos » (520 × 276) : ajout en tête (échéance facultative via le calendrier du
// champ), case ronde, étoile, échéance et suppression au survol. Tri : non faites d'abord
// (marquées, puis échéance la plus proche, puis ordre de saisie), faites en bas. Persisté.
import { useDraft } from "../../stores/drafts";
import { computed } from "vue";
import { PhCalendarBlank, PhCheck, PhPlus, PhStar, PhX } from "@phosphor-icons/vue";
import { Button } from "@/sdk/ui/button";
import { Input } from "@/sdk/ui/input";
import DuePicker from "./DuePicker.vue";
import { dueLabel, overdue, setDue, today, todos as state, type Due, type Todo } from "./store";
import { takeKeyboard } from "../../notch/state";
import { t } from "../../notch/i18n";
const draft = useDraft("todos.input", () => "");
const draftDue = useDraft<Due | null>("todos.due", () => null);

/** Clé de tri d'échéance : sans heure en fin de journée, sans échéance tout en bas. */
const dueKey = (it: Todo) => (it.due ? `${it.due}T${it.time ?? "24:00"}` : "￿");

const sorted = computed(() =>
  state.items
    .map((it, i) => ({ it, i }))
    .sort(
      (a, b) =>
        Number(a.it.done) - Number(b.it.done) ||
        (a.it.done ? 0 : Number(b.it.starred) - Number(a.it.starred) || dueKey(a.it).localeCompare(dueKey(b.it))) ||
        a.i - b.i,
    )
    .map((x) => x.it),
);
const open = computed(() => state.items.filter((i) => !i.done).length);

function add() {
  const text = draft.value.trim();
  if (!text) return;
  const it: Todo = { id: crypto.randomUUID(), text, done: false, starred: false };
  setDue(it, draftDue.value);
  state.items.unshift(it);
  draft.value = "";
  draftDue.value = null;
}
const remove = (id: string) => (state.items = state.items.filter((i) => i.id !== id));

const dueOf = (it: Todo): Due | null => (it.due ? { due: it.due, time: it.time } : null);

/** Couleur de la pastille : rouge en retard, orange aujourd'hui (tâches non faites). */
function tone(d: Due, done = false) {
  if (done) return "bg-fill-1 text-label-4";
  if (overdue(d)) return "bg-sys-red/15 text-sys-red";
  if (d.due === today()) return "bg-sys-orange/15 text-sys-orange";
  return "bg-fill-2 text-label-2";
}
const pill = "type-caption flex h-5 shrink-0 items-center rounded-full px-2 whitespace-nowrap tabular-nums transition-opacity duration-150 hover:opacity-80";
</script>

<template>
  <div class="flex h-full flex-col gap-2 px-4">
    <label class="flex h-9 items-center gap-2 rounded-md bg-fill-1 pr-1.5 pl-3" data-no-drag>
      <PhPlus :size="14" weight="bold" class="text-label-3" />
      <Input
        v-model="draft"
        class="type-body h-auto flex-1 rounded-none bg-transparent p-0 text-[13px] leading-[18px] text-label focus-visible:ring-0"
        :placeholder="t('todos.placeholder')"
        @focus="takeKeyboard"
        @keydown.enter="add"
      />
      <span v-if="open" class="type-caption shrink-0 text-label-3">{{ t("todos.open", { n: open }) }}</span>
      <DuePicker v-model="draftDue">
        <button v-if="draftDue" type="button" :class="[pill, tone(draftDue)]">{{ dueLabel(draftDue) }}</button>
        <Button v-else variant="ghost" size="icon-sm" class="text-label-3 hover:text-label-2" :title="t('todos.due.set')">
          <PhCalendarBlank :size="15" weight="bold" />
        </Button>
      </DuePicker>
    </label>

    <div class="min-h-0 flex-1 overflow-y-auto [scrollbar-color:var(--fill-3)_transparent] [scrollbar-width:thin]">
      <div v-if="!state.items.length" class="type-body py-6 text-center text-label-3">{{ t("todos.empty") }}</div>
      <TransitionGroup
        tag="div"
        move-class="transition-transform duration-240 ease-(--ease-out)"
        enter-active-class="overflow-hidden transition-[height,opacity] duration-150 ease-(--ease-out)"
        enter-from-class="h-0! opacity-0"
        leave-active-class="overflow-hidden transition-[height,opacity] duration-150 ease-(--ease-out)"
        leave-to-class="h-0! opacity-0"
      >
        <div v-for="it in sorted" :key="it.id" class="group/row flex h-8 items-center gap-2.5 rounded-[10px] px-2 hover:bg-fill-1">
          <button
            type="button"
            class="grid size-[18px] shrink-0 place-items-center rounded-full border-[1.5px] text-black transition-[background-color,border-color] duration-160 ease-(--ease-out)"
            :class="it.done ? 'border-sys-green bg-sys-green' : 'border-label-3'"
            @click="it.done = !it.done"
          >
            <Transition
              enter-active-class="transition-transform duration-240 ease-[cubic-bezier(0.34,1.56,0.64,1)]"
              enter-from-class="scale-50"
            >
              <PhCheck v-if="it.done" :size="11" weight="bold" />
            </Transition>
          </button>
          <span class="type-body flex-1 truncate" :class="{ 'text-label-3 line-through': it.done }">{{ it.text }}</span>
          <DuePicker :model-value="dueOf(it)" @update:model-value="(d) => setDue(it, d)">
            <template #default="{ open: picking }">
              <button v-if="it.due" type="button" :class="[pill, tone(dueOf(it)!, it.done)]">{{ dueLabel(dueOf(it)!) }}</button>
              <Button
                v-else
                variant="ghost"
                size="icon-xs"
                class="size-[22px] text-label-3 transition-opacity duration-160 ease-(--ease-out) hover:bg-transparent hover:text-label-2 active:scale-100 group-hover/row:opacity-100"
                :class="picking ? 'opacity-100' : 'opacity-0'"
                :title="t('todos.due.set')"
              >
                <PhCalendarBlank :size="13" weight="bold" />
              </Button>
            </template>
          </DuePicker>
          <Button
            variant="ghost"
            size="icon-xs"
            class="size-[22px] transition-opacity duration-160 ease-(--ease-out) hover:bg-transparent active:scale-100 group-hover/row:opacity-100"
            :class="it.starred ? 'text-sys-yellow opacity-100 hover:text-sys-yellow' : 'text-label-3 opacity-0 hover:text-label-3'"
            @click="it.starred = !it.starred"
          >
            <PhStar :size="13" :weight="it.starred ? 'fill' : 'bold'" class="size-[13px]" />
          </Button>
          <Button
            variant="ghost"
            size="icon-xs"
            class="size-[22px] text-label-3 opacity-0 transition-opacity duration-160 ease-(--ease-out) hover:bg-transparent hover:text-label-3 active:scale-100 group-hover/row:opacity-100"
            @click="remove(it.id)"
          >
            <PhX :size="12" weight="bold" />
          </Button>
        </div>
      </TransitionGroup>
    </div>
  </div>
</template>
