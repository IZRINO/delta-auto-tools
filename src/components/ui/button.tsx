import * as React from "react"
import { cva, type VariantProps } from "class-variance-authority"
import { Slot } from "radix-ui"

import { cn } from "@/lib/utils"

const buttonVariants = cva(
  "group/button inline-flex shrink-0 items-center justify-center border-2 border-[var(--ink)] bg-clip-padding font-mono text-xs font-black tracking-[0.12em] whitespace-nowrap uppercase transition-colors outline-none select-none focus-visible:outline-2 focus-visible:outline-offset-[-2px] focus-visible:outline-[var(--alert-red)] active:not-aria-[haspopup]:bg-[var(--ink)] active:not-aria-[haspopup]:text-[var(--paper)] disabled:pointer-events-none disabled:opacity-45 aria-invalid:border-[var(--alert-red)] aria-invalid:outline-2 aria-invalid:outline-[var(--alert-red)] [&_svg]:pointer-events-none [&_svg]:shrink-0 [&_svg:not([class*='size-'])]:size-4",
  {
    variants: {
      variant: {
        default: "bg-[var(--ink)] text-[var(--paper)] hover:bg-[var(--alert-red)] hover:text-[var(--paper)]",
        outline: "bg-[var(--paper)] text-[var(--ink)] hover:bg-[var(--ink)] hover:text-[var(--paper)] aria-expanded:bg-[var(--ink)] aria-expanded:text-[var(--paper)]",
        secondary: "bg-[var(--bone)] text-[var(--ink)] hover:bg-[var(--ink)] hover:text-[var(--paper)] aria-expanded:bg-[var(--ink)] aria-expanded:text-[var(--paper)]",
        ghost: "border-transparent bg-transparent text-[var(--ink)] hover:border-[var(--ink)] hover:bg-[var(--bone)] aria-expanded:border-[var(--ink)] aria-expanded:bg-[var(--bone)]",
        destructive: "border-[var(--alert-red)] bg-[var(--paper)] text-[var(--alert-red)] hover:bg-[var(--alert-red)] hover:text-[var(--paper)] focus-visible:outline-[var(--alert-red)]",
        link: "border-transparent bg-transparent px-0 text-[var(--alert-red)] underline-offset-4 hover:underline",
      },
      size: {
        default: "h-8 gap-1.5 px-2.5 has-data-[icon=inline-end]:pr-2 has-data-[icon=inline-start]:pl-2",
        xs: "h-6 gap-1 px-2 text-[0.62rem] has-data-[icon=inline-end]:pr-1.5 has-data-[icon=inline-start]:pl-1.5 [&_svg:not([class*='size-'])]:size-3",
        sm: "h-7 gap-1 px-2.5 has-data-[icon=inline-end]:pr-1.5 has-data-[icon=inline-start]:pl-1.5 [&_svg:not([class*='size-'])]:size-3.5",
        lg: "h-9 gap-1.5 px-3 has-data-[icon=inline-end]:pr-2 has-data-[icon=inline-start]:pl-2",
        icon: "size-8",
        "icon-xs": "size-6 [&_svg:not([class*='size-'])]:size-3",
        "icon-sm": "size-7",
        "icon-lg": "size-9",
      },
    },
    defaultVariants: {
      variant: "default",
      size: "default",
    },
  }
)

function Button({
  className,
  variant = "default",
  size = "default",
  asChild = false,
  ...props
}: React.ComponentProps<"button"> &
  VariantProps<typeof buttonVariants> & {
    asChild?: boolean
  }) {
  const Comp = asChild ? Slot.Root : "button"

  return (
    <Comp
      data-slot="button"
      data-variant={variant}
      data-size={size}
      className={cn(buttonVariants({ variant, size, className }))}
      {...props}
    />
  )
}

export { Button, buttonVariants }
