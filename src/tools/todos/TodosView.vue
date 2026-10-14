<script setup lang="ts">
// Outil « To-dos » (520 × 276) : ajout en tête, case ronde, étoile, suppression au
// survol. Tri : marquées en haut, faites en bas, sinon ordre de saisie. Persisté.
import { useDraft } from "../../stores/drafts";
import { computed } from "vue";
import { PhCheck, PhPlus, PhStar, PhX } from "@phosphor-icons/vue";
import { Button } from "@/sdk/ui/button";
import { Input } from "@/sdk/ui/input";
import { todos as state } from "./store";
import { takeKeyboard } from "../../notch/state";
import { t } from "../../notch/i18n";
const draft = useDraft("todos.input", () => "");

const sorted = computed(() =>
  state.items
    .map((it, i) => ({ it, i }))
    .sort((a, b) => Number(a.it.done) - Number(b.it.done) || Number(b.it.starred) - Number(a.it.starred) || a.i - b.i)
    .map((x) => x.it),
);
const open = computed(() => state.items.filter((i) => !i.done).length);

function add() {
  const text = draft.value.trim();
  if (!text) return;
  state.items.unshift({ id: crypto.randomUUID(), text, done: false, starred: false });
  draft.value = "";
}
const remove = (id: string) => (state.items = state.items.filter((i) => i.id !== id));
</script>

<template>
  <div class="flex h-full flex-col gap-2 px-4">
    <label class="flex h-9 items-center gap-2 rounded-md bg-fill-1 px-3" data-no-drag>
      <PhPlus :size="14" weight="bold" class="text-label-3" />
      <Input
        v-model="draft"
        class="type-body h-auto flex-1 rounded-none bg-transparent p-0 text-[13px] leading-[18px] text-label focus-visible:ring-0"
        :placeholder="t('todos.placeholder')"
        @focus="takeKeyboard"
        @keydown.enter="add"
      />
      <span v-if="open" class="type-caption shrink-0 text-label-3">{{ t("todos.open", { n: open }) }}</span>
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
