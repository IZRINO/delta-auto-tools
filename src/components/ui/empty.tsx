import {cva, type VariantProps} from "class-variance-authority"

import {cn} from "@/lib/utils"

function Empty({className, ...props}: React.ComponentProps<"div">) {
    return (
        <div
            data-slot="empty"
            className={cn(
                "flex w-full min-w-0 flex-1 flex-col items-center justify-center gap-4 border-2 border-dashed border-[var(--chalk)] p-6 text-center text-balance",
                className
            )}
            {...props}
        />
    )
}

function EmptyHeader({className, ...props}: React.ComponentProps<"div">) {
    return (
        <div
            data-slot="empty-header"
            className={cn("flex max-w-sm flex-col items-center gap-1", className)}
            {...props}
        />
    )
}

const emptyMediaVariants = cva(
    "mb-2 flex shrink-0 items-center justify-center [&_svg]:pointer-events-none [&_svg]:shrink-0",
    {
        variants: {
            variant: {
                default: "bg-transparent",
                icon: "flex size-8 shrink-0 items-center justify-center border-2 border-[var(--chalk)] bg-[var(--slate)] text-[var(--chalk)] [&_svg:not([class*='size-'])]:size-4",
            },
        },
        defaultVariants: {
            variant: "default",
        },
    }
)

function EmptyMedia({
                        className,
                        variant = "default",
                        ...props
                    }: React.ComponentProps<"div"> & VariantProps<typeof emptyMediaVariants>) {
    return (
        <div
            data-slot="empty-icon"
            data-variant={variant}
            className={cn(emptyMediaVariants({variant, className}))}
            {...props}
        />
    )
}

function EmptyTitle({className, ...props}: React.ComponentProps<"div">) {
    return (
        <div
            data-slot="empty-title"
            className={cn(
                "font-heading text-sm font-black tracking-[-0.03em] uppercase",
                className
            )}
            {...props}
        />
    )
}

function EmptyDescription({className, ...props}: React.ComponentProps<"p">) {
    return (
        <div
            data-slot="empty-description"
            className={cn(
                "font-mono text-xs/relaxed font-bold tracking-[0.06em] text-[var(--zinc)] uppercase [&>a]:underline [&>a]:underline-offset-4 [&>a:hover]:text-[var(--amber)]",
                className
            )}
            {...props}
        />
    )
}

function EmptyContent({className, ...props}: React.ComponentProps<"div">) {
    return (
        <div
            data-slot="empty-content"
            className={cn(
                "flex w-full max-w-sm min-w-0 flex-col items-center gap-2 font-mono text-xs/relaxed font-bold text-balance",
                className
            )}
            {...props}
        />
    )
}

export {
    Empty,
    EmptyHeader,
    EmptyTitle,
    EmptyDescription,
    EmptyContent,
    EmptyMedia,
}
