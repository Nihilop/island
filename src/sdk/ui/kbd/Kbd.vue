<script setup lang="ts">
import type { HTMLAttributes } from "vue"
import { cn } from "@/lib/utils"

const props = defineProps<{
  class?: HTMLAttributes["class"]
}>()
</script>

<template>
  <kbd
    data-slot="kbd"
    :class="cn('pointer-events-none inline-flex h-5 min-w-5 w-fit select-none items-center justify-center gap-1 rounded-[6px] bg-fill-2 px-1.5 font-sans text-[11px] font-medium text-label-2', props.class)"
  >
    <slot />
  </kbd>
</template>
