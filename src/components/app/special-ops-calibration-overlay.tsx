import {useCallback, useEffect, useMemo, useState} from "react";

import {Badge} from "@/components/ui/badge";
import {MIN_SELECTION_HEIGHT, MIN_SELECTION_WIDTH, type Point} from "@/components/app/morse-types";
import {getSelectionRect} from "@/components/app/morse-utils";
import {invokeLogged as invoke} from "@/lib/logging";

export function SpecialOpsCalibrationOverlay() {
    const params = useMemo(() => new URLSearchParams(window.location.search), []);
    const environmentId = params.get("environment_id") ?? "";
    const targetKey = params.get("target_key") ?? "";
    const accountId = params.get("account_id");
    const targetKind = params.get("target_kind") ?? "RecognitionRegion";
    const settingsRevision = Number(params.get("settings_revision") ?? "0");
    const [dragStart, setDragStart] = useState<Point | null>(null);
    const [dragCurrent, setDragCurrent] = useState<Point | null>(null);
    const [submitting, setSubmitting] = useState(false);
    const isClickPoint = targetKind === "ClickPoint";
    const [status, setStatus] = useState(isClickPoint ? "单击目标位置立即保存点击点；Esc 或右键取消。" : "按住左键拖拽框选区域，松开后自动保存；Esc 或右键取消。");
    const currentRect = useMemo(
        () => dragStart && dragCurrent ? getSelectionRect(dragStart, dragCurrent) : null,
        [dragCurrent, dragStart],
    );

    const cancel = useCallback(async () => {
        if (submitting) return;
        setSubmitting(true);
        try {
            await invoke("special_ops_cancel_calibration_selection", {environmentId, targetKey, accountId});
        } catch (error) {
            setStatus(String(error));
            setSubmitting(false);
        }
    }, [accountId, environmentId, submitting, targetKey]);

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

    const handleMouseDown = (event: React.MouseEvent<HTMLDivElement>) => {
        if (event.button !== 0 || submitting) return;
        const point = {x: event.clientX, y: event.clientY};
        if (isClickPoint) {
            setSubmitting(true);
            setStatus("正在保存点击点...");
            void invoke("special_ops_submit_calibration_selection", {
                environmentId,
                targetKey,
                accountId,
                region: {x: point.x, y: point.y, width: 1, height: 1},
                settingsRevision,
            }).catch((error) => {
                setStatus(String(error));
                setSubmitting(false);
            });
            return;
        }
        setDragStart(point);
        setDragCurrent(point);
        setStatus("正在框选区域...");
    };

    const handleMouseMove = (event: React.MouseEvent<HTMLDivElement>) => {
        if (dragStart && !submitting) setDragCurrent({x: event.clientX, y: event.clientY});
    };

    const handleMouseUp = (event: React.MouseEvent<HTMLDivElement>) => {
        if (!dragStart || submitting) return;
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
            accountId,
            region: rect,
            settingsRevision,
        }).catch((error) => {
            setStatus(String(error));
            setSubmitting(false);
        });
    };

    return <div
        className="fixed inset-0 cursor-crosshair select-none text-white"
        onContextMenu={(event) => { event.preventDefault(); void cancel(); }}
        onMouseDown={handleMouseDown}
        onMouseMove={handleMouseMove}
        onMouseUp={handleMouseUp}
    >
        {currentRect && <div className="pointer-events-none absolute border-2 border-primary bg-primary/16" style={{left: currentRect.x, top: currentRect.y, width: currentRect.width, height: currentRect.height}}/>}
        <div className="pointer-events-none absolute left-6 top-6 max-w-md border-2 border-white/40 bg-background/88 px-4 py-4 text-foreground backdrop-blur-md">
            <div className="flex items-center gap-2"><Badge variant="secondary">特勤处校准</Badge><Badge variant="outline">{targetKey}</Badge></div>
            <p className="mt-3 text-sm text-muted-foreground">{status}</p>
            <p className="mt-2 text-xs text-muted-foreground">{isClickPoint ? "单击保存；" : "拖拽框选后保存；"}Esc、右键或 Alt+F4 取消；窗口 30 秒后自动关闭。</p>
            {currentRect && <p className="mt-3 border border-border/70 bg-background/80 px-3 py-2 font-mono text-xs text-muted-foreground">{`X ${currentRect.x} · Y ${currentRect.y} · W ${currentRect.width} · H ${currentRect.height}`}</p>}
        </div>
    </div>;
}
