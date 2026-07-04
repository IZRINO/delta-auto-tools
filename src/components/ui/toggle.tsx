import * as React from "react"
import {cva, type VariantProps} from "class-variance-authority"
import {Toggle as TogglePrimitive} from "radix-ui"

import {cn} from "@/lib/utils"

const toggleVariants = cva(
    "group/toggle btn inline-flex items-center justify-center gap-1 whitespace-nowrap outline-none disabled:pointer-events-none disabled:opacity-50 data-[state=on]:btn-active [&_svg]:pointer-events-none [&_svg]:shrink-0 [&_svg:not([class*='size-'])]:size-4",
    {
        variants: {
            variant: {
                default: "btn-ghost",
                outline: "btn-outline",
            },
            size: {
                default: "btn-sm min-w-7 has-data-[icon=inline-end]:pe-1.5 has-data-[icon=inline-start]:ps-1.5",
                sm: "btn-xs min-w-6 has-data-[icon=inline-end]:pe-1.5 has-data-[icon=inline-start]:ps-1.5 [&_svg:not([class*='size-'])]:size-3",
                lg: "btn-md min-w-8 has-data-[icon=inline-end]:pe-2 has-data-[icon=inline-start]:ps-2",
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
            className={cn(toggleVariants({variant, size, className}))}
            {...props}
        />
    )
}

export {Toggle, toggleVariants}
