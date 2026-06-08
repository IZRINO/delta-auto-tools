import * as React from "react"

import { cn } from "@/lib/utils"

function Textarea({ className, ...props }: React.ComponentProps<"textarea">) {
  return (
    <textarea
      data-slot="textarea"
      className={cn(
        "flex field-sizing-content min-h-16 w-full resize-none border-2 border-[var(--ink)] bg-[var(--paper)] px-3 py-2 font-mono text-sm font-bold tracking-[0.04em] text-[var(--ink)] transition-colors outline-none placeholder:text-[var(--ash)] focus-visible:outline-2 focus-visible:outline-offset-[-2px] focus-visible:outline-[var(--alert-red)] disabled:cursor-not-allowed disabled:bg-[var(--bone)] disabled:opacity-55 aria-invalid:border-[var(--alert-red)] aria-invalid:outline-2 aria-invalid:outline-[var(--alert-red)] md:text-xs/relaxed",
        className
      )}
      {...props}
    />
  )
}

export { Textarea }
