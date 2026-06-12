import * as React from "react"

import { cn } from "@/lib/utils"

function Textarea({ className, ...props }: React.ComponentProps<"textarea">) {
  return (
    <textarea
      data-slot="textarea"
      className={cn(
        "flex field-sizing-content min-h-16 w-full resize-none border-2 border-[var(--chalk)] bg-[var(--carbon)] px-3 py-2 font-mono text-sm font-bold tracking-[0.04em] text-[var(--chalk)] transition-colors outline-none placeholder:text-[var(--dust)] focus-visible:outline-2 focus-visible:outline-offset-[-2px] focus-visible:outline-[var(--amber)] disabled:cursor-not-allowed disabled:bg-[var(--slate)] disabled:opacity-55 aria-invalid:border-[var(--amber)] aria-invalid:outline-2 aria-invalid:outline-[var(--amber)] md:text-xs/relaxed",
        className
      )}
      {...props}
    />
  )
}

export { Textarea }
