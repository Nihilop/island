<script setup lang="ts">
import type { HTMLAttributes } from 'vue'
import { cn } from '@/lib/utils'

defineProps<{
  class?: HTMLAttributes['class']
}>()
</script>

<template>
  <p
    data-slot="empty-description"
    :class="cn(
      'max-w-72 text-[11px] leading-[14px] text-label-3 [&>a]:underline',
      $attrs.class ?? '',
    )"
  >
    <slot />
  </p>
</template>
