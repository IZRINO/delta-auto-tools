import {useState} from "react";
import {ColorPicker} from "chromakit-react";
import "chromakit-react/chromakit.css";
import {RiPaletteLine} from "@remixicon/react";

import {Popover, PopoverContent, PopoverTrigger} from "@/components/ui/popover";
import {Input} from "@/components/ui/input";
import {Button} from "@/components/ui/button";
import {colorToHex, normalizeColorInput} from "@/components/app/theme-utils";

/**
 * 颜色选择器：色块+原生取色器入口、文本输入框、Popover 拾色按钮（chromakit-react OKLCH 面板）。
 *
 * value 接受任意合法 CSS 颜色字符串（hex 或 oklch()），onChange 永远输出标准
 * `oklch(L C H)` 字符串（用 culori 转换）。供主题面板 TOKENS 编辑区使用。
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
        const normalized = normalizeColorInput(raw);
        setText(normalized);
        onChange(normalized);
    };

    return (
        <div className="flex items-center gap-2">
            {/* 色块预览 + 原生取色器入口（点击直接调系统色板） */}
            <label
                className="relative size-8 shrink-0 cursor-pointer overflow-hidden rounded-field border border-base-300"
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
                    value={colorToHex(value)}
                    onChange={(e) => commit(e.target.value)}
                    className="absolute inset-0 size-full cursor-pointer opacity-0"
                    aria-hidden="true"
                    tabIndex={-1}
                />
            </label>

            {/* 颜色值输入框（接受 hex 或 oklch() 字符串） */}
            <Input
                value={text}
                onChange={(e) => setText(e.target.value)}
                onBlur={(e) => commit(e.target.value)}
                onKeyDown={(e) => {
                    if (e.key === "Enter") {
                        commit((e.target as HTMLInputElement).value);
                    }
                }}
                className="h-8 w-28 font-mono text-xs"
                spellCheck={false}
            />

            {/* Popover OKLCH 拾色面板（L/C/H 三通道交互控件） */}
            <Popover open={open} onOpenChange={setOpen}>
                <PopoverTrigger asChild>
                    <Button
                        variant="outline"
                        size="icon"
                        className="size-8"
                        aria-label="打开 OKLCH 拾色器"
                    >
                        <RiPaletteLine className="size-4" aria-hidden="true"/>
                    </Button>
                </PopoverTrigger>
                <PopoverContent align="start" className="w-auto p-2">
                    <ColorPicker
                        value={value}
                        onChange={(colorValue) => commit(colorValue.hex)}
                        formats={["oklch"]}
                        showAlpha={false}
                        showCopyButton={false}
                        showPresets={false}
                        enableHistory={false}
                    />
                </PopoverContent>
            </Popover>
        </div>
    );
}
