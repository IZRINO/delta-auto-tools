import { useEffect, useMemo, useState } from "react";

import type { TimerBootstrap, CounterBootstrap, TimerRunState } from "@/components/app/timer-types";
import { timerRunsById, isTimerRunActive, timerProgressPercent, useTimerOverlayBootstrap } from "@/components/app/timer-utils";
import { counterRunsById, useCounterOverlayBootstrap } from "@/components/app/counter-utils";
import { PositionOverlay } from "@/components/ui/position-overlay";
import { Progress } from "@/components/ui/progress";
import { cn } from "@/lib/utils";

/* ───── Timer Display Overlay ───── */

export function TimerDisplayOverlay({ groupId, isNativeShell }: { groupId: string; isNativeShell: boolean }) {
  const [bootstrap, setBootstrap] = useState<TimerBootstrap | null>(null);
  const [now, setNow] = useState(Date.now);

  useTimerOverlayBootstrap(isNativeShell, setBootstrap);

  useEffect(() => {
    let rafId: number;
    const tick = () => {
      setNow(Date.now());
      rafId = requestAnimationFrame(tick);
    };
    rafId = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(rafId);
  }, []);

  const runsById = useMemo(() => timerRunsById(bootstrap?.runs ?? []), [bootstrap?.runs]);
  const group = bootstrap?.settings.timerGroups?.find((item) => item.id === groupId);
  const opacity = group?.display.fontOpacity ?? bootstrap?.settings.display.fontOpacity ?? 0.92;

  function smoothProgress(run: TimerRunState | undefined): number {
    if (!run || !run.startedAtMs || run.status === "finished") {
      return timerProgressPercent(run, 0);
    }
    if (run.segmentCount != null && run.segmentCount >= 2) {
      const durationMs = run.durationSeconds * 1000;
      const poolMs = run.recoveryStartPool * 1000 + (now - run.startedAtMs);
      const cappedPoolMs = Math.max(0, Math.min(durationMs, poolMs));
      if (run.direction === "countdown") {
        return Math.max(0, Math.min(100, ((durationMs - cappedPoolMs) / durationMs) * 100));
      }
      return Math.max(0, Math.min(100, (cappedPoolMs / durationMs) * 100));
    }
    const durationMs = run.durationSeconds * 1000;
    if (run.direction === "countup") {
      return Math.max(0, Math.min(100, ((now - run.startedAtMs) / durationMs) * 100));
    }
    return Math.max(0, Math.min(100, ((run.startedAtMs + durationMs - now) / durationMs) * 100));
  }

  function smoothDisplayValue(run: TimerRunState | undefined): string {
    if (!run) return "";
    if (!run.startedAtMs || run.status === "finished") {
      if (run.segmentCount != null && run.segmentCount >= 2 && run.direction === "countdown") {
        return Math.floor(run.durationSeconds - run.currentSeconds).toString();
      }
      return Math.floor(run.currentSeconds).toString();
    }
    if (run.segmentCount != null && run.segmentCount >= 2) {
      const durationMs = run.durationSeconds * 1000;
      const poolMs = Math.min(durationMs, run.recoveryStartPool * 1000 + (now - run.startedAtMs));
      if (run.direction === "countdown") {
        return Math.ceil((durationMs - poolMs) / 1000).toString();
      }
      return Math.floor(poolMs / 1000).toString();
    }
    const durationMs = run.durationSeconds * 1000;
    if (run.direction === "countdown") {
      const remainingMs = Math.max(0, run.startedAtMs + durationMs - now);
      return Math.ceil(remainingMs / 1000).toString();
    }
    const elapsedMs = Math.min(durationMs, now - run.startedAtMs);
    return Math.floor(elapsedMs / 1000).toString();
  }

  return (
    <div className="flex h-screen w-screen items-start justify-start overflow-hidden bg-transparent p-2 font-mono text-white" style={{ opacity }}>
      <div className="h-full w-full overflow-hidden rounded-md border border-white/20 bg-black/20 px-3 py-2 backdrop-blur-[1px]">
        {bootstrap?.settings.timers.filter((t) => t.enabled && t.groupId === groupId && (group?.enabled ?? true)).map((timer) => {
          const run = runsById.get(timer.id);
          const finished = run?.status === "finished";
          const isActive = isTimerRunActive(run);
          const isMultiSegment = timer.segmentCount != null && timer.segmentCount >= 2;
          const progress = smoothProgress(run);
          let displayValue: string;
          if (!run) {
            if (isMultiSegment) {
              const total = timer.segmentCount! * timer.durationSeconds;
              displayValue = String(total);
            } else {
              displayValue = String(timer.durationSeconds);
            }
          } else {
            displayValue = smoothDisplayValue(run);
          }
          return (
            <div key={timer.id} className={cn("relative my-0.5 min-w-0 overflow-hidden rounded-md px-2 py-0.5 text-base font-semibold tracking-wide", isActive ? "bg-primary/20 ring-1 ring-primary/70" : "")}>
              {(run && !isMultiSegment) || isMultiSegment ? (
                <Progress aria-label={`${timer.name} 进度`} className="absolute inset-0 h-full rounded-md bg-white/20 [&_[data-slot=progress-indicator]]:bg-[var(--rust)]" value={progress} />
              ) : null}
              <div className="relative flex min-w-0 items-center justify-between gap-3">
                <span className="flex min-w-0 items-center gap-1.5">
                  {isActive ? <span aria-hidden="true" className="h-1.5 w-1.5 shrink-0 animate-pulse rounded-full bg-primary" /> : null}
                  <span className={cn("min-w-0 truncate", finished && !isMultiSegment ? "text-primary-foreground italic" : "text-white")}>{timer.name}</span>
                  <span className={cn("shrink-0 font-mono text-xs", isActive ? "text-primary" : finished ? "text-[var(--amber)]" : "text-white/60")}>
                    {isActive ? "RUNNING" : finished ? "FINISHED" : "IDLE"}
                  </span>
                </span>
                <span className={finished && !isMultiSegment ? "shrink-0 text-primary-foreground italic" : "shrink-0 text-white"}>{displayValue}</span>
              </div>
            </div>
          );
        })}
      </div>
    </div>
  );
}

/* ───── Counter Display Overlay ───── */

export function CounterDisplayOverlay({ groupId, isNativeShell }: { groupId: string; isNativeShell: boolean }) {
  const [bootstrap, setBootstrap] = useState<CounterBootstrap | null>(null);

  useCounterOverlayBootstrap(isNativeShell, setBootstrap);

  const counterRunsByIdMap = useMemo(() => counterRunsById(bootstrap?.counterRuns ?? []), [bootstrap?.counterRuns]);
  const group = bootstrap?.settings.counterGroups?.find((item) => item.id === groupId);
  const opacity = group?.display.fontOpacity ?? bootstrap?.settings.display.fontOpacity ?? 0.92;

  return (
    <div className="flex h-screen w-screen items-start justify-start overflow-hidden bg-transparent p-2 font-mono text-white" style={{ opacity }}>
      <div className="h-full w-full overflow-hidden rounded-md border border-white/20 bg-black/20 px-3 py-2 backdrop-blur-[1px]">
        {bootstrap?.settings.counters.filter((c) => c.enabled && c.groupId === groupId && (group?.enabled ?? true)).map((counter) => {
          const run = counterRunsByIdMap.get(counter.id);
          return (
            <div key={counter.id} className="flex min-w-0 items-center justify-between gap-3 py-0.5 text-base font-semibold tracking-wide">
              <span className="min-w-0 truncate text-white">{counter.name}</span>
              <span className="shrink-0 text-white">{run?.value ?? counter.startValue}</span>
            </div>
          );
        })}
      </div>
    </div>
  );
}

/* ───── Position Overlays ───── */

export function TimerPositionOverlay({ isNativeShell }: { isNativeShell: boolean }) {
  return (
    <PositionOverlay
      isNativeShell={isNativeShell}
      label="计时器"
      commands={{
        commit: "timer_position_commit",
        cancel: "timer_position_cancel",
        moved: "timer_position_moved",
      }}
      initialStatusSuffix="关闭计时器总开关后透明窗口会隐藏并解绑快捷键。"
    />
  );
}

export function CounterPositionOverlay({ isNativeShell }: { isNativeShell: boolean }) {
  return (
    <PositionOverlay
      isNativeShell={isNativeShell}
      label="计数器"
      commands={{
        commit: "counter_position_commit",
        cancel: "counter_position_cancel",
        moved: "counter_position_moved",
      }}
      initialStatusSuffix="关闭计数器总开关后透明窗口会隐藏并解绑快捷键。"
    />
  );
}
