import * as React from "react"
import {cva, type VariantProps} from "class-variance-authority"
import {Slot} from "radix-ui"

import {cn} from "@/lib/utils"

// ponytail: 直接复用 daisyUI .btn 体系（自带 flex+gap 管理图标间距）。
// data-icon="inline-start|inline-end" 属性保留为项目语义标记，不再驱动 CSS padding 补偿。
// cva 仅做 variant/size → daisyUI class 字典映射，避免重新发明样式系统。
const buttonVariants = cva("btn", {
    variants: {
        variant: {
            default: "btn-primary",
            outline: "btn-outline",
            secondary: "btn-secondary",
            ghost: "btn-ghost",
            destructive: "btn-error",
            link: "btn-link",
        },
        size: {
            default: "btn-md",
            xs: "btn-xs",
            sm: "btn-sm",
            lg: "btn-lg",
            icon: "btn-square",
            "icon-xs": "btn-square btn-xs",
            "icon-sm": "btn-square btn-sm",
            "icon-lg": "btn-square btn-lg",
        },
    },
    defaultVariants: {
        variant: "default",
        size: "default",
    },
})

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
            className={cn(buttonVariants({variant, size}), className)}
            {...props}
        />
    )
}

export {Button, buttonVariants}
