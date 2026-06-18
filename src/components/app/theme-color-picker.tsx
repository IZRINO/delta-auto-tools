import {useState} from "react";
import {HexColorPicker} from "react-colorful";
import {RiPaletteLine} from "@remixicon/react";

import {Popover, PopoverContent, PopoverTrigger} from "@/components/ui/popover";
import {Input} from "@/components/ui/input";
import {Button} from "@/components/ui/button";
import {normalizeHex} from "@/components/app/theme-utils";

/**
 * 颜色选择器：Popover + react-colorful 色轮 + hex 输入框 + 原生取色器入口。
 *
 * 用于主题面板的 TOKENS 编辑区。value 是 `#RRGGBB` 字符串。
 */
export function ThemeColorPicker({
    value,
    onChange,
    label,
}: {
    value: string;
    onChange: (value: string) => void;
    label?: string;
}) {
    const [open, setOpen] = useState(false);
    const [text, setText] = useState(value);

    // 外部 value 变化时同步输入框（如切换主题时）
    if (text !== value && !open) {
        setText(value);
    }

    const commit = (raw: string) => {
        const normalized = normalizeHex(raw);
        setText(normalized);
        onChange(normalized);
    };

    return (
        <div className="flex items-center gap-2">
            {/* 色块预览 + 原生取色器入口（点击直接调系统色板） */}
            <label
                className="relative size-8 shrink-0 cursor-pointer border-2 border-[var(--chalk)]"
                title={label ? `选择${label}` : "选择颜色"}
                aria-label={label ? `选择${label}颜色` : "选择颜色"}
            >
                <span
                    className="block size-full"
                    style={{backgroundColor: value}}
                    aria-hidden="true"
                />
                <input
                    type="color"
                    value={value.startsWith("#") ? value : "#000000"}
                    onChange={(e) => commit(e.target.value)}
                    className="absolute inset-0 size-full cursor-pointer opacity-0"
                    aria-hidden="true"
                    tabIndex={-1}
                />
            </label>

            {/* hex 输入框 */}
            <Input
                value={text}
                onChange={(e) => setText(e.target.value)}
                onBlur={(e) => commit(e.target.value)}
                onKeyDown={(e) => {
                    if (e.key === "Enter") {
                        commit((e.target as HTMLInputElement).value);
                    }
                }}
                className="h-8 w-24 font-mono text-xs"
                spellCheck={false}
            />

            {/* Popover 色轮（更精细的调整） */}
            <Popover open={open} onOpenChange={setOpen}>
                <PopoverTrigger asChild>
                    <Button
                        variant="outline"
                        size="icon"
                        className="size-8 border-2 border-[var(--chalk)]"
                        aria-label="打开色轮"
                    >
                        <RiPaletteLine className="size-4" aria-hidden="true"/>
                    </Button>
                </PopoverTrigger>
                <PopoverContent align="start" className="w-auto p-3">
                    <HexColorPicker
                        color={value}
                        onChange={commit}
                    />
                </PopoverContent>
            </Popover>
        </div>
    );
}
