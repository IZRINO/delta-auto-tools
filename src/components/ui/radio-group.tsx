import * as React from "react"
import {RadioGroup as RadioGroupPrimitive} from "radix-ui"

import {cn} from "@/lib/utils"

function RadioGroup({
                        className,
                        ...props
                    }: React.ComponentProps<typeof RadioGroupPrimitive.Root>) {
    return (
        <RadioGroupPrimitive.Root
            data-slot="radio-group"
            className={cn("grid w-full gap-3", className)}
            {...props}
        />
    )
}

function RadioGroupItem({
                            className,
                            ...props
                        }: React.ComponentProps<typeof RadioGroupPrimitive.Item>) {
    return (
        <RadioGroupPrimitive.Item
            data-slot="radio-group-item"
            className={cn(
                "radio radio-primary radio-sm group/radio-group-item peer relative shrink-0 outline-none disabled:cursor-not-allowed disabled:opacity-50",
                className
            )}
            {...props}
        >
            <RadioGroupPrimitive.Indicator
                data-slot="radio-group-indicator"
                className="hidden"
            >
            </RadioGroupPrimitive.Indicator>
        </RadioGroupPrimitive.Item>
    )
}

export {RadioGroup, RadioGroupItem}
