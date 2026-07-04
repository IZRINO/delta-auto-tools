"use client"

import * as React from "react"
import {cva, type VariantProps} from "class-variance-authority"
import {Tabs as TabsPrimitive} from "radix-ui"

import {cn} from "@/lib/utils"

function Tabs({
                  className,
                  orientation = "horizontal",
                  ...props
              }: React.ComponentProps<typeof TabsPrimitive.Root>) {
    return (
        <TabsPrimitive.Root
            data-slot="tabs"
            data-orientation={orientation}
            className={cn(
                "group/tabs flex gap-2 data-horizontal:flex-col",
                className
            )}
            {...props}
        />
    )
}

const tabsListVariants = cva(
    "tabs group/tabs-list w-fit group-data-vertical/tabs:flex-col",
    {
        variants: {
            variant: {
                default: "",
                line: "tabs-border",
            },
        },
        defaultVariants: {
            variant: "default",
        },
    }
)

function TabsList({
                      className,
                      variant = "default",
                      ...props
                  }: React.ComponentProps<typeof TabsPrimitive.List> &
    VariantProps<typeof tabsListVariants>) {
    return (
        <TabsPrimitive.List
            data-slot="tabs-list"
            data-variant={variant}
            className={cn(tabsListVariants({variant}), className)}
            {...props}
        />
    )
}

function TabsTrigger({
                         className,
                         ...props
                     }: React.ComponentProps<typeof TabsPrimitive.Trigger>) {
    return (
        <TabsPrimitive.Trigger
            data-slot="tabs-trigger"
            className={cn(
                "tab h-9 gap-1.5 whitespace-nowrap text-sm disabled:pointer-events-none disabled:opacity-50 data-[state=active]:tab-active has-data-[icon=inline-end]:pe-1 has-data-[icon=inline-start]:ps-1 [&_svg]:pointer-events-none [&_svg]:shrink-0 [&_svg:not([class*='size-'])]:size-3.5",
                className
            )}
            {...props}
        />
    )
}

function TabsContent({
                         className,
                         ...props
                     }: React.ComponentProps<typeof TabsPrimitive.Content>) {
    return (
        <TabsPrimitive.Content
            data-slot="tabs-content"
            className={cn("flex-1 text-xs/relaxed outline-none", className)}
            {...props}
        />
    )
}

export {Tabs, TabsList, TabsTrigger, TabsContent, tabsListVariants}
