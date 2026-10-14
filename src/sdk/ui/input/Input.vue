<script setup lang="ts">
import type { HTMLAttributes } from 'vue'
import { useVModel } from '@vueuse/core'
import { cn } from '@/lib/utils'

const props = defineProps<{
  defaultValue?: string | number
  modelValue?: string | number
  class?: HTMLAttributes['class']
}>()

const emits = defineEmits<{
  (e: 'update:modelValue', payload: string | number): void
}>()

const modelValue = useVModel(props, 'modelValue', emits, {
  passive: true,
  defaultValue: props.defaultValue,
})
</script>

<template>
  <!-- [&::-ms-reveal]/[&::-ms-clear] : masque l'œil et la croix natifs d'Edge/WebView2 (doublon avec le bouton de TextField) -->
  <input
    v-model="modelValue"
    data-slot="input"
    :class="cn(
      'h-7 w-full min-w-0 select-text rounded-sm border-0 bg-fill-2 px-2.5 text-xs text-foreground outline-none transition-shadow placeholder:text-label-4 focus-visible:ring-1 focus-visible:ring-fill-3 disabled:pointer-events-none disabled:opacity-35 [&::-ms-clear]:hidden [&::-ms-reveal]:hidden',
      props.class,
    )"
  >
</template>
