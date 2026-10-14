<script setup lang="ts">
import type { HTMLAttributes } from 'vue'
import { cn } from '@/lib/utils'

const props = defineProps<{
  class?: HTMLAttributes['class']
}>()
</script>

<template>
  <div
    data-slot="empty"
    :class="cn(
      'flex h-full w-full min-w-0 flex-1 flex-col items-center justify-center gap-1 px-6 text-center',
      props.class,
    )"
  >
    <slot />
  </div>
</template>
