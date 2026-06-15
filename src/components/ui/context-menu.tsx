import * as React from "react"
import {ContextMenu as ContextMenuPrimitive} from "radix-ui"
import {RiArrowRightSLine, RiCheckLine} from "@remixicon/react"

import {cn} from "@/lib/utils"

function ContextMenu({
                         ...props
                     }: React.ComponentProps<typeof ContextMenuPrimitive.Root>) {
    return <ContextMenuPrimitive.Root data-slot="context-menu" {...props} />
}

function ContextMenuTrigger({
                                className,
                                ...props
                            }: React.ComponentProps<typeof ContextMenuPrimitive.Trigger>) {
    return (
        <ContextMenuPrimitive.Trigger
            data-slot="context-menu-trigger"
            className={cn(className)}
            {...props}
        />
    )
}

function ContextMenuGroup({
                              ...props
                          }: React.ComponentProps<typeof ContextMenuPrimitive.Group>) {
    return <ContextMenuPrimitive.Group data-slot="context-menu-group" {...props} />
}

function ContextMenuPortal({
                               ...props
                           }: React.ComponentProps<typeof ContextMenuPrimitive.Portal>) {
    return <ContextMenuPrimitive.Portal data-slot="context-menu-portal" {...props} />
}

function ContextMenuSub({
                            ...props
                        }: React.ComponentProps<typeof ContextMenuPrimitive.Sub>) {
    return <ContextMenuPrimitive.Sub data-slot="context-menu-sub" {...props} />
}

function ContextMenuRadioGroup({
                                   ...props
                               }: React.ComponentProps<typeof ContextMenuPrimitive.RadioGroup>) {
    return (
        <ContextMenuPrimitive.RadioGroup
            data-slot="context-menu-radio-group"
            {...props}
        />
    )
}

const contextMenuContentClass = "z-50 max-h-(--radix-context-menu-content-available-height) min-w-32 origin-(--radix-context-menu-content-transform-origin) overflow-x-hidden overflow-y-auto border-2 border-[var(--chalk)] bg-[var(--carbon)] p-1 font-mono text-xs font-bold tracking-[0.06em] text-[var(--chalk)] shadow-none duration-100 data-[side=bottom]:slide-in-from-top-2 data-[side=left]:slide-in-from-right-2 data-[side=right]:slide-in-from-left-2 data-[side=top]:slide-in-from-bottom-2 data-open:animate-in data-open:fade-in-0 data-open:zoom-in-95 data-closed:animate-out data-closed:fade-out-0 data-closed:zoom-out-95"
const contextMenuItemClass = "relative flex min-h-8 cursor-default items-center gap-2 border border-transparent px-2 py-1 text-xs/relaxed outline-hidden select-none uppercase focus:border-[var(--chalk)] focus:bg-[var(--chalk)] focus:text-[var(--carbon)] data-inset:ps-7.5 data-[variant=destructive]:text-[var(--amber)] data-[variant=destructive]:focus:bg-[var(--amber)] data-[variant=destructive]:focus:text-[var(--carbon)] data-disabled:pointer-events-none data-disabled:opacity-50 [&_svg]:pointer-events-none [&_svg]:shrink-0 [&_svg:not([class*='size-'])]:size-3.5"

function ContextMenuContent({
                                className,
                                ...props
                            }: React.ComponentProps<typeof ContextMenuPrimitive.Content> & {
    side?: "top" | "right" | "bottom" | "left"
}) {
    return (
        <ContextMenuPrimitive.Portal>
            <ContextMenuPrimitive.Content
                data-slot="context-menu-content"
                className={cn(contextMenuContentClass, className)}
                {...props}
            />
        </ContextMenuPrimitive.Portal>
    )
}

function ContextMenuItem({
                             className,
                             inset,
                             variant = "default",
                             ...props
                         }: React.ComponentProps<typeof ContextMenuPrimitive.Item> & {
    inset?: boolean
    variant?: "default" | "destructive"
}) {
    return (
        <ContextMenuPrimitive.Item
            data-slot="context-menu-item"
            data-inset={inset}
            data-variant={variant}
            className={cn(contextMenuItemClass, className)}
            {...props}
        />
    )
}

function ContextMenuSubTrigger({
                                   className,
                                   inset,
                                   children,
                                   ...props
                               }: React.ComponentProps<typeof ContextMenuPrimitive.SubTrigger> & {
    inset?: boolean
}) {
    return (
        <ContextMenuPrimitive.SubTrigger
            data-slot="context-menu-sub-trigger"
            data-inset={inset}
            className={cn(contextMenuItemClass, "data-open:border-[var(--chalk)] data-open:bg-[var(--chalk)] data-open:text-[var(--carbon)]", className)}
            {...props}
        >
            {children}
            <RiArrowRightSLine className="rtl:rotate-180 ms-auto"/>
        </ContextMenuPrimitive.SubTrigger>
    )
}

function ContextMenuSubContent({
                                   className,
                                   ...props
                               }: React.ComponentProps<typeof ContextMenuPrimitive.SubContent>) {
    return (
        <ContextMenuPrimitive.SubContent
            data-slot="context-menu-sub-content"
            className={cn(contextMenuContentClass, "overflow-hidden", className)}
            {...props}
        />
    )
}

function ContextMenuCheckboxItem({
                                     className,
                                     children,
                                     checked,
                                     inset,
                                     ...props
                                 }: React.ComponentProps<typeof ContextMenuPrimitive.CheckboxItem> & {
    inset?: boolean
}) {
    return (
        <ContextMenuPrimitive.CheckboxItem
            data-slot="context-menu-checkbox-item"
            data-inset={inset}
            className={cn(contextMenuItemClass, "pe-8", className)}
            checked={checked}
            {...props}
        >
      <span className="pointer-events-none absolute end-2 flex items-center justify-center">
        <ContextMenuPrimitive.ItemIndicator>
          <RiCheckLine/>
        </ContextMenuPrimitive.ItemIndicator>
      </span>
            {children}
        </ContextMenuPrimitive.CheckboxItem>
    )
}

function ContextMenuRadioItem({
                                  className,
                                  children,
                                  inset,
                                  ...props
                              }: React.ComponentProps<typeof ContextMenuPrimitive.RadioItem> & {
    inset?: boolean
}) {
    return (
        <ContextMenuPrimitive.RadioItem
            data-slot="context-menu-radio-item"
            data-inset={inset}
            className={cn(contextMenuItemClass, "pe-8", className)}
            {...props}
        >
      <span className="pointer-events-none absolute end-2 flex items-center justify-center">
        <ContextMenuPrimitive.ItemIndicator>
          <RiCheckLine/>
        </ContextMenuPrimitive.ItemIndicator>
      </span>
            {children}
        </ContextMenuPrimitive.RadioItem>
    )
}

function ContextMenuLabel({
                              className,
                              inset,
                              ...props
                          }: React.ComponentProps<typeof ContextMenuPrimitive.Label> & {
    inset?: boolean
}) {
    return (
        <ContextMenuPrimitive.Label
            data-slot="context-menu-label"
            data-inset={inset}
            className={cn(
                "px-2 py-1.5 text-xs font-black tracking-[0.14em] text-[var(--zinc)] uppercase data-inset:ps-7.5",
                className
            )}
            {...props}
        />
    )
}

function ContextMenuSeparator({
                                  className,
                                  ...props
                              }: React.ComponentProps<typeof ContextMenuPrimitive.Separator>) {
    return (
        <ContextMenuPrimitive.Separator
            data-slot="context-menu-separator"
            className={cn("-mx-1 my-1 h-px bg-[var(--chalk)]", className)}
            {...props}
        />
    )
}

function ContextMenuShortcut({
                                 className,
                                 ...props
                             }: React.ComponentProps<"span">) {
    return (
        <span
            data-slot="context-menu-shortcut"
            className={cn(
                "text-[0.62rem] tracking-[0.18em] text-[var(--zinc)] uppercase ms-auto",
                className
            )}
            {...props}
        />
    )
}

export {
    ContextMenu,
    ContextMenuTrigger,
    ContextMenuContent,
    ContextMenuItem,
    ContextMenuCheckboxItem,
    ContextMenuRadioItem,
    ContextMenuLabel,
    ContextMenuSeparator,
    ContextMenuShortcut,
    ContextMenuGroup,
    ContextMenuPortal,
    ContextMenuSub,
    ContextMenuSubContent,
    ContextMenuSubTrigger,
    ContextMenuRadioGroup,
}
