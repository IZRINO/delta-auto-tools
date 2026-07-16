import {useCallback, useEffect, useMemo, useState} from "react";
import {RiCheckLine, RiCloseLine} from "@remixicon/react";

import {invokeLogged as invoke} from "@/lib/logging";
import type {Point} from "@/components/app/morse-types";
import {MIN_SELECTION_HEIGHT, MIN_SELECTION_WIDTH} from "@/components/app/morse-types";
import {getErrorMessage, getSelectionRect} from "@/components/app/morse-utils";
import {getSettingsRevision} from "@/components/app/profile-utils";
import {useProfile} from "@/hooks/use-profile";
import {Button} from "@/components/ui/button";

export function RecognitionRegionOverlay() {
    const {bootstrap: profileBootstrap} = useProfile();
    const params = useMemo(() => new URLSearchParams(window.location.search), []);
    const cardId = params.get("recognition_card") ?? "";
    const selectionTarget = params.get("selection_target") ?? undefined;
    // 识色模式探针框选时透传的探针索引；区域监听模式为 null
    const probeIndex = useMemo(() => {
        const raw = params.get("probe_index");
        if (raw === null) return undefined;
        const parsed = Number.parseInt(raw, 10);
        return Number.isNaN(parsed) ? undefined : parsed;
    }, [params]);

    const [dragStart, setDragStart] = useState<Point | null>(null);
    const [dragCurrent, setDragCurrent] = useState<Point | null>(null);
    // 松开后固定的已选区域（等待确认/重选）
    const [committedRect, setCommittedRect] = useState<{
        x: number;
        y: number;
        width: number;
        height: number
    } | null>(null);
    const [statusMessage, setStatusMessage] = useState("拖拽鼠标框选监听区域，松开后按 Enter 确认，Esc 取消。");
    const [submitting, setSubmitting] = useState(false);

    // 拖拽中的实时矩形
    const currentRect = useMemo(() => {
        if (!dragStart || !dragCurrent) return null;
        return getSelectionRect(dragStart, dragCurrent);
    }, [dragStart, dragCurrent]);

    const cancelSelection = useCallback(async () => {
        if (submitting) return;
        setSubmitting(true);
        setStatusMessage("正在取消...");
        try {
            await invoke("recognition_overlay_cancel_selection", {cardId});
        } catch (error) {
            setStatusMessage(getErrorMessage(error));
            setSubmitting(false);
        }
    }, [cardId, submitting]);

    const submitSelection = useCallback(async () => {
        const rect = committedRect;
        if (!rect || submitting) return;
        if (rect.width <= MIN_SELECTION_WIDTH || rect.height <= MIN_SELECTION_HEIGHT) {
            setStatusMessage(`区域太小（${rect.width}x${rect.height}），请重新框选。`);
            return;
        }
        setSubmitting(true);
        setStatusMessage("正在提交...");
        try {
            await invoke("recognition_overlay_submit_selection", {
                cardId,
                probeIndex,
                selectionTarget,
                region: rect,
                settingsRevision: getSettingsRevision(profileBootstrap),
            });
        } catch (error) {
            setStatusMessage(getErrorMessage(error));
            setSubmitting(false);
        }
    }, [cardId, probeIndex, selectionTarget, committedRect, profileBootstrap, submitting]);

    useEffect(() => {
        const handleKeyDown = (event: KeyboardEvent) => {
            if (event.key === "Escape") {
                event.preventDefault();
                void cancelSelection();
            } else if (event.key === "Enter" && committedRect && !submitting) {
                event.preventDefault();
                void submitSelection();
            }
        };
        window.addEventListener("keydown", handleKeyDown);
        return () => window.removeEventListener("keydown", handleKeyDown);
    }, [cancelSelection, committedRect, submitSelection, submitting]);

    const handleMouseDown = (event: React.MouseEvent<HTMLDivElement>) => {
        if (submitting || event.button !== 0) return;
        // 重新框选：清除已提交矩形
        setCommittedRect(null);
        const point = {x: event.clientX, y: event.clientY};
        setDragStart(point);
        setDragCurrent(point);
        setStatusMessage("正在框选...");
    };

    const handleMouseMove = (event: React.MouseEvent<HTMLDivElement>) => {
        if (!dragStart || submitting) return;
        setDragCurrent({x: event.clientX, y: event.clientY});
    };

    const handleMouseUp = () => {
        if (!dragStart || submitting) return;
        const rect = currentRect;
        // 清除拖拽状态，固定矩形
        setDragStart(null);
        setDragCurrent(null);

        if (rect && (rect.width <= MIN_SELECTION_WIDTH || rect.height <= MIN_SELECTION_HEIGHT)) {
            setCommittedRect(null);
            setStatusMessage(`区域太小（${rect.width}x${rect.height}），请重新框选。`);
            return;
        }
        if (rect) {
            setCommittedRect(rect);
            setStatusMessage("区域已框选，按 Enter 确认或重新拖拽框选，Esc 取消。");
        }
    };

    // 最终显示的矩形：拖拽中用实时矩形，松开后用已提交矩形
    const displayRect = currentRect ?? committedRect;

    return (
        <div
            className="fixed inset-0 cursor-crosshair select-none text-white"
            onContextMenu={(event) => {
                event.preventDefault();
                void cancelSelection();
            }}
            onMouseDown={handleMouseDown}
            onMouseMove={handleMouseMove}
            onMouseUp={handleMouseUp}
        >
            {displayRect && (
                <div
                    className="pointer-events-none absolute border border-primary bg-primary/16"
                    style={{
                        left: displayRect.x,
                        top: displayRect.y,
                        width: displayRect.width,
                        height: displayRect.height,
                    }}
                />
            )}

            <div
                className="pointer-events-none absolute left-6 top-6 max-w-md border border-white/40 bg-base-100/88 px-4 py-4 text-base-content backdrop-blur-md">
                <h1 className="text-lg font-semibold text-base-content">识别区域选择</h1>
                <p className="mt-2 text-sm text-base-content/60">{statusMessage}</p>
                {displayRect && (
                    <p className="mt-3 border border-base-300 bg-base-200/80 px-3 py-2 font-mono text-xs text-base-content/60">
                        {`X ${displayRect.x} · Y ${displayRect.y} · W ${displayRect.width} · H ${displayRect.height}`}
                    </p>
                )}
            </div>

            <div
                className="absolute right-6 top-6 flex items-center gap-2 border border-white/30 bg-base-100/80 px-3 py-3 backdrop-blur-md">
                <Button
                    disabled={!committedRect || submitting}
                    onClick={() => void submitSelection()}
                    type="button"
                    variant="secondary"
                    data-icon="inline-start"
                >
                    <RiCheckLine className="size-4" aria-hidden="true"/>
                    确认
                </Button>
                <Button
                    disabled={submitting}
                    onClick={() => void cancelSelection()}
                    type="button"
                    variant="secondary"
                    data-icon="inline-start"
                >
                    <RiCloseLine className="size-4" aria-hidden="true"/>
                    取消
                </Button>
            </div>
        </div>
    );
}
