<script setup lang="ts">
// Outil « Ask » (560 × 440) : chat rapide. Vide : actions rapides sur le dernier texte du
// presse-papiers. Messages : utilisateur à droite (bulle), assistant à gauche (texte),
// curseur pendant le flux, « Copier » au survol. Entrée envoie, Maj+Entrée = retour ligne.
import { useDraft } from "../../stores/drafts";
import { computed, nextTick, onMounted, ref, watch } from "vue";
import { PhArrowUp, PhCheck, PhCopy, PhSparkle, PhStop, PhTrash } from "@phosphor-icons/vue";
import IconButton from "../../ui/IconButton.vue";
import { Button } from "@/sdk/ui/button";
import { Textarea } from "@/sdk/ui/textarea";
import { chat, clear, current, error, provider, send, stop, streaming } from "./store";
import { entries } from "../clipboard/store";
import { openSettings } from "../settings/nav";
import { takeKeyboard } from "../../notch/state";
import { t } from "../../notch/i18n";

const draft = useDraft("ask.input", () => "");
const input = ref<{ $el: HTMLTextAreaElement }>();
const list = ref<HTMLElement>();
const copiedIdx = ref<number | null>(null);

const label = computed(() => current.value.model || t(`ask.provider.${provider.value}`));
const lastText = computed(() => entries.value.find((e) => e.kind === "text" || e.kind === "link")?.preview ?? null);

function submit() {
  if (streaming.value) return;
  send(draft.value);
  draft.value = "";
  void nextTick(resize);
}

function onKey(e: KeyboardEvent) {
  if (e.key === "Enter" && !e.shiftKey) {
    e.preventDefault();
    submit();
  }
}

function resize() {
  const el = input.value?.$el;
  if (!el) return;
  el.style.height = "auto";
  el.style.height = `${Math.min(54, el.scrollHeight)}px`;
}

function quick(kind: "summarize" | "rephrase" | "translate") {
  if (!lastText.value) return;
  send(`${t(`ask.quick.${kind}.prompt`)}\n\n${lastText.value}`);
}

async function copyMsg(i: number) {
  await navigator.clipboard.writeText(chat.messages[i].content).catch(() => {});
  copiedIdx.value = i;
  window.setTimeout(() => (copiedIdx.value = null), 1200);
}

// Défilement automatique en bas.
watch(
  () => chat.messages.map((m) => m.content.length).join(),
  () => nextTick(() => list.value?.scrollTo({ top: list.value.scrollHeight })),
);

onMounted(() => {
  takeKeyboard();
  input.value?.$el?.focus();
});
</script>

<template>
  <div class="flex h-full flex-col gap-2 px-4 pb-1">
    <Teleport defer to="#notch-header-actions">
      <Button
        variant="ghost"
        size="sm"
        class="type-caption inline-block h-[22px] max-w-40 truncate px-2 font-normal active:scale-100"
        @click="openSettings('ask')"
      >
        {{ label }}
      </Button>
      <IconButton :icon="PhTrash" :label="t('ask.clear')" :disabled="!chat.messages.length" @click="clear" />
    </Teleport>

    <div ref="list" class="flex min-h-0 flex-1 flex-col gap-2.5 overflow-y-auto [scrollbar-color:var(--fill-3)_transparent] [scrollbar-width:thin]">
      <div v-if="!chat.messages.length" class="m-auto flex flex-col items-center gap-2.5 text-center">
        <PhSparkle :size="22" weight="duotone" class="text-label-3" />
        <div class="type-body text-label-2">{{ t("ask.empty") }}</div>
        <div class="flex flex-wrap justify-center gap-2">
          <Button
            v-for="k in ['summarize', 'rephrase', 'translate'] as const"
            :key="k"
            variant="secondary"
            class="type-footnote font-normal"
            :disabled="!lastText"
            @click="quick(k)"
          >
            {{ t(`ask.quick.${k}`) }}
          </Button>
        </div>
        <div v-if="!lastText" class="type-caption text-label-3">{{ t("ask.copyFirst") }}</div>
      </div>
      <template v-for="(m, i) in chat.messages" :key="i">
        <div
          v-if="m.role === 'user'"
          class="type-body max-w-[85%] select-text self-end whitespace-pre-wrap rounded-[14px_14px_4px_14px] bg-fill-2 px-3 py-2"
        >{{ m.content }}</div>
        <div v-else class="group/msg max-w-full">
          <div class="type-body select-text whitespace-pre-wrap text-label-2">{{ m.content }}<span v-if="streaming && i === chat.messages.length - 1" class="ml-0.5 inline-block h-[1em] w-0.5 animate-blink bg-label align-text-bottom" /></div>
          <button
            v-if="m.content && !(streaming && i === chat.messages.length - 1)"
            type="button"
            class="type-caption mt-1 inline-flex items-center gap-1 text-label-3 opacity-0 transition-opacity duration-160 ease-(--ease-out) group-hover/msg:opacity-100"
            @click="copyMsg(i)"
          >
            <component :is="copiedIdx === i ? PhCheck : PhCopy" :size="11" weight="bold" />
            {{ copiedIdx === i ? t("clip.copied") : t("clip.copy") }}
          </button>
        </div>
      </template>
      <div v-if="error" class="type-caption text-sys-orange">
        {{ error.startsWith("noKey") ? t("ask.err.noKey", { p: t(`ask.provider.${provider}`) }) : t(`ask.err.${error.split(":")[0]}`, { msg: error.split(":").slice(1).join(":") }) }}
        <button type="button" class="underline" @click="openSettings('ask')">{{ t("ask.settings") }}</button>
      </div>
    </div>

    <div class="flex items-end gap-2 rounded-2xl bg-fill-1 py-1.5 pl-3 pr-1.5" data-no-drag>
      <Textarea
        ref="input"
        v-model="draft"
        rows="1"
        class="type-body max-h-[54px] flex-1 rounded-none bg-transparent p-0 text-label focus-visible:ring-0"
        :placeholder="t('ask.placeholder')"
        @keydown="onKey"
        @input="resize"
        @focus="takeKeyboard"
      />
      <Button v-if="streaming" variant="default" size="icon-sm" class="bg-label text-notch hover:bg-label" @click="stop">
        <PhStop :size="12" weight="fill" />
      </Button>
      <Button
        v-else
        variant="default"
        size="icon-sm"
        class="bg-label text-notch hover:bg-label"
        :class="{ 'opacity-25': !draft.trim() }"
        @click="submit"
      >
        <PhArrowUp :size="12" weight="bold" />
      </Button>
    </div>
  </div>
</template>
