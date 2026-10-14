<script setup lang="ts">
import { ChevronDownIcon } from '@lucide/vue';

import type { SelectTriggerProps } from "reka-ui"
import type { HTMLAttributes } from "vue"
import { reactiveOmit } from "@vueuse/core"
import { SelectIcon, SelectTrigger, useForwardProps } from "reka-ui"
import { cn } from "@/lib/utils"

const props = withDefaults(
  defineProps<SelectTriggerProps & { class?: HTMLAttributes["class"], size?: "sm" | "default" }>(),
  { size: "default" },
)

const delegatedProps = reactiveOmit(props, "class", "size")
const forwardedProps = useForwardProps(delegatedProps)
</script>

<template>
  <SelectTrigger
    data-slot="select-trigger"
    :data-size="size"
    v-bind="forwardedProps"
    :class="cn(
      'flex h-7 w-fit items-center justify-between gap-1.5 whitespace-nowrap rounded-sm bg-fill-2 px-2.5 text-xs text-foreground outline-none transition-colors hover:bg-fill-3 focus-visible:ring-1 focus-visible:ring-fill-3 disabled:opacity-35 data-placeholder:text-label-3 *:data-[slot=select-value]:line-clamp-1 [&_svg]:pointer-events-none [&_svg]:shrink-0 [&_svg:not([class*=size-])]:size-3.5',
      props.class,
    )"
  >
    <slot />
    <SelectIcon as-child>
      <ChevronDownIcon class="text-muted-foreground size-4 pointer-events-none" />
    </SelectIcon>
  </SelectTrigger>
</template>
