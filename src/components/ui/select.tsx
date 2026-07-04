"use client"

import * as React from "react"
import {Select as SelectPrimitive} from "radix-ui"

import {cn} from "@/lib/utils"
import {RiArrowDownSLine, RiArrowUpSLine, RiCheckLine} from "@remixicon/react"

function Select({
                    ...props
                }: React.ComponentProps<typeof SelectPrimitive.Root>) {
    return <SelectPrimitive.Root data-slot="select" {...props} />
}

function SelectGroup({
                         className,
                         ...props
                     }: React.ComponentProps<typeof SelectPrimitive.Group>) {
    return (
        <SelectPrimitive.Group
            data-slot="select-group"
            className={cn("scroll-my-1 p-1", className)}
            {...props}
        />
    )
}

function SelectValue({
                         ...props
                     }: React.ComponentProps<typeof SelectPrimitive.Value>) {
    return <SelectPrimitive.Value data-slot="select-value" {...props} />
}

function SelectTrigger({
                           className,
                           size = "default",
                           children,
                           ...props
                       }: React.ComponentProps<typeof SelectPrimitive.Trigger> & {
    size?: "sm" | "default"
}) {
    return (
        <SelectPrimitive.Trigger
            data-slot="select-trigger"
            data-size={size}
            className={cn(
                "select flex w-fit items-center justify-between gap-1.5 whitespace-nowrap outline-none data-[size=default]:select-sm data-[size=sm]:select-xs *:data-[slot=select-value]:line-clamp-1 *:data-[slot=select-value]:flex *:data-[slot=select-value]:items-center *:data-[slot=select-value]:gap-1.5 [&_svg]:pointer-events-none [&_svg]:shrink-0 [&_svg:not([class*='size-'])]:size-3.5",
                className
            )}
            {...props}
        >
            {children}
            <SelectPrimitive.Icon asChild>
                <RiArrowDownSLine className="pointer-events-none size-3.5"/>
            </SelectPrimitive.Icon>
        </SelectPrimitive.Trigger>
    )
}

function SelectContent({
                           className,
                           children,
                           position = "item-aligned",
                           align = "center",
                           ...props
                       }: React.ComponentProps<typeof SelectPrimitive.Content>) {
    return (
        <SelectPrimitive.Portal>
            <SelectPrimitive.Content
                data-slot="select-content"
                data-align-trigger={position === "item-aligned"}
                className={cn("menu relative z-50 max-h-(--radix-select-content-available-height) min-w-32 origin-(--radix-select-content-transform-origin) overflow-x-hidden overflow-y-auto rounded-box border border-base-300 bg-base-200 p-1 text-base-content shadow-none duration-100 data-[align-trigger=true]:animate-none data-[side=bottom]:slide-in-from-top-2 data-[side=left]:slide-in-from-right-2 data-[side=right]:slide-in-from-left-2 data-[side=top]:slide-in-from-bottom-2 data-open:animate-in data-open:fade-in-0 data-open:zoom-in-95 data-closed:animate-out data-closed:fade-out-0 data-closed:zoom-out-95", position === "popper" && "data-[side=bottom]:translate-y-1 data-[side=left]:-translate-x-1 rtl:data-[side=left]:translate-x-1 data-[side=right]:translate-x-1 rtl:data-[side=right]:-translate-x-1 data-[side=top]:-translate-y-1", className)}
                position={position}
                align={align}
                {...props}
            >
                <SelectScrollUpButton/>
                <SelectPrimitive.Viewport
                    data-position={position}
                    className={cn(
                        "data-[position=popper]:h-(--radix-select-trigger-height) data-[position=popper]:w-full data-[position=popper]:min-w-(--radix-select-trigger-width)",
                        position === "popper" && ""
                    )}
                >
                    {children}
                </SelectPrimitive.Viewport>
                <SelectScrollDownButton/>
            </SelectPrimitive.Content>
        </SelectPrimitive.Portal>
    )
}

function SelectLabel({
                         className,
                         ...props
                     }: React.ComponentProps<typeof SelectPrimitive.Label>) {
    return (
        <SelectPrimitive.Label
            data-slot="select-label"
            className={cn("menu-title px-2 py-1.5 text-xs text-base-content/60", className)}
            {...props}
        />
    )
}

function SelectItem({
                        className,
                        children,
                        ...props
                    }: React.ComponentProps<typeof SelectPrimitive.Item>) {
    return (
        <SelectPrimitive.Item
            data-slot="select-item"
            className={cn(
                "relative flex min-h-8 w-full cursor-default items-center gap-2 rounded-field px-2 py-1 text-sm outline-hidden select-none focus:bg-base-300 data-disabled:pointer-events-none data-disabled:opacity-50 [&_svg]:pointer-events-none [&_svg]:shrink-0 [&_svg:not([class*='size-'])]:size-3.5 *:[span]:last:flex *:[span]:last:items-center *:[span]:last:gap-2",
                className
            )}
            {...props}
        >
      <span className="pointer-events-none absolute end-2 flex items-center justify-center">
        <SelectPrimitive.ItemIndicator>
          <RiCheckLine className="pointer-events-none"/>
        </SelectPrimitive.ItemIndicator>
      </span>
            <SelectPrimitive.ItemText>{children}</SelectPrimitive.ItemText>
        </SelectPrimitive.Item>
    )
}

function SelectSeparator({
                             className,
                             ...props
                         }: React.ComponentProps<typeof SelectPrimitive.Separator>) {
    return (
        <SelectPrimitive.Separator
            data-slot="select-separator"
            className={cn("pointer-events-none -mx-1 my-1 h-px bg-base-300", className)}
            {...props}
        />
    )
}

function SelectScrollUpButton({
                                  className,
                                  ...props
                              }: React.ComponentProps<typeof SelectPrimitive.ScrollUpButton>) {
    return (
        <SelectPrimitive.ScrollUpButton
            data-slot="select-scroll-up-button"
            className={cn(
                "z-10 flex cursor-default items-center justify-center bg-base-200 py-1 [&_svg:not([class*='size-'])]:size-3.5",
                className
            )}
            {...props}
        >
            <RiArrowUpSLine/>
        </SelectPrimitive.ScrollUpButton>
    )
}

function SelectScrollDownButton({
                                    className,
                                    ...props
                                }: React.ComponentProps<typeof SelectPrimitive.ScrollDownButton>) {
    return (
        <SelectPrimitive.ScrollDownButton
            data-slot="select-scroll-down-button"
            className={cn(
                "z-10 flex cursor-default items-center justify-center bg-base-200 py-1 [&_svg:not([class*='size-'])]:size-3.5",
                className
            )}
            {...props}
        >
            <RiArrowDownSLine/>
        </SelectPrimitive.ScrollDownButton>
    )
}

export {
    Select,
    SelectContent,
    SelectGroup,
    SelectItem,
    SelectLabel,
    SelectScrollDownButton,
    SelectScrollUpButton,
    SelectSeparator,
    SelectTrigger,
    SelectValue,
}
