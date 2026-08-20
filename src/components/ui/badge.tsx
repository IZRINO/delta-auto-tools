import * as React from "react"
import {cva, type VariantProps} from "class-variance-authority"
import {Slot} from "radix-ui"

import {cn} from "@/lib/utils"

// ponytail: 直接复用 daisyUI .badge 体系（自带 inline-flex + gap，图标与文字间距原生处理）。
// asChild(Slot) 行为保留，与 daisyUI class 组合无冲突。
const badgeVariants = cva("badge", {
    variants: {
        variant: {
            default: "badge-primary",
            secondary: "badge-secondary",
            destructive: "badge-error",
            outline: "badge-outline",
            ghost: "badge-ghost",
            link: "badge-ghost text-base-content underline-offset-4 hover:underline",
        },
    },
    defaultVariants: {
        variant: "default",
    },
})

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
            className={cn(badgeVariants({variant}), className)}
            {...props}
        />
    )
}

export {Badge, badgeVariants}
