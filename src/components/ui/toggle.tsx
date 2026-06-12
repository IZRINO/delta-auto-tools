import * as React from "react"
import { cva, type VariantProps } from "class-variance-authority"
import { Toggle as TogglePrimitive } from "radix-ui"

import { cn } from "@/lib/utils"

const toggleVariants = cva(
  "group/toggle inline-flex items-center justify-center gap-1 border-2 border-transparent font-mono text-xs font-black tracking-[0.12em] whitespace-nowrap uppercase transition-colors outline-none hover:border-[var(--chalk)] hover:bg-[var(--slate)] focus-visible:outline-2 focus-visible:outline-offset-[-2px] focus-visible:outline-[var(--amber)] disabled:pointer-events-none disabled:opacity-50 aria-invalid:border-[var(--amber)] aria-pressed:border-[var(--chalk)] aria-pressed:bg-[var(--chalk)] aria-pressed:text-[var(--carbon)] data-[state=on]:border-[var(--chalk)] data-[state=on]:bg-[var(--chalk)] data-[state=on]:text-[var(--carbon)] [&_svg]:pointer-events-none [&_svg]:shrink-0 [&_svg:not([class*='size-'])]:size-4",
  {
    variants: {
      variant: {
        default: "bg-[var(--carbon)]",
        outline: "border-[var(--chalk)] bg-[var(--carbon)] hover:bg-[var(--slate)]",
      },
      size: {
        default: "h-7 min-w-7 px-2 has-data-[icon=inline-end]:pe-1.5 has-data-[icon=inline-start]:ps-1.5",
        sm: "h-6 min-w-6 px-2 text-[0.625rem] has-data-[icon=inline-end]:pe-1.5 has-data-[icon=inline-start]:ps-1.5 [&_svg:not([class*='size-'])]:size-3",
        lg: "h-8 min-w-8 px-2.5 has-data-[icon=inline-end]:pe-2 has-data-[icon=inline-start]:ps-2",
      },
    },
    defaultVariants: {
      variant: "default",
      size: "default",
    },
  }
)

function Toggle({
  className,
  variant = "default",
  size = "default",
  ...props
}: React.ComponentProps<typeof TogglePrimitive.Root> &
  VariantProps<typeof toggleVariants>) {
  return (
    <TogglePrimitive.Root
      data-slot="toggle"
      className={cn(toggleVariants({ variant, size, className }))}
      {...props}
    />
  )
}

export { Toggle, toggleVariants }
