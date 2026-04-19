import { useCallback, useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  EMPTY_REGIONS,
  MIN_SELECTION_HEIGHT,
  MIN_SELECTION_WIDTH,
  REGION_LABELS,
  type Point,
  type RegionSelectionProgress,
  type RegionTuple,
} from "@/components/app/morse-types";
import { getErrorMessage, getSelectionRect } from "@/components/app/morse-utils";

export function RegionSelectionOverlay({ slots }: { slots: number[] }) {
  const [dragStart, setDragStart] = useState<Point | null>(null);
  const [dragCurrent, setDragCurrent] = useState<Point | null>(null);
  const [regions, setRegions] = useState<RegionTuple>(EMPTY_REGIONS);
  const [completedSlots, setCompletedSlots] = useState<number[]>([]);
  const [currentSlot, setCurrentSlot] = useState<number | null>(slots[0] ?? null);
  const [statusMessage, setStatusMessage] = useState("拖拽框选当前区域，Esc 或右键取消。");
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
      await invoke("morse_overlay_cancel_selection", { slot: currentSlot });
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
    if (currentSlot === null || submitting || event.button !== 0) {
      return;
    }

    const point = { x: event.clientX, y: event.clientY };
    setDragStart(point);
    setDragCurrent(point);
    setStatusMessage(`正在框选 ${REGION_LABELS[currentSlot]}...`);
  };

  const handleMouseMove = (event: React.MouseEvent<HTMLDivElement>) => {
    if (!dragStart || submitting) {
      return;
    }

    setDragCurrent({ x: event.clientX, y: event.clientY });
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
      });
      setRegions(progress.regions);
      setCompletedSlots(progress.completedSlots);
      setCurrentSlot(progress.currentSlot);

      if (progress.currentSlot === null) {
        setStatusMessage("3 个区域已保存，正在返回主界面...");
        return;
      }

      setStatusMessage(`${REGION_LABELS[currentSlot]} 已保存，请继续框选 ${REGION_LABELS[progress.currentSlot]}。`);
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
            className={isCurrent ? "pointer-events-none absolute rounded-md border-2 border-primary bg-primary/12 shadow-[0_0_0_1px_rgba(255,255,255,0.32)]" : "pointer-events-none absolute rounded-md border border-white/85 bg-white/10"}
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
          className="pointer-events-none absolute rounded-md border-2 border-primary bg-primary/16 shadow-[0_0_0_1px_rgba(255,255,255,0.32)]"
          style={{
            left: currentRect.x,
            top: currentRect.y,
            width: currentRect.width,
            height: currentRect.height,
          }}
        />
      ) : null}

      <div className="pointer-events-none absolute left-6 top-6 max-w-md rounded-2xl border border-white/30 bg-background/88 px-4 py-4 text-foreground shadow-2xl backdrop-blur-md">
        <div className="flex items-center gap-2">
          <Badge variant="outline">{`第 ${activeStep} / ${slots.length} 步`}</Badge>
          {currentSlot !== null ? <Badge variant="secondary">{REGION_LABELS[currentSlot]}</Badge> : null}
          {completedSlots.length > 0 ? <Badge variant="outline">已完成 {completedSlots.length}</Badge> : null}
        </div>
        <h1 className="mt-3 text-lg font-semibold text-foreground">
          {currentSlot === null ? "区域已完成" : `选择 ${REGION_LABELS[currentSlot]}`}
        </h1>
        <p className="mt-2 text-sm text-muted-foreground">{statusMessage}</p>
        {currentRect ? (
          <p className="mt-3 rounded-lg border border-border/70 bg-background/80 px-3 py-2 font-mono text-xs text-muted-foreground">
            {`X ${currentRect.x} · Y ${currentRect.y} · W ${currentRect.width} · H ${currentRect.height}`}
          </p>
        ) : (
          <p className="mt-3 text-xs text-muted-foreground">拖拽鼠标完成当前区域，右键或 Esc 可取消本轮选择。</p>
        )}
      </div>

      <div className="absolute right-6 top-6 flex items-center gap-2 rounded-2xl border border-white/25 bg-background/80 px-3 py-3 shadow-xl backdrop-blur-md">
        {completedSlots.map((slot) => (
          <Badge key={slot} variant="secondary">
            {REGION_LABELS[slot]}
          </Badge>
        ))}
        <Button disabled={submitting || currentSlot === null} onClick={() => void cancelSelection()} type="button" variant="secondary">
          取消
        </Button>
      </div>
    </div>
  );
}
