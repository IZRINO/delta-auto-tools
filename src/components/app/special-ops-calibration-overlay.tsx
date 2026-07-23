import {useCallback, useEffect, useMemo, useState} from "react";

import {Badge} from "@/components/ui/badge";
import {MIN_SELECTION_HEIGHT, MIN_SELECTION_WIDTH, type Point} from "@/components/app/morse-types";
import {getSelectionRect} from "@/components/app/morse-utils";
import {invokeLogged as invoke} from "@/lib/logging";

export function SpecialOpsCalibrationOverlay() {
    const params = useMemo(() => new URLSearchParams(window.location.search), []);
    const environmentId = params.get("environment_id") ?? "";
    const targetKey = params.get("target_key") ?? "";
    const settingsRevision = Number(params.get("settings_revision") ?? "0");
    const screenX = Number(params.get("screen_x") ?? "0");
    const screenY = Number(params.get("screen_y") ?? "0");
    const [dragStart, setDragStart] = useState<Point | null>(null);
    const [dragCurrent, setDragCurrent] = useState<Point | null>(null);
    const [submitting, setSubmitting] = useState(false);
    const [status, setStatus] = useState("按住左键拖拽框选区域，松开后自动保存；Esc 或右键取消。");
    const currentRect = useMemo(
        () => dragStart && dragCurrent ? getSelectionRect(dragStart, dragCurrent) : null,
        [dragCurrent, dragStart],
    );

    const cancel = useCallback(async () => {
        if (submitting) return;
        setSubmitting(true);
        try {
            await invoke("special_ops_cancel_calibration_selection", {environmentId, targetKey});
        } catch (error) {
            setStatus(String(error));
            setSubmitting(false);
        }
    }, [environmentId, submitting, targetKey]);

    useEffect(() => {
        const handleKeyDown = (event: KeyboardEvent) => {
            if (event.key === "Escape") {
                event.preventDefault();
                void cancel();
            }
        };
        window.addEventListener("keydown", handleKeyDown);
        return () => window.removeEventListener("keydown", handleKeyDown);
    }, [cancel]);

    return <div
        className="fixed inset-0 cursor-crosshair select-none text-white"
        onContextMenu={(event) => { event.preventDefault(); void cancel(); }}
        onPointerDown={(event) => {
            if (event.button !== 0 || submitting) return;
            event.currentTarget.setPointerCapture(event.pointerId);
            const point = {x: event.clientX, y: event.clientY};
            setDragStart(point);
            setDragCurrent(point);
            setStatus("正在框选区域...");
        }}
        onPointerMove={(event) => {
            if (dragStart && !submitting) setDragCurrent({x: event.clientX, y: event.clientY});
        }}
        onPointerUp={(event) => {
            if (!dragStart || submitting) return;
            if (event.currentTarget.hasPointerCapture(event.pointerId)) {
                event.currentTarget.releasePointerCapture(event.pointerId);
            }
            const rect = getSelectionRect(dragStart, {x: event.clientX, y: event.clientY});
            setDragStart(null);
            setDragCurrent(null);
            if (rect.width <= MIN_SELECTION_WIDTH || rect.height <= MIN_SELECTION_HEIGHT) {
                setStatus("区域太小，请重新框选。");
                return;
            }
            setSubmitting(true);
            setStatus("正在保存区域...");
            void invoke("special_ops_submit_calibration_selection", {
                environmentId,
                targetKey,
                region: {...rect, x: rect.x + screenX, y: rect.y + screenY},
                settingsRevision,
            }).catch((error) => {
                setStatus(String(error));
                setSubmitting(false);
            });
        }}
        onPointerCancel={(event) => {
            if (event.currentTarget.hasPointerCapture(event.pointerId)) {
                event.currentTarget.releasePointerCapture(event.pointerId);
            }
            setDragStart(null);
            setDragCurrent(null);
            setStatus("框选已中断，请重新拖拽。");
        }}
    >
        {currentRect && <div className="pointer-events-none absolute border-2 border-primary bg-primary/16" style={{left: currentRect.x, top: currentRect.y, width: currentRect.width, height: currentRect.height}}/>}
        <div className="pointer-events-none absolute left-6 top-6 max-w-md border-2 border-white/40 bg-background/88 px-4 py-4 text-foreground backdrop-blur-md">
            <div className="flex items-center gap-2"><Badge variant="secondary">特勤处校准</Badge><Badge variant="outline">{targetKey}</Badge></div>
            <p className="mt-3 text-sm text-muted-foreground">{status}</p>
            {currentRect && <p className="mt-3 border border-border/70 bg-background/80 px-3 py-2 font-mono text-xs text-muted-foreground">{`X ${currentRect.x} · Y ${currentRect.y} · W ${currentRect.width} · H ${currentRect.height}`}</p>}
        </div>
    </div>;
}
