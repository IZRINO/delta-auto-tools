import * as React from "react"

import {cn} from "@/lib/utils"

function Card({
                  className,
                  size = "default",
                  ...props
              }: React.ComponentProps<"div"> & { size?: "default" | "sm" }) {
    return (
        <div
            data-slot="card"
            data-size={size}
            className={cn(
                "card card-border group/card overflow-hidden bg-base-200 text-base-content shadow-none",
                size === "sm" ? "card-sm" : "card-md",
                className
            )}
            {...props}
        />
    )
}

function CardHeader({className, ...props}: React.ComponentProps<"div">) {
    return (
        <div
            data-slot="card-header"
            className={cn(
                "group/card-header @container/card-header grid auto-rows-min items-start gap-1.5 px-5 pt-5 group-data-[size=sm]/card:px-4 group-data-[size=sm]/card:pt-4 has-data-[slot=card-action]:grid-cols-[1fr_auto] has-data-[slot=card-description]:grid-rows-[auto_auto]",
                className
            )}
            {...props}
        />
    )
}

function CardTitle({className, ...props}: React.ComponentProps<"div">) {
    return (
        <div
            data-slot="card-title"
            className={cn("card-title text-base", className)}
            {...props}
        />
    )
}

function CardDescription({className, ...props}: React.ComponentProps<"div">) {
    return (
        <div
            data-slot="card-description"
            className={cn("text-sm text-base-content/60", className)}
            {...props}
        />
    )
}

function CardAction({className, ...props}: React.ComponentProps<"div">) {
    return (
        <div
            data-slot="card-action"
            className={cn(
                "col-start-2 row-span-2 row-start-1 self-start justify-self-end",
                className
            )}
            {...props}
        />
    )
}

function CardContent({className, ...props}: React.ComponentProps<"div">) {
    return (
        <div
            data-slot="card-content"
            className={cn("px-5 pb-5 group-data-[size=sm]/card:px-4 group-data-[size=sm]/card:pb-4", className)}
            {...props}
        />
    )
}

function CardFooter({className, ...props}: React.ComponentProps<"div">) {
    return (
        <div
            data-slot="card-footer"
            className={cn(
                "card-actions px-5 pb-5 group-data-[size=sm]/card:px-4 group-data-[size=sm]/card:pb-4",
                className
            )}
            {...props}
        />
    )
}

export {
    Card,
    CardHeader,
    CardFooter,
    CardTitle,
    CardAction,
    CardDescription,
    CardContent,
}
