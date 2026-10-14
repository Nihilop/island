<script setup lang="ts">
import type { HTMLAttributes } from 'vue'
import { cn } from '@/lib/utils'

const props = defineProps<{
  class?: HTMLAttributes['class']
}>()
</script>

<template>
  <p
    data-slot="item-description"
    :class="cn(
      'line-clamp-1 text-[11px] leading-[14px] text-label-3',
      props.class,
    )"
  >
    <slot />
  </p>
</template>
