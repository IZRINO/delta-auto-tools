import * as React from "react"

import {cn} from "@/lib/utils"

type SwitchProps = Omit<React.InputHTMLAttributes<HTMLInputElement>, "onChange" | "size" | "type"> & {
    onCheckedChange?: (checked: boolean) => void
    size?: "sm" | "default"
}

function Switch({
                    className,
                    checked,
                    defaultChecked,
                    disabled,
                    onCheckedChange,
                    size = "default",
                    ...props
                }: SwitchProps) {
    return (
        <input
            data-slot="switch"
            type="checkbox"
            role="switch"
            checked={checked}
            defaultChecked={defaultChecked}
            disabled={disabled}
            className={cn(
                "toggle toggle-primary",
                size === "sm" ? "toggle-sm" : "toggle-md",
                className,
            )}
            onChange={(event) => onCheckedChange?.(event.currentTarget.checked)}
            {...props}
        />
    )
}

export {Switch}
