<script setup lang="ts">
import { CheckIcon } from '@lucide/vue';

import type { SelectItemProps } from "reka-ui"
import type { HTMLAttributes } from "vue"
import { reactiveOmit } from "@vueuse/core"
import {
  SelectItem,
  SelectItemIndicator,
  SelectItemText,
  useForwardProps,
} from "reka-ui"
import { cn } from "@/lib/utils"

const props = defineProps<SelectItemProps & { class?: HTMLAttributes["class"] }>()

const delegatedProps = reactiveOmit(props, "class")

const forwardedProps = useForwardProps(delegatedProps)
</script>

<template>
  <SelectItem
    data-slot="select-item"
    v-bind="forwardedProps"
    :class="
      cn(
        'relative flex h-7 w-full cursor-default select-none items-center gap-2 rounded-[6px] pl-2 pr-7 text-xs outline-hidden focus:bg-fill-2 data-[state=checked]:font-medium data-disabled:pointer-events-none data-disabled:opacity-35 [&_svg]:pointer-events-none [&_svg]:shrink-0 [&_svg:not([class*=size-])]:size-3.5',
        props.class,
      )
    "
  >
    <span class="pointer-events-none absolute right-2 flex size-4 items-center justify-center">
      <SelectItemIndicator>
        <slot name="indicator-icon">
          <CheckIcon class="pointer-events-none" />
        </slot>
      </SelectItemIndicator>
    </span>

    <SelectItemText>
      <slot />
    </SelectItemText>
  </SelectItem>
</template>
