<script setup lang="ts">
// Champ texte sur l'Input shadcn (28 px, fill-2). Mode secret avec bouton œil ; `reveal`
// l'affiche de l'extérieur (ex. mot de passe généré). Donne le focus clavier au notch.
import { ref, watch } from "vue";
import { PhEye, PhEyeSlash } from "@phosphor-icons/vue";
import { Input } from "@/sdk/ui/input";
import { cn } from "@/lib/utils";
import { takeKeyboard } from "../notch/state";

const props = defineProps<{
  placeholder?: string;
  secret?: boolean;
  reveal?: boolean;
  autofocus?: boolean;
  type?: string;
  inputmode?: "text" | "numeric" | "email" | "decimal";
  autocomplete?: string;
}>();
const model = defineModel<string>({ required: true });
const emit = defineEmits<{ enter: []; blur: [] }>();

const shown = ref(!!props.reveal);
watch(
  () => props.reveal,
  (r) => (shown.value = !!r),
);
const input = ref<{ $el: HTMLInputElement }>();

defineExpose({ focus: () => input.value?.$el?.focus() });
</script>

<template>
  <div class="relative flex items-center" data-no-drag>
    <Input
      ref="input"
      v-model="model"
      :class="cn(secret && 'pr-7')"
      :type="secret && !shown ? 'password' : (type ?? 'text')"
      :placeholder="placeholder"
      :autofocus="autofocus"
      :inputmode="inputmode"
      :autocomplete="autocomplete ?? 'off'"
      spellcheck="false"
      @focus="takeKeyboard"
      @keydown.enter="emit('enter')"
      @blur="emit('blur')"
    />
    <button
      v-if="secret"
      type="button"
      tabindex="-1"
      class="absolute right-0 grid size-7 place-items-center text-label-3 hover:text-label-2"
      @click="shown = !shown"
    >
      <PhEyeSlash v-if="shown" :size="14" weight="bold" />
      <PhEye v-else :size="14" weight="bold" />
    </button>
  </div>
</template>
