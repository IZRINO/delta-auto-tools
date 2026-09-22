import {useCallback, useEffect, useMemo, useState} from "react";
import {invokeLogged as invoke} from "@/lib/logging";

import {Badge} from "@/components/ui/badge";
import {Button} from "@/components/ui/button";
import {MIN_SELECTION_HEIGHT, MIN_SELECTION_WIDTH, type Point} from "@/components/app/morse-types";
import {getSelectionRect} from "@/components/app/morse-utils";
import {getSettingsRevision} from "@/components/app/profile-utils";
import {useProfile} from "@/hooks/use-profile";
import {
    type RegionRect,
    type RegionSelectionProgress,
} from "@/components/app/fingerprint-types";
import {
    getErrorMessage,
    layoutLabels,
    parseOverlayTarget,
} from "@/components/app/fingerprint-utils";

export function FingerprintRegionOverlay({slots}: {slots: number[]}) {
    const {bootstrap: profileBootstrap} = useProfile();
    const target = useMemo(() => parseOverlayTarget(), []);
    const labels = layoutLabels(target);
    const [dragStart, setDragStart] = useState<Point | null>(null);
    const [dragCurrent, setDragCurrent] = useState<Point | null>(null);
    const [rects, setRects] = useState<Array<RegionRect | null>>(() => Array.from({length: labels.length}, () => null));
    const [completedSlots, setCompletedSlots] = useState<number[]>([]);
    const [currentSlot, setCurrentSlot] = useState<number | null>(slots[0] ?? null);
    const [statusMessage, setStatusMessage] = useState(
        target === "click" ? "拖拽框选点击区域，Esc 取消" : "拖拽框选区域，Esc 取消",
    );
    const [submitting, setSubmitting] = useState(false);

    const currentRect = useMemo(() => {
        if (!dragStart || !dragCurrent) return null;
        return getSelectionRect(dragStart, dragCurrent);
    }, [dragCurrent, dragStart]);

    const activeStep = currentSlot === null ? slots.length : completedSlots.length + 1;

    const cancelSelection = useCallback(async () => {
        if (currentSlot === null || submitting) return;
        setSubmitting(true);
        setStatusMessage("正在取消区域选择...");
        try {
            await invoke("fingerprint_overlay_cancel_selection", {slot: currentSlot});
        } catch (error) {
            setStatusMessage(getErrorMessage(error));
            setSubmitting(false);
        }
    }, [currentSlot, submitting]);

    useEffect(() => {
        const handleKeyDown = (event: KeyboardEvent) => {
            if (event.key === "Escape") {
                event.preventDefault();
                void cancelSelection();
            }
        };
        window.addEventListener("keydown", handleKeyDown);
        return () => window.removeEventListener("keydown", handleKeyDown);
    }, [cancelSelection]);

    const handleMouseDown = (event: React.MouseEvent<HTMLDivElement>) => {
        if (currentSlot === null || submitting || event.button !== 0) return;
        const point = {x: event.clientX, y: event.clientY};
        setDragStart(point);
        setDragCurrent(point);
        setStatusMessage(`正在框选 ${labels[currentSlot]}...`);
    };

    const handleMouseMove = (event: React.MouseEvent<HTMLDivElement>) => {
        if (!dragStart || submitting) return;
        setDragCurrent({x: event.clientX, y: event.clientY});
    };

    const handleMouseUp = async (event: React.MouseEvent<HTMLDivElement>) => {
        if (!dragStart || currentSlot === null || submitting) return;
        const rect = getSelectionRect(dragStart, {x: event.clientX, y: event.clientY});
        setDragStart(null);
        setDragCurrent(null);
        if (rect.width <= MIN_SELECTION_WIDTH || rect.height <= MIN_SELECTION_HEIGHT) {
            setStatusMessage("区域太小，请重新框选。");
            return;
        }
        setSubmitting(true);
        try {
            const progress = await invoke<RegionSelectionProgress>("fingerprint_overlay_submit_selection", {
                slot: currentSlot,
                rect,
                settingsRevision: getSettingsRevision(profileBootstrap),
            });
            setRects(progress.rects);
            setCompletedSlots(progress.completedSlots);
            setCurrentSlot(progress.currentSlot);
            if (progress.currentSlot === null) {
                setStatusMessage("区域已保存，正在返回主界面...");
                return;
            }
            setStatusMessage(`${labels[currentSlot]} 已保存，请继续框选 ${labels[progress.currentSlot]}。`);
            setSubmitting(false);
        } catch (error) {
            setStatusMessage(getErrorMessage(error));
            setSubmitting(false);
        }
    };

    return (
        <div
            className="fixed inset-0 cursor-crosshair select-none text-base-content"
            onContextMenu={(event) => {
                event.preventDefault();
                void cancelSelection();
            }}
            onMouseDown={handleMouseDown}
            onMouseMove={handleMouseMove}
            onMouseUp={handleMouseUp}
        >
            {rects.map((region, index) => {
                if (!region) return null;
                const isCurrent = currentSlot === index;
                return (
                    <div
                        key={index}
                        className={isCurrent ? "pointer-events-none absolute border-2 border-primary bg-primary/12" : "pointer-events-none absolute border border-base-content/80 bg-base-100/10"}
                        style={{left: region.x, top: region.y, width: region.width, height: region.height}}
                    />
                );
            })}
            {currentRect ? (
                <div
                    className="pointer-events-none absolute border-2 border-primary bg-primary/16"
                    style={{left: currentRect.x, top: currentRect.y, width: currentRect.width, height: currentRect.height}}
                />
            ) : null}
            <div className="pointer-events-none absolute left-6 top-6 max-w-md border-2 border-base-content/40 bg-base-100/90 px-4 py-4 backdrop-blur-md">
                <div className="flex items-center gap-2">
                    <Badge variant="outline">{`第 ${activeStep} / ${slots.length} 步`}</Badge>
                    {currentSlot !== null ? <Badge variant="secondary">{labels[currentSlot]}</Badge> : null}
                </div>
                <h1 className="mt-3 text-lg font-semibold">
                    {currentSlot === null ? "区域已完成" : `选择 ${labels[currentSlot]}`}
                </h1>
                <p className="mt-2 text-sm text-base-content/70">{statusMessage}</p>
            </div>
            <div className="absolute right-6 top-6 border-2 border-base-content/30 bg-base-100/80 px-3 py-3 backdrop-blur-md">
                <Button disabled={submitting || currentSlot === null} onClick={() => void cancelSelection()} type="button" variant="secondary">
                    取消
                </Button>
            </div>
        </div>
    );
}
