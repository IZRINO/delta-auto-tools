import * as React from "react"
import {cva, type VariantProps} from "class-variance-authority"
import {Slot} from "radix-ui"

import {cn} from "@/lib/utils"

const buttonVariants = cva(
    "group/button inline-flex shrink-0 items-center justify-center border-2 border-[var(--chalk)] bg-clip-padding font-mono text-xs font-black tracking-[0.12em] whitespace-nowrap uppercase transition-colors outline-none select-none focus-visible:outline-2 focus-visible:outline-offset-[-2px] focus-visible:outline-[var(--amber)] active:not-aria-[haspopup]:bg-[var(--chalk)] active:not-aria-[haspopup]:text-[var(--carbon)] disabled:pointer-events-none disabled:opacity-45 aria-invalid:border-[var(--rust)] aria-invalid:outline-2 aria-invalid:outline-[var(--rust)] [&_svg]:pointer-events-none [&_svg]:shrink-0 [&_svg:not([class*='size-'])]:size-4",
    {
        variants: {
            variant: {
                default: "bg-[var(--chalk)] text-[var(--carbon)] hover:bg-[var(--amber)] hover:text-[var(--carbon)]",
                outline: "bg-[var(--carbon)] text-[var(--chalk)] hover:bg-[var(--chalk)] hover:text-[var(--carbon)] aria-expanded:bg-[var(--chalk)] aria-expanded:text-[var(--carbon)]",
                secondary: "bg-[var(--slate)] text-[var(--chalk)] hover:bg-[var(--chalk)] hover:text-[var(--carbon)] aria-expanded:bg-[var(--chalk)] aria-expanded:text-[var(--carbon)]",
                ghost: "border-transparent bg-transparent text-[var(--chalk)] hover:border-[var(--chalk)] hover:bg-[var(--slate)] aria-expanded:border-[var(--chalk)] aria-expanded:bg-[var(--slate)]",
                destructive: "border-[var(--rust)] bg-[var(--carbon)] text-[var(--rust)] hover:bg-[var(--rust)] hover:text-[var(--carbon)] focus-visible:outline-[var(--rust)]",
                link: "border-transparent bg-transparent px-0 text-[var(--amber)] underline-offset-4 hover:underline",
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
            className={cn(buttonVariants({variant, size, className}))}
            {...props}
        />
    )
}

export {Button, buttonVariants}
