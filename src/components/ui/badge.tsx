import * as React from "react"
import { cva, type VariantProps } from "class-variance-authority"
import { Slot } from "radix-ui"

import { cn } from "@/lib/utils"

const badgeVariants = cva(
  "group/badge inline-flex h-6 w-fit shrink-0 items-center justify-center gap-1 overflow-hidden border border-[var(--chalk)] px-2 py-0.5 font-mono text-[0.68rem] font-black tracking-[0.12em] whitespace-nowrap uppercase tabular-nums transition-colors focus-visible:outline-2 focus-visible:outline-offset-[-2px] focus-visible:outline-[var(--amber)] has-data-[icon=inline-end]:pe-2 has-data-[icon=inline-start]:ps-2 aria-invalid:border-[var(--rust)] [&>svg]:pointer-events-none [&>svg]:size-3!",
  {
    variants: {
      variant: {
        default: "bg-[var(--amber)] text-[var(--carbon)] [a]:hover:bg-[var(--chalk)]",
        secondary: "bg-[var(--slate)] text-[var(--chalk)] [a]:hover:bg-[var(--carbon)]",
        destructive: "border-[var(--rust)] bg-[var(--carbon)] text-[var(--rust)] [a]:hover:bg-[var(--rust)] [a]:hover:text-[var(--carbon)]",
        outline: "bg-[var(--carbon)] text-[var(--chalk)] [a]:hover:bg-[var(--chalk)] [a]:hover:text-[var(--carbon)]",
        ghost: "border-transparent bg-transparent text-[var(--chalk)] hover:border-[var(--chalk)] hover:bg-[var(--slate)]",
        link: "border-transparent bg-transparent text-[var(--amber)] underline-offset-4 hover:underline",
      },
    },
    defaultVariants: {
      variant: "default",
    },
  }
)

function Badge({
  className,
  variant = "default",
  asChild = false,
  ...props
}: React.ComponentProps<"span"> &
  VariantProps<typeof badgeVariants> & { asChild?: boolean }) {
  const Comp = asChild ? Slot.Root : "span"

  return (
    <Comp
      data-slot="badge"
      data-variant={variant}
      className={cn(badgeVariants({ variant }), className)}
      {...props}
    />
  )
}

export { Badge, badgeVariants }
