import type { VariantProps } from 'class-variance-authority'
import { cva } from 'class-variance-authority'

export { default as Badge } from './Badge.vue'

export const badgeVariants = cva(
  'inline-flex h-4 w-fit shrink-0 items-center justify-center gap-1 whitespace-nowrap rounded-full px-1.5 text-[11px] font-medium [&>svg]:pointer-events-none [&>svg]:size-3!',
  {
    variants: {
      variant: {
        default: 'bg-fill-2 text-label-2',
        secondary: 'bg-fill-1 text-label-3',
        link: 'bg-sys-link/20 text-sys-link',
        destructive: 'bg-destructive text-white',
        outline: 'border border-fill-3 text-label-2',
        ghost: 'text-label-3',
      },
    },
    defaultVariants: {
      variant: 'default',
    },
  },
)
export type BadgeVariants = VariantProps<typeof badgeVariants>
