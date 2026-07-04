import * as React from "react"

import {cn} from "@/lib/utils"

function Textarea({className, ...props}: React.ComponentProps<"textarea">) {
    return (
        <textarea
            data-slot="textarea"
            className={cn(
                "textarea textarea-sm min-h-16 w-full resize-none",
                className
            )}
            {...props}
        />
    )
}

export {Textarea}
