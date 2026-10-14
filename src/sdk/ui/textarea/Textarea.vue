<script setup lang="ts">
import type { HTMLAttributes } from 'vue'
import { useVModel } from '@vueuse/core'
import { cn } from '@/lib/utils'

const props = defineProps<{
  class?: HTMLAttributes['class']
  defaultValue?: string | number
  modelValue?: string | number
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
  <textarea
    v-model="modelValue"
    data-slot="textarea"
    :class="cn('w-full min-w-0 select-text resize-none rounded-md border-0 bg-fill-1 px-3 py-2.5 text-[13px] leading-[18px] text-foreground outline-none placeholder:text-label-4 focus-visible:ring-1 focus-visible:ring-fill-3 disabled:opacity-35', props.class)"
  />
</template>
