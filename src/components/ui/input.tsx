import * as React from "react"

import {cn} from "@/lib/utils"

function Input({className, type, ...props}: React.ComponentProps<"input">) {
    return (
        <input
            type={type}
            data-slot="input"
            className={cn(
                "h-9 w-full min-w-0 border-2 border-[var(--chalk)] bg-[var(--carbon)] px-3 py-2 font-mono text-sm font-bold tracking-[0.04em] text-[var(--chalk)] transition-colors outline-none file:inline-flex file:h-7 file:border-0 file:bg-transparent file:text-sm file:font-black file:text-[var(--chalk)] placeholder:text-[var(--dust)] focus-visible:outline-2 focus-visible:outline-offset-[-2px] focus-visible:outline-[var(--amber)] disabled:pointer-events-none disabled:cursor-not-allowed disabled:bg-[var(--slate)] disabled:opacity-55 aria-invalid:border-[var(--amber)] aria-invalid:outline-2 aria-invalid:outline-[var(--amber)]",
                className
            )}
            {...props}
        />
    )
}

export {Input}
