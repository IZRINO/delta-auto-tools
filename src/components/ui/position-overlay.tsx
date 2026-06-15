import {useCallback, useEffect, useState} from "react";
import {invoke} from "@tauri-apps/api/core";

import {Badge} from "@/components/ui/badge";
import {getErrorMessage} from "@/lib/error-utils";

/** 位置设置 overlay 的 Tauri 命令集 */
export interface PositionOverlayCommands {
    /** 保存位置（如 "timer_position_commit" / "rapidfire_position_commit"） */
    commit: string;
    /** 取消设置（如 "timer_position_cancel" / "rapidfire_position_cancel"） */
    cancel: string;
    /** 窗口移动（如 "timer_position_moved" / "rapidfire_position_moved"） */
    moved: string;
}

export interface PositionOverlayProps {
    isNativeShell: boolean;
    /** 模块显示名称（如 "计时器" / "计数器" / "连发器"） */
    label: string;
    /** Tauri 命令集 */
    commands: PositionOverlayCommands;
    /** 初始状态消息后缀（可选）；Timer 添加关闭总开关提示 */
    initialStatusSuffix?: string;
}

/**
 * 通用位置设置 overlay 组件。
 *
 * 合并 TimerPositionOverlay 和 RapidfirePositionOverlay 的共同逻辑：
 * 拖拽定位 → Enter 保存 / Esc 取消 / 实时 moveTo 通知 Rust。
 */
export function PositionOverlay({isNativeShell, label, commands, initialStatusSuffix = ""}: PositionOverlayProps) {
    const [statusMessage, setStatusMessage] = useState(
        `拖动此固定大小框到目标位置，按 Enter 保存，按 Esc 退出修改。${initialStatusSuffix}`,
    );
    const [dragStart, setDragStart] = useState<{ mouseX: number; mouseY: number; x: number; y: number } | null>(null);
    const [position, setPosition] = useState({x: window.screenX, y: window.screenY, width: window.innerWidth});

    useEffect(() => {
        document.body.dataset.overlayMode = "true";
        return () => {
            delete document.body.dataset.overlayMode;
        };
    }, []);

    const commit = useCallback(async () => {
        if (!isNativeShell) {
            return;
        }
        setStatusMessage(`正在保存${label}透明窗口位置...`);
        try {
            await invoke(commands.commit);
        } catch (error) {
            setStatusMessage(getErrorMessage(error));
        }
    }, [isNativeShell, label, commands]);

    const cancel = useCallback(async () => {
        if (!isNativeShell) {
            return;
        }
        setStatusMessage(`正在退出${label}透明窗口位置设置...`);
        try {
            await invoke(commands.cancel);
        } catch (error) {
            setStatusMessage(getErrorMessage(error));
        }
    }, [isNativeShell, label, commands]);

    useEffect(() => {
        const handleKeyDown = (event: KeyboardEvent) => {
            if (event.key === "Enter") {
                event.preventDefault();
                void commit();
            }
            if (event.key === "Escape") {
                event.preventDefault();
                void cancel();
            }
        };

        window.addEventListener("keydown", handleKeyDown);
        return () => window.removeEventListener("keydown", handleKeyDown);
    }, [cancel, commit]);

    const moveTo = useCallback(async (x: number, y: number) => {
        setPosition((current) => ({...current, x, y}));
        if (!isNativeShell) {
            return;
        }
        try {
            await invoke(commands.moved, {x, y});
        } catch (error) {
            setStatusMessage(getErrorMessage(error));
        }
    }, [isNativeShell, commands]);

    return (
        <div
            className="flex h-screen w-screen cursor-move select-none items-center justify-center rounded-md border-2 border-primary bg-background/82 px-4 py-4 text-foreground backdrop-blur-md"
            onMouseDown={(event) => {
                if (event.button !== 0) {
                    return;
                }
                setDragStart({mouseX: event.screenX, mouseY: event.screenY, x: position.x, y: position.y});
            }}
            onMouseMove={(event) => {
                if (!dragStart) {
                    return;
                }
                void moveTo(dragStart.x + event.screenX - dragStart.mouseX, dragStart.y + event.screenY - dragStart.mouseY);
            }}
            onMouseUp={() => setDragStart(null)}
        >
            <div className="text-center">
                <Badge variant="secondary">{label}透明窗口位置</Badge>
                <p className="mt-3 text-sm font-medium">{statusMessage}</p>
                <p className="mt-2 font-mono text-xs text-muted-foreground">X {position.x} · Y {position.y} ·
                    W {position.width}</p>
            </div>
        </div>
    );
}
