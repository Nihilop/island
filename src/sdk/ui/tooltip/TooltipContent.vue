<script setup lang="ts">
import type { TooltipContentEmits, TooltipContentProps } from 'reka-ui'
import type { HTMLAttributes } from 'vue'
import { reactiveOmit } from '@vueuse/core'
import { TooltipContent, TooltipPortal, useForwardPropsEmits } from 'reka-ui'
import { cn } from '@/lib/utils'

defineOptions({
  inheritAttrs: false,
})

const props = withDefaults(defineProps<TooltipContentProps & { class?: HTMLAttributes['class'] }>(), {
  sideOffset: 0,
})

const emits = defineEmits<TooltipContentEmits>()

const delegatedProps = reactiveOmit(props, 'class')
const forwarded = useForwardPropsEmits(delegatedProps, emits)
</script>

<template>
  <TooltipPortal>
    <TooltipContent
      data-slot="tooltip-content"
      v-bind="{ ...forwarded, ...$attrs }"
      :class="cn('z-50 w-fit max-w-xs origin-(--reka-tooltip-content-transform-origin) rounded-[10px] bg-popover px-2.5 py-1 text-[11px] text-label-2 shadow-[0_6px_18px_rgb(0_0_0/0.45)] ring-1 ring-separator data-open:animate-in data-open:fade-in-0 data-open:zoom-in-95 data-closed:animate-out data-closed:fade-out-0', props.class)"
    >
      <slot />
    </TooltipContent>
  </TooltipPortal>
</template>
