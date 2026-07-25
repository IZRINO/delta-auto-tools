import {useEffect, useState} from "react";

import type {LoginRunSnapshot} from "@/components/app/special-ops-types";
import {SPECIAL_OPS_EVENTS} from "@/lib/tauri-events";
import {subscribeTauriEvent} from "@/lib/tauri-listener";

const DEFAULT_EMERGENCY_HOTKEY = "Ctrl+Shift+F12";

export function operationOverlayText(
    snapshot: LoginRunSnapshot,
    hotkey = DEFAULT_EMERGENCY_HOTKEY,
) {
    return {
        title: snapshot.countdownSeconds === null ? "特勤处操作中" : "即将占用键盘鼠标",
        detail: snapshot.countdownSeconds === null
            ? snapshot.message
            : `${snapshot.countdownSeconds} 秒后执行当前步骤`,
        hotkey,
    };
}

export function SpecialOpsOperationOverlay() {
    const [snapshot, setSnapshot] = useState<LoginRunSnapshot | null>(null);

    useEffect(() => subscribeTauriEvent<LoginRunSnapshot>(
        SPECIAL_OPS_EVENTS.runChanged,
        (event) => setSnapshot(event.payload),
    ), []);

    if (!snapshot) return null;

    const hotkey = new URLSearchParams(window.location.search).get("emergencyHotkey")
        ?? DEFAULT_EMERGENCY_HOTKEY;
    const text = operationOverlayText(snapshot, hotkey);
    return (
        <main className="card card-border h-dvh w-full bg-base-200 text-base-content shadow-lg">
            <div className="card-body gap-2 p-4">
                <h1 className="card-title text-base">{text.title}</h1>
                <p className="text-sm text-base-content/70">{text.detail}</p>
                <span className="badge badge-warning badge-sm">紧急停止：{text.hotkey}</span>
            </div>
        </main>
    );
}
