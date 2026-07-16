import {useCallback, useEffect, useMemo, useState} from "react";
import {invokeLogged as invoke} from "@/lib/logging";

import {Badge} from "@/components/ui/badge";
import {Button} from "@/components/ui/button";
import {
    CLICK_REGION_LABELS,
    EMPTY_REGIONS,
    MIN_SELECTION_HEIGHT,
    MIN_SELECTION_WIDTH,
    type Point,
    REGION_LABELS,
    type RegionSelectionProgress,
    type RegionTuple,
} from "@/components/app/morse-types";
import {getErrorMessage, getSelectionRect, parseOverlayTarget} from "@/components/app/morse-utils";
import {getSettingsRevision} from "@/components/app/profile-utils";
import {useProfile} from "@/hooks/use-profile";

export function RegionSelectionOverlay({slots}: { slots: number[] }) {
    const {bootstrap: profileBootstrap} = useProfile();
    const target = useMemo(() => parseOverlayTarget(), []);
    const labels = target === "click" ? CLICK_REGION_LABELS : REGION_LABELS;
    const [dragStart, setDragStart] = useState<Point | null>(null);
    const [dragCurrent, setDragCurrent] = useState<Point | null>(null);
    const [regions, setRegions] = useState<RegionTuple>(EMPTY_REGIONS);
    const [completedSlots, setCompletedSlots] = useState<number[]>([]);
    const [currentSlot, setCurrentSlot] = useState<number | null>(slots[0] ?? null);
    const [statusMessage, setStatusMessage] = useState(
        target === "click" ? "拖拽框选点击区域，Enter 保存已选，Esc 取消" : "拖拽框选区域，Enter 完成，Esc 取消",
    );
    const [submitting, setSubmitting] = useState(false);

    const currentRect = useMemo(() => {
        if (!dragStart || !dragCurrent) {
            return null;
        }

        return getSelectionRect(dragStart, dragCurrent);
    }, [dragCurrent, dragStart]);

    const activeStep = currentSlot === null ? slots.length : completedSlots.length + 1;

    const cancelSelection = useCallback(async () => {
        if (currentSlot === null || submitting) {
            return;
        }

        setSubmitting(true);
        setStatusMessage("正在取消区域选择...");

        try {
            await invoke("morse_overlay_cancel_selection", {slot: currentSlot});
        } catch (error) {
            setStatusMessage(getErrorMessage(error));
            setSubmitting(false);
        }
    }, [currentSlot, submitting]);


    const finishEarly = useCallback(async () => {
        if (completedSlots.length === 0 || submitting) {
            return;
        }

        setSubmitting(true);
        setStatusMessage("正在提交已选区域...");

        try {
            await invoke("morse_overlay_finish_early", {
                settingsRevision: getSettingsRevision(profileBootstrap),
            });
        } catch (error) {
            setStatusMessage(getErrorMessage(error));
            setSubmitting(false);
        }
    }, [completedSlots, profileBootstrap, submitting]);

    useEffect(() => {
        const handleKeyDown = (event: KeyboardEvent) => {
            if (event.key === "Escape") {
                event.preventDefault();
                void cancelSelection();
            } else if (event.key === "Enter" && completedSlots.length > 0 && !submitting) {
                event.preventDefault();
                void finishEarly();
            }
        };

        window.addEventListener("keydown", handleKeyDown);
        return () => window.removeEventListener("keydown", handleKeyDown);
    }, [cancelSelection, completedSlots, finishEarly, submitting]);
    const handleMouseDown = (event: React.MouseEvent<HTMLDivElement>) => {
        if (currentSlot === null || submitting || event.button !== 0) {
            return;
        }

        const point = {x: event.clientX, y: event.clientY};
        setDragStart(point);
        setDragCurrent(point);
        setStatusMessage(`正在框选 ${labels[currentSlot]}...`);
    };

    const handleMouseMove = (event: React.MouseEvent<HTMLDivElement>) => {
        if (!dragStart || submitting) {
            return;
        }

        setDragCurrent({x: event.clientX, y: event.clientY});
    };

    const handleMouseUp = async (event: React.MouseEvent<HTMLDivElement>) => {
        if (!dragStart || currentSlot === null || submitting) {
            return;
        }

        const rect = getSelectionRect(dragStart, {
            x: event.clientX,
            y: event.clientY,
        });

        setDragStart(null);
        setDragCurrent(null);

        if (rect.width <= MIN_SELECTION_WIDTH || rect.height <= MIN_SELECTION_HEIGHT) {
            setStatusMessage("区域太小，请重新框选。");
            return;
        }

        setSubmitting(true);

        try {
            const progress = await invoke<RegionSelectionProgress>("morse_overlay_submit_selection", {
                slot: currentSlot,
                rect,
                settingsRevision: getSettingsRevision(profileBootstrap),
            });
            setRegions(progress.regions);
            setCompletedSlots(progress.completedSlots);
            setCurrentSlot(progress.currentSlot);

            if (progress.currentSlot === null) {
                setStatusMessage(target === "click" ? "点击区域已保存，正在返回主界面..." : "3 个区域已保存，正在返回主界面...");
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
            className="fixed inset-0 cursor-crosshair select-none text-white"
            onContextMenu={(event) => {
                event.preventDefault();
                void cancelSelection();
            }}
            onMouseDown={handleMouseDown}
            onMouseMove={handleMouseMove}
            onMouseUp={handleMouseUp}
        >
            {regions.map((region, index) => {
                if (!region) {
                    return null;
                }

                const isCurrent = currentSlot === index;
                return (
                    <div
                        key={index}
                        className={isCurrent ? "pointer-events-none absolute border-2 border-primary bg-primary/12" : "pointer-events-none absolute border border-white/85 bg-white/10"}
                        style={{
                            left: region.x,
                            top: region.y,
                            width: region.width,
                            height: region.height,
                        }}
                    />
                );
            })}

            {currentRect ? (
                <div
                    className="pointer-events-none absolute border-2 border-primary bg-primary/16"
                    style={{
                        left: currentRect.x,
                        top: currentRect.y,
                        width: currentRect.width,
                        height: currentRect.height,
                    }}
                />
            ) : null}

            <div
                className="pointer-events-none absolute left-6 top-6 max-w-md border-2 border-white/40 bg-background/88 px-4 py-4 text-foreground backdrop-blur-md">
                <div className="flex items-center gap-2">
                    <Badge variant="outline">{`第 ${activeStep} / ${slots.length} 步`}</Badge>
                    {currentSlot !== null ? <Badge variant="secondary">{labels[currentSlot]}</Badge> : null}
                    {completedSlots.length > 0 ? <Badge variant="outline">已完成 {completedSlots.length}</Badge> : null}
                </div>
                <h1 className="mt-3 text-lg font-semibold text-foreground">
                    {currentSlot === null ? "区域已完成" : `选择 ${labels[currentSlot]}`}
                </h1>
                <p className="mt-2 text-sm text-muted-foreground">{statusMessage}</p>
                {currentRect ? (
                    <p className="mt-3 border border-border/70 bg-background/80 px-3 py-2 font-mono text-xs text-muted-foreground">
                        {`X ${currentRect.x} · Y ${currentRect.y} · W ${currentRect.width} · H ${currentRect.height}`}
                    </p>
                ) : (
                    <p className="mt-3 text-xs text-muted-foreground">拖拽鼠标完成当前区域，Enter 提交，Esc
                        或右键取消。</p>
                )}
            </div>

            <div
                className="absolute right-6 top-6 flex items-center gap-2 border-2 border-white/30 bg-background/80 px-3 py-3 backdrop-blur-md">
                {completedSlots.map((slot) => (
                    <Badge key={slot} variant="secondary">
                        {labels[slot]}
                    </Badge>
                ))}
                <Button disabled={submitting || currentSlot === null} onClick={() => void cancelSelection()}
                        type="button" variant="secondary">
                    取消
                </Button>
            </div>
        </div>
    );
}
