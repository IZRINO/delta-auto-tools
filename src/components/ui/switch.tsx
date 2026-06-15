"use client"

import * as React from "react"
import {Switch as SwitchPrimitive} from "radix-ui"

import {cn} from "@/lib/utils"

function Switch({
                    className,
                    size = "default",
                    ...props
                }: React.ComponentProps<typeof SwitchPrimitive.Root> & {
    size?: "sm" | "default"
}) {
    return (
        <SwitchPrimitive.Root
            data-slot="switch"
            data-size={size}
            className={cn(
                "peer group/switch relative inline-flex shrink-0 items-center border-2 border-[var(--chalk)] transition-colors outline-none after:absolute after:-inset-x-3 after:-inset-y-2 focus-visible:outline-2 focus-visible:outline-offset-[-2px] focus-visible:outline-[var(--amber)] aria-invalid:border-[var(--amber)] data-[size=default]:h-5 data-[size=default]:w-9 data-[size=sm]:h-4 data-[size=sm]:w-7 data-checked:bg-[var(--amber)] data-unchecked:bg-[var(--carbon)] data-disabled:cursor-not-allowed data-disabled:opacity-50",
                className
            )}
            {...props}
        >
            <SwitchPrimitive.Thumb
                data-slot="switch-thumb"
                className="pointer-events-none block bg-[var(--chalk)] ring-0 transition-transform group-data-[size=default]/switch:size-3.5 group-data-[size=sm]/switch:size-2.5 group-data-[size=default]/switch:data-checked:translate-x-[17px] rtl:group-data-[size=default]/switch:data-checked:-translate-x-[17px] group-data-[size=sm]/switch:data-checked:translate-x-[13px] rtl:group-data-[size=sm]/switch:data-checked:-translate-x-[13px] group-data-[size=default]/switch:data-unchecked:translate-x-0.5 rtl:group-data-[size=default]/switch:data-unchecked:-translate-x-0.5 group-data-[size=sm]/switch:data-unchecked:translate-x-0.5 rtl:group-data-[size=sm]/switch:data-unchecked:-translate-x-0.5 data-checked:bg-[var(--carbon)]"
            />
        </SwitchPrimitive.Root>
    )
}

export {Switch}
