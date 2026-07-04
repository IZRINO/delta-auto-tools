import * as React from "react"
import {cva, type VariantProps} from "class-variance-authority"

import {cn} from "@/lib/utils"

const alertVariants = cva(
    "alert group/alert relative w-full text-start has-data-[slot=alert-action]:relative has-data-[slot=alert-action]:pe-18 *:[svg]:text-current *:[svg:not([class*='size-'])]:size-4",
    {
        variants: {
            variant: {
                default: "alert-soft",
                destructive: "alert-error alert-soft",
            },
        },
        defaultVariants: {
            variant: "default",
        },
    }
)

function Alert({
                   className,
                   variant,
                   ...props
               }: React.ComponentProps<"div"> & VariantProps<typeof alertVariants>) {
    return (
        <div
            data-slot="alert"
            role="alert"
            className={cn(alertVariants({variant}), className)}
            {...props}
        />
    )
}

function AlertTitle({className, ...props}: React.ComponentProps<"div">) {
    return (
        <div
            data-slot="alert-title"
            className={cn(
                "font-semibold group-has-[>svg]/alert:col-start-2 [&_a]:underline [&_a]:underline-offset-3",
                className
            )}
            {...props}
        />
    )
}

function AlertDescription({
                              className,
                              ...props
                          }: React.ComponentProps<"div">) {
    return (
        <div
            data-slot="alert-description"
            className={cn(
                "text-sm/relaxed text-balance md:text-pretty [&_a]:underline [&_a]:underline-offset-3 [&_p:not(:last-child)]:mb-4",
                className
            )}
            {...props}
        />
    )
}

function AlertAction({className, ...props}: React.ComponentProps<"div">) {
    return (
        <div
            data-slot="alert-action"
            className={cn("absolute top-2 end-2", className)}
            {...props}
        />
    )
}

export {Alert, AlertTitle, AlertDescription, AlertAction}
