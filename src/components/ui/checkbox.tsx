import * as React from "react"
import {Checkbox as CheckboxPrimitive} from "radix-ui"

import {cn} from "@/lib/utils"
import {RiCheckLine} from "@remixicon/react"

function Checkbox({
                      className,
                      ...props
                  }: React.ComponentProps<typeof CheckboxPrimitive.Root>) {
    return (
        <CheckboxPrimitive.Root
            data-slot="checkbox"
            className={cn(
                "peer relative flex size-4 shrink-0 items-center justify-center border-2 border-[var(--chalk)] bg-[var(--carbon)] transition-colors outline-none group-has-disabled/field:opacity-50 after:absolute after:-inset-x-3 after:-inset-y-2 focus-visible:outline-2 focus-visible:outline-offset-[-2px] focus-visible:outline-[var(--amber)] disabled:cursor-not-allowed disabled:opacity-50 aria-invalid:border-[var(--amber)] data-checked:bg-[var(--chalk)] data-checked:text-[var(--carbon)]",
                className
            )}
            {...props}
        >
            <CheckboxPrimitive.Indicator
                data-slot="checkbox-indicator"
                className="grid place-content-center text-current transition-none [&>svg]:size-3.5"
            >
                <RiCheckLine/>
            </CheckboxPrimitive.Indicator>
        </CheckboxPrimitive.Root>
    )
}

export {Checkbox}
