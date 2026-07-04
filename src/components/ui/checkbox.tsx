import * as React from "react"

import {cn} from "@/lib/utils"

type CheckboxProps = Omit<React.InputHTMLAttributes<HTMLInputElement>, "onChange" | "type"> & {
    onCheckedChange?: (checked: boolean) => void
}

function Checkbox({
                      className,
                      checked,
                      defaultChecked,
                      disabled,
                      onCheckedChange,
                      ...props
                  }: CheckboxProps) {
    return (
        <input
            data-slot="checkbox"
            type="checkbox"
            checked={checked}
            defaultChecked={defaultChecked}
            disabled={disabled}
            className={cn("checkbox checkbox-primary checkbox-sm", className)}
            onChange={(event) => onCheckedChange?.(event.currentTarget.checked)}
            {...props}
        />
    )
}

export {Checkbox}
