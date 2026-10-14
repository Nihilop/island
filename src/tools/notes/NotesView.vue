<script setup lang="ts">
// Outil « Note » : une seule note, enregistrée à chaque frappe. Pied : compteur de mots et
// « Enregistré » qui s'efface 1,2 s après la dernière frappe. Verrou Windows Hello en
// option (Réglages › Outils) : la note reste masquée jusqu'à la confirmation.
import { computed, nextTick, onMounted, ref, watch } from "vue";
import { PhFingerprint, PhLockSimple, PhLockSimpleOpen } from "@phosphor-icons/vue";
import { Textarea } from "@/sdk/ui/textarea";
import { Button } from "@/sdk/ui/button";
import Empty from "../../ui/Empty.vue";
import IconButton from "../../ui/IconButton.vue";
import { note as state } from "./store";
import { takeKeyboard } from "../../notch/state";
import { settings } from "../../notch/settings";
import { hello, helloError, isLocked, lockScope, touchScope, unlockScope } from "../../notch/hello";
import { t } from "../../notch/i18n";

const saved = ref(false);
const area = ref<{ $el: HTMLTextAreaElement }>();
const error = ref<string | null>(null);
let timer: number | undefined;

const locked = computed(() => isLocked("notes"));
const words = computed(() => (state.text.trim() ? state.text.trim().split(/\s+/).length : 0));

function onInput() {
  touchScope("notes");
  saved.value = true;
  window.clearTimeout(timer);
  timer = window.setTimeout(() => (saved.value = false), 1200);
}

function focusArea() {
  takeKeyboard();
  area.value?.$el?.focus();
}

async function unlock() {
  if (hello.prompting) return;
  error.value = null;
  try {
    await unlockScope("notes");
  } catch (e) {
    error.value = helloError(t, e);
  }
}

watch(locked, async (l) => {
  if (l) return;
  await nextTick();
  focusArea();
});

onMounted(() => {
  if (locked.value) void unlock();
  else focusArea();
});
</script>

<template>
  <div class="flex h-full flex-col gap-1.5 px-4 pb-1">
    <Teleport defer to="#notch-header-actions">
      <IconButton v-if="settings.hello.notes && !locked" :icon="PhLockSimpleOpen" active :label="t('notes.lock')" @click="lockScope('notes')" />
    </Teleport>

    <Empty v-if="locked" :icon="PhLockSimple" :title="t('notes.locked')" :text="error ?? t('notes.lockedHelp')">
      <Button variant="default" :loading="hello.prompting" @click="unlock"><PhFingerprint weight="bold" />{{ t("hello.unlock") }}</Button>
    </Empty>
    <template v-else>
      <Textarea
        ref="area"
        v-model="state.text"
        class="type-body flex-1 text-label focus-visible:ring-0 [scrollbar-color:var(--fill-3)_transparent] [scrollbar-width:thin]"
        :placeholder="t('notes.placeholder')"
        spellcheck="false"
        data-no-drag
        @input="onInput"
        @focus="takeKeyboard"
      />
      <div class="type-caption flex justify-between px-1">
        <span class="text-label-3">{{ t(words === 1 ? "notes.word" : "notes.words", { n: words }) }}</span>
        <Transition leave-active-class="transition-opacity duration-400 ease-(--ease-out)" leave-to-class="opacity-0">
          <span v-if="saved" class="text-label-3">{{ t("notes.saved") }}</span>
        </Transition>
      </div>
    </template>
  </div>
</template>
