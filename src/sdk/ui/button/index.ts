import type { VariantProps } from "class-variance-authority"
import { cva } from "class-variance-authority"

export { default as Button } from "./Button.vue"

// Bouton notch : pilule, retour d'appui (scale 0,94), désactivé à 35 %.
//   default = blanc (primaire), secondary = fill-2, ghost = icône discrète (header),
//   destructive = rouge système, outline = contour fill-3.
export const buttonVariants = cva(
  "group/button inline-flex shrink-0 select-none items-center justify-center gap-1.5 whitespace-nowrap rounded-full font-medium outline-none transition-[background-color,color,transform,opacity] duration-150 ease-out active:scale-[0.94] disabled:pointer-events-none disabled:opacity-35 focus-visible:ring-2 focus-visible:ring-ring [&_svg]:pointer-events-none [&_svg]:shrink-0",
  {
    variants: {
      variant: {
        default: "bg-primary text-primary-foreground hover:bg-primary/90",
        secondary: "bg-secondary text-secondary-foreground hover:bg-fill-3",
        ghost: "text-label-3 hover:bg-fill-2 hover:text-label-2 aria-pressed:bg-fill-3 aria-pressed:text-label data-[active=true]:bg-fill-3 data-[active=true]:text-label",
        destructive: "bg-destructive text-white hover:bg-destructive/90",
        outline: "border border-fill-3 bg-transparent text-label hover:bg-fill-1",
        link: "text-sys-link underline-offset-2 hover:underline",
      },
      size: {
        default: "h-7 px-3 text-xs [&_svg:not([class*=size-]):not([width])]:size-3.5",
        sm: "h-6 px-2.5 text-[11px] [&_svg:not([class*=size-]):not([width])]:size-3",
        lg: "h-9 px-4 text-[13px] [&_svg:not([class*=size-]):not([width])]:size-4",
        icon: "size-7 [&_svg:not([class*=size-]):not([width])]:size-3.5",
        "icon-xs": "size-5 [&_svg:not([class*=size-]):not([width])]:size-3",
        "icon-sm": "size-6 [&_svg:not([class*=size-]):not([width])]:size-3",
        "icon-lg": "size-10 [&_svg:not([class*=size-]):not([width])]:size-5",
        "icon-xl": "size-11 [&_svg:not([class*=size-]):not([width])]:size-5",
      },
    },
    defaultVariants: {
      variant: "secondary",
      size: "default",
    },
  },
)
export type ButtonVariants = VariantProps<typeof buttonVariants>
