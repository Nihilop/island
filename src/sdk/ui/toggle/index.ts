import type { VariantProps } from 'class-variance-authority'
import { cva } from 'class-variance-authority'

export { default as Toggle } from './Toggle.vue'

export const toggleVariants = cva(
  'inline-flex items-center justify-center gap-1 whitespace-nowrap rounded-full font-medium text-label-2 outline-none transition-colors duration-150 hover:text-label data-[state=on]:text-label disabled:pointer-events-none disabled:opacity-35 [&_svg]:pointer-events-none [&_svg]:shrink-0',
  {
    variants: {
      variant: {
        default: 'bg-transparent',
        outline: 'border border-fill-3 bg-transparent',
      },
      size: {
        default: 'h-6 px-3 text-[11px]',
        sm: 'h-5 px-2 text-[11px]',
        lg: 'h-7 px-3.5 text-xs',
      },
    },
    defaultVariants: {
      variant: 'default',
      size: 'default',
    },
  },
)

export type ToggleVariants = VariantProps<typeof toggleVariants>
