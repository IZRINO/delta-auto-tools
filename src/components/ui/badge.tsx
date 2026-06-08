import * as React from "react"
import { cva, type VariantProps } from "class-variance-authority"
import { Slot } from "radix-ui"

import { cn } from "@/lib/utils"

const badgeVariants = cva(
  "group/badge inline-flex h-6 w-fit shrink-0 items-center justify-center gap-1 overflow-hidden border border-[var(--ink)] px-2 py-0.5 font-mono text-[0.68rem] font-black tracking-[0.12em] whitespace-nowrap uppercase tabular-nums transition-colors focus-visible:outline-2 focus-visible:outline-offset-[-2px] focus-visible:outline-[var(--alert-red)] has-data-[icon=inline-end]:pe-2 has-data-[icon=inline-start]:ps-2 aria-invalid:border-[var(--alert-red)] [&>svg]:pointer-events-none [&>svg]:size-3!",
  {
    variants: {
      variant: {
        default: "bg-[var(--alert-red)] text-[var(--paper)] [a]:hover:bg-[var(--ink)]",
        secondary: "bg-[var(--bone)] text-[var(--ink)] [a]:hover:bg-[var(--paper)]",
        destructive: "border-[var(--alert-red)] bg-[var(--paper)] text-[var(--alert-red)] [a]:hover:bg-[var(--alert-red)] [a]:hover:text-[var(--paper)]",
        outline: "bg-[var(--paper)] text-[var(--ink)] [a]:hover:bg-[var(--ink)] [a]:hover:text-[var(--paper)]",
        ghost: "border-transparent bg-transparent text-[var(--ink)] hover:border-[var(--ink)] hover:bg-[var(--bone)]",
        link: "border-transparent bg-transparent text-[var(--alert-red)] underline-offset-4 hover:underline",
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
