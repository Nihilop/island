import type { VariantProps } from 'class-variance-authority'
import { cva } from 'class-variance-authority'

export { default as Item } from './Item.vue'
export { default as ItemActions } from './ItemActions.vue'
export { default as ItemContent } from './ItemContent.vue'
export { default as ItemDescription } from './ItemDescription.vue'
export { default as ItemFooter } from './ItemFooter.vue'
export { default as ItemGroup } from './ItemGroup.vue'
export { default as ItemHeader } from './ItemHeader.vue'
export { default as ItemMedia } from './ItemMedia.vue'
export { default as ItemSeparator } from './ItemSeparator.vue'
export { default as ItemTitle } from './ItemTitle.vue'

export const itemVariants = cva(
  'group/item flex w-full items-center rounded-[10px] text-[13px] outline-none transition-colors duration-150 [a]:hover:bg-fill-1 hover:bg-fill-1 focus-visible:ring-1 focus-visible:ring-fill-3',
  {
    variants: {
      variant: {
        default: '',
        outline: 'border border-separator',
        muted: 'bg-fill-1',
      },
      size: {
        default: 'h-11 gap-2.5 px-2',
        sm: 'h-8 gap-2.5 px-2',
        xs: 'h-7 gap-2 px-2',
      },
    },
    defaultVariants: {
      variant: 'default',
      size: 'default',
    },
  },
)

export const itemMediaVariants = cva(
  'flex shrink-0 items-center justify-center [&_svg]:pointer-events-none',
  {
    variants: {
      variant: {
        default: 'bg-transparent',
        icon: 'size-7 rounded-sm bg-fill-2 text-[12px] font-semibold text-label-2 [&_svg:not([class*=size-])]:size-4',
        image: 'size-7 overflow-hidden rounded-sm [&_img]:size-full [&_img]:object-cover',
      },
    },
    defaultVariants: {
      variant: 'default',
    },
  },
)

export type ItemVariants = VariantProps<typeof itemVariants>
export type ItemMediaVariants = VariantProps<typeof itemMediaVariants>
