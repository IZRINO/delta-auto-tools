import {useCallback, useEffect, useMemo, useState} from "react";
import {RiCheckLine, RiCloseLine} from "@remixicon/react";

import {Button} from "@/components/ui/button";
import {invokeLogged as invoke} from "@/lib/logging";
import type {CalibrationRect} from "@/components/app/special-ops-types";

type Point = {x: number; y: number};

function selectionRect(start: Point, end: Point): CalibrationRect {
    return {
        x: Math.min(start.x, end.x),
        y: Math.min(start.y, end.y),
        width: Math.abs(end.x - start.x),
        height: Math.abs(end.y - start.y),
    };
}

export function SpecialOpsCalibrationOverlay() {
    const params = useMemo(() => new URLSearchParams(window.location.search), []);
    const environmentId = params.get("environment_id") ?? "";
    const targetKey = params.get("target_key") ?? "";
    const targetKind = params.get("target_kind") ?? "recognitionRegion";
    const isClickPoint = targetKind === "clickPoint";
    const settingsRevision = Number(params.get("settings_revision") ?? "0");
    const screenX = Number(params.get("screen_x") ?? "0");
    const screenY = Number(params.get("screen_y") ?? "0");
    const [start, setStart] = useState<Point | null>(null);
    const [current, setCurrent] = useState<Point | null>(null);
    const [committed, setCommitted] = useState<CalibrationRect | null>(null);
    const [submitting, setSubmitting] = useState(false);
    const activeRect = start && current ? selectionRect(start, current) : committed;

    const cancel = useCallback(async () => {
        if (submitting) return;
        setSubmitting(true);
        try {
            await invoke("special_ops_cancel_calibration_selection", {environmentId, targetKey});
        } catch {
            setSubmitting(false);
        }
    }, [environmentId, submitting, targetKey]);
    const submit = useCallback(async () => {
        if (!committed || submitting) return;
        setSubmitting(true);
        try {
            await invoke("special_ops_submit_calibration_selection", {
                environmentId,
                targetKey,
                region: {...committed, x: committed.x + screenX, y: committed.y + screenY},
                settingsRevision,
            });
        } catch {
            setSubmitting(false);
        }
    }, [committed, environmentId, screenX, screenY, settingsRevision, submitting, targetKey]);

    useEffect(() => {
        const onKeyDown = (event: KeyboardEvent) => {
            if (event.key === "Escape") void cancel();
            if (event.key === "Enter") void submit();
        };
        window.addEventListener("keydown", onKeyDown);
        return () => window.removeEventListener("keydown", onKeyDown);
    }, [cancel, submit]);

    const commitPoint = (point: Point) => setCommitted({x: point.x - 4, y: point.y - 4, width: 8, height: 8});

    return <div
        className="fixed inset-0 cursor-crosshair select-none text-white"
        onContextMenu={(event) => { event.preventDefault(); void cancel(); }}
        onMouseDown={(event) => {
            if (event.button !== 0 || submitting) return;
            const point = {x: event.clientX, y: event.clientY};
            if (isClickPoint) {
                commitPoint(point);
                return;
            }
            setCommitted(null);
            setStart(point);
            setCurrent(point);
        }}
        onMouseMove={(event) => { if (!isClickPoint && start && !submitting) setCurrent({x: event.clientX, y: event.clientY}); }}
        onMouseUp={() => { if (isClickPoint || !start || !current) return; const rect = selectionRect(start, current); setStart(null); setCurrent(null); if (rect.width > 2 && rect.height > 2) setCommitted(rect); }}
    >
        {activeRect && <div className="pointer-events-none absolute border-2 border-primary bg-primary/20" style={{left: activeRect.x, top: activeRect.y, width: activeRect.width, height: activeRect.height}}/>}
        <div className="pointer-events-none absolute bottom-6 left-1/2 w-[min(92vw,640px)] -translate-x-1/2 rounded-box bg-base-100/90 p-3 text-base-content shadow-xl backdrop-blur">
            <h1 className="font-semibold">特勤处校准</h1>
            <p className="mt-1 text-sm text-base-content/60">底层画面保持可见但不会接收点击。{isClickPoint ? "单击目标位置取点。" : "按住左键拖拽框选目标区域。"} Enter 确认，Esc 取消。</p>
            <p className="mt-2 font-mono text-xs">{targetKey}{activeRect ? ` · X ${activeRect.x} Y ${activeRect.y} W ${activeRect.width} H ${activeRect.height}` : ""}</p>
        </div>
        <div className="absolute right-6 top-6 flex gap-2 rounded-box bg-base-100/90 p-3" onMouseDown={(event) => event.stopPropagation()} onMouseUp={(event) => event.stopPropagation()}>
            <Button disabled={!committed || submitting} onClick={() => void submit()}><RiCheckLine data-icon="inline-start"/>确认</Button>
            <Button variant="secondary" disabled={submitting} onClick={() => void cancel()}><RiCloseLine data-icon="inline-start"/>取消</Button>
        </div>
    </div>;
}
