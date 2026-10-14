<script setup lang="ts">
import type { SwitchRootEmits, SwitchRootProps } from "reka-ui"
import type { HTMLAttributes } from "vue"
import { reactiveOmit } from "@vueuse/core"
import {
  SwitchRoot,
  SwitchThumb,
  useForwardPropsEmits,
} from "reka-ui"
import { cn } from "@/lib/utils"

const props = withDefaults(defineProps<SwitchRootProps & {
  class?: HTMLAttributes["class"]
  size?: "sm" | "default"
}>(), {
  size: "default",
})

const emits = defineEmits<SwitchRootEmits>()

const delegatedProps = reactiveOmit(props, "class", "size")

const forwarded = useForwardPropsEmits(delegatedProps, emits)
</script>

<template>
  <SwitchRoot
    v-slot="slotProps"
    data-slot="switch"
    :data-size="size"
    v-bind="forwarded"
    :class="cn(
      'peer inline-flex h-[18px] w-[30px] shrink-0 items-center rounded-full p-0.5 outline-none transition-colors duration-200 data-[state=checked]:bg-sys-green data-[state=unchecked]:bg-fill-3 focus-visible:ring-2 focus-visible:ring-ring data-disabled:pointer-events-none data-disabled:opacity-35',
      props.class,
    )"
  >
    <SwitchThumb
      data-slot="switch-thumb"
      class="pointer-events-none block size-3.5 rounded-full bg-white shadow-[0_1px_3px_rgb(0_0_0/0.5)] transition-transform duration-200 ease-out data-[state=checked]:translate-x-3 data-[state=unchecked]:translate-x-0"
    >
      <slot name="thumb" v-bind="slotProps" />
    </SwitchThumb>
  </SwitchRoot>
</template>
