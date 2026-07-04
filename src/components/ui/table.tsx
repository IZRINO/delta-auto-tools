import * as React from "react"

import {cn} from "@/lib/utils"

function Table({className, ...props}: React.ComponentProps<"table">) {
    return (
        <div
            data-slot="table-container"
            className="relative w-full overflow-x-auto rounded-box border border-base-300"
        >
            <table
                data-slot="table"
                className={cn("table table-sm w-full caption-bottom", className)}
                {...props}
            />
        </div>
    )
}

function TableHeader({className, ...props}: React.ComponentProps<"thead">) {
    return (
        <thead
            data-slot="table-header"
            className={cn("bg-base-200", className)}
            {...props}
        />
    )
}

function TableBody({className, ...props}: React.ComponentProps<"tbody">) {
    return (
        <tbody
            data-slot="table-body"
            className={cn("[&_tr:last-child]:border-0", className)}
            {...props}
        />
    )
}

function TableFooter({className, ...props}: React.ComponentProps<"tfoot">) {
    return (
        <tfoot
            data-slot="table-footer"
            className={cn(
                "border-t border-base-300 bg-base-200 font-medium [&>tr]:last:border-b-0",
                className
            )}
            {...props}
        />
    )
}

function TableRow({className, ...props}: React.ComponentProps<"tr">) {
    return (
        <tr
            data-slot="table-row"
            className={cn(
                "border-b border-base-300 transition-colors hover:bg-base-200 has-aria-expanded:bg-base-200 data-[state=selected]:bg-base-200",
                className
            )}
            {...props}
        />
    )
}

function TableHead({className, ...props}: React.ComponentProps<"th">) {
    return (
        <th
            data-slot="table-head"
            className={cn(
                "h-10 px-2 text-start align-middle font-medium whitespace-nowrap [&:has([role=checkbox])]:pe-0",
                className
            )}
            {...props}
        />
    )
}

function TableCell({className, ...props}: React.ComponentProps<"td">) {
    return (
        <td
            data-slot="table-cell"
            className={cn(
                "p-2 align-middle whitespace-nowrap [&:has([role=checkbox])]:pe-0",
                className
            )}
            {...props}
        />
    )
}

function TableCaption({
                          className,
                          ...props
                      }: React.ComponentProps<"caption">) {
    return (
        <caption
            data-slot="table-caption"
            className={cn("mt-4 text-xs text-base-content/60", className)}
            {...props}
        />
    )
}

export {
    Table,
    TableHeader,
    TableBody,
    TableFooter,
    TableHead,
    TableRow,
    TableCell,
    TableCaption,
}
