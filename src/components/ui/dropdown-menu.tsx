"use client"

import * as React from "react"
import {DropdownMenu as DropdownMenuPrimitive} from "radix-ui"

import {cn} from "@/lib/utils"
import {RiArrowRightSLine, RiCheckLine} from "@remixicon/react"

function DropdownMenu({
                          ...props
                      }: React.ComponentProps<typeof DropdownMenuPrimitive.Root>) {
    return <DropdownMenuPrimitive.Root data-slot="dropdown-menu" {...props} />
}

function DropdownMenuPortal({
                                ...props
                            }: React.ComponentProps<typeof DropdownMenuPrimitive.Portal>) {
    return <DropdownMenuPrimitive.Portal data-slot="dropdown-menu-portal" {...props} />
}

function DropdownMenuTrigger({
                                 ...props
                             }: React.ComponentProps<typeof DropdownMenuPrimitive.Trigger>) {
    return (
        <DropdownMenuPrimitive.Trigger
            data-slot="dropdown-menu-trigger"
            {...props}
        />
    )
}

const menuContentClass = "menu z-50 max-h-(--radix-dropdown-menu-content-available-height) w-(--radix-dropdown-menu-trigger-width) min-w-32 origin-(--radix-dropdown-menu-content-transform-origin) overflow-x-hidden overflow-y-auto rounded-box border border-base-300 bg-base-200 p-1 text-sm text-base-content shadow-none duration-100 data-[side=bottom]:slide-in-from-top-2 data-[side=left]:slide-in-from-right-2 data-[side=right]:slide-in-from-left-2 data-[side=top]:slide-in-from-bottom-2 data-[state=closed]:overflow-hidden data-open:animate-in data-open:fade-in-0 data-open:zoom-in-95 data-closed:animate-out data-closed:fade-out-0 data-closed:zoom-out-95"
const menuItemClass = "relative flex min-h-8 cursor-default items-center gap-2 rounded-field px-2 py-1 outline-hidden select-none focus:bg-base-300 data-inset:ps-7.5 data-[variant=destructive]:text-base-content data-[variant=destructive]:focus:bg-error data-[variant=destructive]:focus:text-error-content data-disabled:pointer-events-none data-disabled:opacity-50 [&_svg]:pointer-events-none [&_svg]:shrink-0 [&_svg:not([class*='size-'])]:size-3.5"

function DropdownMenuContent({
                                 className,
                                 align = "start",
                                 sideOffset = 4,
                                 ...props
                             }: React.ComponentProps<typeof DropdownMenuPrimitive.Content>) {
    return (
        <DropdownMenuPrimitive.Portal>
            <DropdownMenuPrimitive.Content
                data-slot="dropdown-menu-content"
                sideOffset={sideOffset}
                align={align}
                className={cn(menuContentClass, className)}
                {...props}
            />
        </DropdownMenuPrimitive.Portal>
    )
}

function DropdownMenuGroup({
                               ...props
                           }: React.ComponentProps<typeof DropdownMenuPrimitive.Group>) {
    return (
        <DropdownMenuPrimitive.Group data-slot="dropdown-menu-group" {...props} />
    )
}

function DropdownMenuItem({
                              className,
                              inset,
                              variant = "default",
                              ...props
                          }: React.ComponentProps<typeof DropdownMenuPrimitive.Item> & {
    inset?: boolean
    variant?: "default" | "destructive"
}) {
    return (
        <DropdownMenuPrimitive.Item
            data-slot="dropdown-menu-item"
            data-inset={inset}
            data-variant={variant}
            className={cn(menuItemClass, className)}
            {...props}
        />
    )
}

function DropdownMenuCheckboxItem({
                                      className,
                                      children,
                                      checked,
                                      inset,
                                      ...props
                                  }: React.ComponentProps<typeof DropdownMenuPrimitive.CheckboxItem> & {
    inset?: boolean
}) {
    return (
        <DropdownMenuPrimitive.CheckboxItem
            data-slot="dropdown-menu-checkbox-item"
            data-inset={inset}
            className={cn(menuItemClass, "pe-8", className)}
            checked={checked}
            {...props}
        >
      <span
          className="pointer-events-none absolute end-2 flex items-center justify-center"
          data-slot="dropdown-menu-checkbox-item-indicator"
      >
        <DropdownMenuPrimitive.ItemIndicator>
          <RiCheckLine/>
        </DropdownMenuPrimitive.ItemIndicator>
      </span>
            {children}
        </DropdownMenuPrimitive.CheckboxItem>
    )
}

function DropdownMenuRadioGroup({
                                    ...props
                                }: React.ComponentProps<typeof DropdownMenuPrimitive.RadioGroup>) {
    return (
        <DropdownMenuPrimitive.RadioGroup
            data-slot="dropdown-menu-radio-group"
            {...props}
        />
    )
}

function DropdownMenuRadioItem({
                                   className,
                                   children,
                                   inset,
                                   ...props
                               }: React.ComponentProps<typeof DropdownMenuPrimitive.RadioItem> & {
    inset?: boolean
}) {
    return (
        <DropdownMenuPrimitive.RadioItem
            data-slot="dropdown-menu-radio-item"
            data-inset={inset}
            className={cn(menuItemClass, "pe-8", className)}
            {...props}
        >
      <span
          className="pointer-events-none absolute end-2 flex items-center justify-center"
          data-slot="dropdown-menu-radio-item-indicator"
      >
        <DropdownMenuPrimitive.ItemIndicator>
          <RiCheckLine/>
        </DropdownMenuPrimitive.ItemIndicator>
      </span>
            {children}
        </DropdownMenuPrimitive.RadioItem>
    )
}

function DropdownMenuLabel({
                               className,
                               inset,
                               ...props
                           }: React.ComponentProps<typeof DropdownMenuPrimitive.Label> & {
    inset?: boolean
}) {
    return (
        <DropdownMenuPrimitive.Label
            data-slot="dropdown-menu-label"
            data-inset={inset}
            className={cn(
                "menu-title px-2 py-1.5 text-xs text-base-content/60 data-inset:ps-7.5",
                className
            )}
            {...props}
        />
    )
}

function DropdownMenuSeparator({
                                   className,
                                   ...props
                               }: React.ComponentProps<typeof DropdownMenuPrimitive.Separator>) {
    return (
        <DropdownMenuPrimitive.Separator
            data-slot="dropdown-menu-separator"
            className={cn("-mx-1 my-1 h-px bg-base-300", className)}
            {...props}
        />
    )
}

function DropdownMenuShortcut({
                                  className,
                                  ...props
                              }: React.ComponentProps<"span">) {
    return (
        <span
            data-slot="dropdown-menu-shortcut"
            className={cn(
                "text-xs text-base-content/60 ms-auto",
                className
            )}
            {...props}
        />
    )
}

function DropdownMenuSub({
                             ...props
                         }: React.ComponentProps<typeof DropdownMenuPrimitive.Sub>) {
    return <DropdownMenuPrimitive.Sub data-slot="dropdown-menu-sub" {...props} />
}

function DropdownMenuSubTrigger({
                                    className,
                                    inset,
                                    children,
                                    ...props
                                }: React.ComponentProps<typeof DropdownMenuPrimitive.SubTrigger> & {
    inset?: boolean
}) {
    return (
        <DropdownMenuPrimitive.SubTrigger
            data-slot="dropdown-menu-sub-trigger"
            data-inset={inset}
            className={cn(menuItemClass, "data-open:bg-base-300", className)}
            {...props}
        >
            {children}
            <RiArrowRightSLine className="rtl:rotate-180 ms-auto"/>
        </DropdownMenuPrimitive.SubTrigger>
    )
}

function DropdownMenuSubContent({
                                    className,
                                    ...props
                                }: React.ComponentProps<typeof DropdownMenuPrimitive.SubContent>) {
    return (
        <DropdownMenuPrimitive.SubContent
            data-slot="dropdown-menu-sub-content"
            className={cn(menuContentClass, "w-auto overflow-hidden", className)}
            {...props}
        />
    )
}

export {
    DropdownMenu,
    DropdownMenuPortal,
    DropdownMenuTrigger,
    DropdownMenuContent,
    DropdownMenuGroup,
    DropdownMenuLabel,
    DropdownMenuItem,
    DropdownMenuCheckboxItem,
    DropdownMenuRadioGroup,
    DropdownMenuRadioItem,
    DropdownMenuSeparator,
    DropdownMenuShortcut,
    DropdownMenuSub,
    DropdownMenuSubTrigger,
    DropdownMenuSubContent,
}
