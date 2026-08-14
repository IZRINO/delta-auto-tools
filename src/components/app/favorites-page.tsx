import {useCallback, useEffect, useMemo, useState} from "react";
import {
    RiArrowDownLine,
    RiArrowUpLine,
    RiDeleteBinLine,
    RiSpeedUpLine,
    RiStarFill,
    RiStarLine,
    RiTimerLine,
} from "@remixicon/react";
import {invokeLogged as invoke} from "@/lib/logging";
import {COUNTER_EVENTS, RAPIDFIRE_EVENTS, TIMER_EVENTS} from "@/lib/tauri-events";
import {subscribeTauriEvent} from "@/lib/tauri-listener";
import {toast} from "sonner";

import {Badge} from "@/components/ui/badge";
import {Button} from "@/components/ui/button";
import {CardHeader} from "@/components/ui/card";
import {Field, FieldLabel} from "@/components/ui/field";
import {Switch} from "@/components/ui/switch";
import {
    AppPage,
    CardBody,
    ChannelTabs,
    ControlTile,
    MacroHeader,
    SignalTile,
    StatusMatrix,
    TacticalCard,
    TacticalEmptyState,
} from "@/components/app/app-ui";
import type {RapidfireBootstrap, RapidfireCardForm} from "@/components/app/rapidfire-types";
import {rapidfireSettingsToForm} from "@/components/app/rapidfire-types";
import type {
    CounterBootstrap,
    CounterItem,
    CounterRunState,
    CounterRunsChanged,
    TimerBootstrap,
    TimerItem,
    TimerRunState,
    TimerRunsChanged,
} from "@/components/app/timer-types";
import {counterSettingsToForm} from "@/components/app/counter-utils";
import {timerSettingsToForm} from "@/components/app/timer-utils";
import {useFavorites} from "@/hooks/use-favorites";
import {useNativeShell} from "@/hooks/use-native-shell";
import {cn} from "@/lib/utils";
import {
    type FavoriteCardKind,
    type FavoriteItem,
    favoriteKey,
    type FavoritesView,
    settleFavoriteBootstraps,
} from "@/components/app/favorites-utils";

type TimerFavoriteDetail = {
    kind: "timer";
    card: TimerItem;
    run: TimerRunState | undefined;
};

type CounterFavoriteDetail = {
    kind: "counter";
    card: CounterItem;
    run: CounterRunState | undefined;
};

type RapidfireFavoriteDetail = {
    kind: "rapidfire";
    card: RapidfireCardForm;
};

type FavoriteDetail = TimerFavoriteDetail | CounterFavoriteDetail | RapidfireFavoriteDetail;

type FavoritesPageProps = {
    onNavigate: (kind: FavoriteCardKind, cardId: string) => void;
};

export function FavoritesPage({onNavigate}: FavoritesPageProps) {
    const isNativeShell = useNativeShell();
    const favorites = useFavorites();
    const {items, view, toggleFavorite, moveFavorite, updateView, prune} = favorites;

    const [timerBootstrap, setTimerBootstrap] = useState<TimerBootstrap | null>(null);
    const [counterBootstrap, setCounterBootstrap] = useState<CounterBootstrap | null>(null);
    const [rapidfireBootstrap, setRapidfireBootstrap] = useState<RapidfireBootstrap | null>(null);
    const [timerRuntimeRuns, setTimerRuntimeRuns] = useState<TimerRunState[] | null>(null);
    const [counterRuntimeRuns, setCounterRuntimeRuns] = useState<CounterRunState[] | null>(null);
    const [loading, setLoading] = useState(isNativeShell);

    useEffect(() => {
        if (!isNativeShell) {
            return;
        }
        let disposed = false;

        setLoading(true);

        void settleFavoriteBootstraps(
            invoke<TimerBootstrap>("timer_get_bootstrap"),
            invoke<CounterBootstrap>("counter_get_bootstrap"),
            invoke<RapidfireBootstrap>("rapidfire_get_bootstrap"),
        ).then((next) => {
            if (disposed) return;
            setTimerBootstrap(next.timer);
            setCounterBootstrap(next.counter);
            setRapidfireBootstrap(next.rapidfire);
        }).finally(() => {
            if (!disposed) setLoading(false);
        });

        const unlistenTimerState = subscribeTauriEvent<TimerBootstrap>(TIMER_EVENTS.stateChanged, (event) => {
            if (!disposed) {
                setTimerBootstrap(event.payload);
            }
        });

        const unlistenTimerRuns = subscribeTauriEvent<TimerRunsChanged>(
            TIMER_EVENTS.runsChanged,
            (event) => {
                if (!disposed) setTimerRuntimeRuns(event.payload.runs);
            },
            undefined,
            () => {
                void invoke<TimerBootstrap>("timer_get_bootstrap").then((next) => {
                    if (!disposed) setTimerRuntimeRuns((current) => current ?? next.runs);
                }, () => undefined);
            },
        );

        const unlistenRapidfireState = subscribeTauriEvent<RapidfireBootstrap>(RAPIDFIRE_EVENTS.stateChanged, (event) => {
            if (!disposed) {
                setRapidfireBootstrap(event.payload);
            }
        });

        const unlistenCounterState = subscribeTauriEvent<CounterBootstrap>(COUNTER_EVENTS.stateChanged, (event) => {
            if (!disposed) {
                setCounterBootstrap(event.payload);
            }
        });

        const unlistenCounterRuns = subscribeTauriEvent<CounterRunsChanged>(
            COUNTER_EVENTS.runsChanged,
            (event) => {
                if (!disposed) setCounterRuntimeRuns(event.payload.counterRuns);
            },
            undefined,
            () => {
                void invoke<CounterBootstrap>("counter_get_bootstrap").then((next) => {
                    if (!disposed) setCounterRuntimeRuns((current) => current ?? next.counterRuns);
                }, () => undefined);
            },
        );

        return () => {
            disposed = true;
            unlistenTimerState();
            unlistenTimerRuns();
            unlistenCounterState();
            unlistenCounterRuns();
            unlistenRapidfireState();
            setLoading(false);
        };
    }, [isNativeShell]);

    const timerSettingsForm = useMemo(
        () => (timerBootstrap ? timerSettingsToForm(timerBootstrap.settings) : null),
        [timerBootstrap],
    );
    const rapidfireSettingsForm = useMemo(
        () => (rapidfireBootstrap ? rapidfireSettingsToForm(rapidfireBootstrap.settings) : null),
        [rapidfireBootstrap],
    );

    const timerRunsById = useMemo(() => {
        const map = new Map<string, TimerRunState>();
        for (const run of timerRuntimeRuns ?? timerBootstrap?.runs ?? []) {
            map.set(run.id, run);
        }
        return map;
    }, [timerBootstrap?.runs, timerRuntimeRuns]);

    const counterSettingsForm = useMemo(
        () => (counterBootstrap ? counterSettingsToForm(counterBootstrap.settings) : null),
        [counterBootstrap],
    );

    const counterRunsById = useMemo(() => {
        const map = new Map<string, CounterRunState>();
        for (const run of counterRuntimeRuns ?? counterBootstrap?.counterRuns ?? []) {
            map.set(run.id, run);
        }
        return map;
    }, [counterBootstrap?.counterRuns, counterRuntimeRuns]);

    // 解析每个收藏项的详情；孤儿收藏（卡片已删除）会被自动清理。
    const details = useMemo(() => {
        const result: Array<{ item: FavoriteItem; detail: FavoriteDetail | null }> = [];
        if (timerSettingsForm === null && counterSettingsForm === null && rapidfireSettingsForm === null) {
            return result;
        }
        for (const item of items) {
            if (item.kind === "timer") {
                const card = timerSettingsForm?.timers.find((timer: { id: string }) => timer.id === item.cardId);
                if (!card) {
                    result.push({item, detail: null});
                    continue;
                }
                const timerItem: TimerItem = {
                    id: card.id,
                    name: card.name,
                    durationSeconds: Number.parseInt(card.durationSeconds, 10) || 0,
                    hotkey: card.hotkey,
                    direction: card.direction,
                    triggerMode: card.triggerMode,
                    enabled: card.enabled,
                    ignoreRunning: card.ignoreRunning,
                    segmentCount: card.segmentCount === "" ? null : Number.parseInt(card.segmentCount, 10) || null,
                };
                result.push({
                    item,
                    detail: {kind: "timer", card: timerItem, run: timerRunsById.get(card.id)},
                });
            } else if (item.kind === "counter") {
                const card = counterSettingsForm?.counters.find((counter: {
                    id: string
                }) => counter.id === item.cardId);
                if (!card) {
                    result.push({item, detail: null});
                    continue;
                }
                const counterItem: CounterItem = {
                    id: card.id,
                    name: card.name,
                    startValue: Number.parseInt(card.startValue, 10) || 0,
                    hotkey: card.hotkey,
                    enabled: card.enabled,
                };
                result.push({
                    item,
                    detail: {kind: "counter", card: counterItem, run: counterRunsById.get(card.id)},
                });
            } else {
                const card = rapidfireSettingsForm?.cards.find((c) => c.id === item.cardId);
                if (!card) {
                    result.push({item, detail: null});
                    continue;
                }
                result.push({item, detail: {kind: "rapidfire", card}});
            }
        }
        return result;
    }, [items, timerSettingsForm, counterSettingsForm, rapidfireSettingsForm, timerRunsById, counterRunsById]);

    // 三类数据全部 ready 后清理孤儿收藏；任一加载失败时保留现有收藏。
    useEffect(() => {
        if (timerSettingsForm === null || counterSettingsForm === null || rapidfireSettingsForm === null) {
            return;
        }
        prune({
            timer: new Set(timerSettingsForm.timers.map((timer) => timer.id)),
            counter: new Set(counterSettingsForm.counters.map((counter) => counter.id)),
            rapidfire: new Set(rapidfireSettingsForm.cards.map((card) => card.id)),
        });
    }, [counterSettingsForm, prune, rapidfireSettingsForm, timerSettingsForm]);

    const handleMoveUp = useCallback((index: number) => {
        if (index <= 0) {
            return;
        }
        const prev = details[index - 1];
        const current = details[index];
        if (!prev || !current) {
            return;
        }
        moveFavorite(current.item.kind, current.item.cardId, {before: prev.item.cardId});
    }, [details, moveFavorite]);

    const handleMoveDown = useCallback((index: number) => {
        if (index >= details.length - 1) {
            return;
        }
        const next = details[index + 1];
        const current = details[index];
        if (!next || !current) {
            return;
        }
        moveFavorite(current.item.kind, current.item.cardId, {after: next.item.cardId});
    }, [details, moveFavorite]);

    const handleRemove = useCallback((kind: FavoriteCardKind, cardId: string) => {
        toggleFavorite(kind, cardId);
        toast.success("已从收藏移除");
    }, [toggleFavorite]);

    const handleNavigate = useCallback((kind: FavoriteCardKind, cardId: string) => {
        onNavigate(kind, cardId);
    }, [onNavigate]);

    const summary = useMemo(() => {
        let timerCount = 0;
        let counterCount = 0;
        let rapidfireCount = 0;
        for (const item of items) {
            if (item.kind === "timer") timerCount += 1;
            else if (item.kind === "counter") counterCount += 1;
            else rapidfireCount += 1;
        }
        return {timerCount, counterCount, rapidfireCount, total: items.length};
    }, [items]);

    if (details.length === 0) {
        return (
            <AppPage>
                <MacroHeader
                    code="PIN"
                    title="FAVORITES / PINNED"
                    subtitle="在计时器、计数器与连发器工具里点击卡片头部的星标即可加入收藏；这里汇总所有收藏、可以跳转和排序。"
                    badges={
                        <>
                            <Badge variant="secondary">跨工具</Badge>
                            <Badge variant="outline">{summary.total} 项</Badge>
                        </>
                    }
                />
                <TacticalEmptyState className="col-span-12" icon={<RiStarLine/>} title="还没有收藏"
                                    description="去计时器、计数器或连发器工具里，点击卡片头部的星标即可加入收藏。"/>
            </AppPage>
        );
    }

    return (
        <AppPage>
            <MacroHeader
                code="PIN"
                title="FAVORITES / PINNED"
                subtitle="在计时器、计数器与连发器工具里点击卡片头部的星标即可加入收藏；这里汇总所有收藏、可以跳转和排序。"
                badges={
                    <>
                        <Badge variant="secondary">跨工具</Badge>
                        <Badge variant="outline">{summary.total} 项</Badge>
                        {loading ? <Badge variant="outline">同步中…</Badge> : null}
                    </>
                }
                actions={
                    <>
                        <SignalTile label="计时器" value={summary.timerCount} detail="已收藏的计时器数量"/>
                        <SignalTile label="计数器" value={summary.counterCount} detail="已收藏的计数器数量"/>
                        <SignalTile label="连发器" value={summary.rapidfireCount} detail="已收藏的连发器数量"/>
                    </>
                }
            />

            <div className="col-span-12">
                <StatusMatrix items={[
                    {id: "timer", state: summary.timerCount > 0 ? "valid" : "idle", label: "计时器"},
                    {id: "counter", state: summary.counterCount > 0 ? "valid" : "idle", label: "计数器"},
                    {id: "rapidfire", state: summary.rapidfireCount > 0 ? "valid" : "idle", label: "连发器"},
                    {id: "total", state: summary.total > 0 ? "valid" : "warning", label: "总计"},
                    {id: "sync", state: loading ? "active" : "valid", label: "同步状态"},
                    {id: "ready", state: "valid", label: "就绪"},
                ]}/>
            </div>

            <div className="col-span-12">
                <ChannelTabs
                    tabs={[
                        {id: "all", label: "全部", active: true},
                    ]}
                    onTabChange={() => {
                    }}
                />
            </div>

            <ControlTile className="col-span-12 flex flex-wrap items-center gap-x-5 gap-y-2 px-3 py-2">
                <Field orientation="horizontal" className="w-auto gap-2">
                    <Switch id="fav-show-hotkey" checked={view.showHotkey}
                            onCheckedChange={(checked) => updateView({showHotkey: checked})}/>
                    <FieldLabel htmlFor="fav-show-hotkey"
                                className="cursor-pointer font-mono text-caption font-semibold text-base-content">快捷键</FieldLabel>
                </Field>
                <Field orientation="horizontal" className="w-auto gap-2">
                    <Switch id="fav-compact" checked={view.compactMode}
                            onCheckedChange={(checked) => updateView({compactMode: checked})}/>
                    <FieldLabel htmlFor="fav-compact"
                                className="cursor-pointer font-mono text-caption font-semibold text-base-content">紧凑</FieldLabel>
                </Field>
                <Field orientation="horizontal" className="w-auto gap-2">
                    <Switch id="fav-show-progress" checked={view.showProgress}
                            onCheckedChange={(checked) => updateView({showProgress: checked})}/>
                    <FieldLabel htmlFor="fav-show-progress"
                                className="cursor-pointer font-mono text-caption font-semibold text-base-content">计时进度</FieldLabel>
                </Field>
                <Field orientation="horizontal" className="w-auto gap-2">
                    <Switch id="fav-show-counter" checked={view.showCounter}
                            onCheckedChange={(checked) => updateView({showCounter: checked})}/>
                    <FieldLabel htmlFor="fav-show-counter"
                                className="cursor-pointer font-mono text-caption font-semibold text-base-content">计数值</FieldLabel>
                </Field>
            </ControlTile>

            <section className="col-span-12 grid gap-3 xl:grid-cols-12">
                {details.map(({item, detail}, index) => (
                    <FavoriteCard
                        key={favoriteKey(item.kind, item.cardId)}
                        item={item}
                        detail={detail}
                        index={index}
                        total={details.length}
                        view={view}
                        onMoveUp={() => handleMoveUp(index)}
                        onMoveDown={() => handleMoveDown(index)}
                        onRemove={() => handleRemove(item.kind, item.cardId)}
                        onNavigate={() => handleNavigate(item.kind, item.cardId)}
                    />
                ))}
            </section>
        </AppPage>
    );
}

type FavoriteCardProps = {
    item: FavoriteItem;
    detail: FavoriteDetail | null;
    index: number;
    total: number;
    view: FavoritesView;
    onMoveUp: () => void;
    onMoveDown: () => void;
    onRemove: () => void;
    onNavigate: () => void;
};

function FavoriteCard({
                          detail,
                          index,
                          item,
                          onMoveDown,
                          onMoveUp,
                          onNavigate,
                          onRemove,
                          total,
                          view
                      }: FavoriteCardProps) {
    const columnClassName = view.compactMode ? "xl:col-span-4" : "xl:col-span-6";

    if (!detail) {
        return (
            <TacticalCard className={cn("col-span-12 p-0 opacity-60", columnClassName)}>
                <CardBody className="space-y-3 bg-base-200">
                    <div className="flex items-center justify-between gap-3 border-b-2 border-base-content pb-3">
                        <div className="flex items-center gap-2">
                            <RiStarLine className="text-primary"/>
                            <span className="text-sm font-semibold text-base-content">已失效的收藏</span>
                            <Badge variant="outline">{kindLabel(item.kind)}</Badge>
                        </div>
                        <Button size="icon-sm" variant="ghost" onClick={onRemove} aria-label="移除收藏">
                            <RiDeleteBinLine/>
                        </Button>
                    </div>
                    <p className="font-mono text-caption font-bold text-base-content/60">该卡片已被删除。点击移除清理此条目。</p>
                </CardBody>
            </TacticalCard>
        );
    }

    return (
        <button
            className={cn(
                "group col-span-12 border border-base-300 bg-base-100 p-0 text-left transition-colors hover:bg-base-200 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary",
                columnClassName,
            )}
            onClick={onNavigate}
            type="button"
        >
            <CardHeader className="border-b-2 border-base-content bg-base-content p-4 pb-3 text-base-100">
                <div className="flex min-w-0 items-center justify-between gap-3">
                    <div className="flex min-w-0 items-center gap-3">
            <span
                className="flex size-9 shrink-0 items-center justify-center border border-base-100 bg-primary text-base-100">
              {detail.kind === "rapidfire" ? <RiSpeedUpLine/> : <RiTimerLine/>}
            </span>
                        <div className="min-w-0">
                            <p className="truncate text-sm font-semibold text-base-100">{detail.card.name || defaultName(detail)}</p>
                            <p className="mt-0.5 font-mono text-caption font-semibold text-base-200">
                                {kindLabel(detail.kind)} · {String(index + 1).padStart(2, "0")} / {String(total).padStart(2, "0")}
                            </p>
                        </div>
                    </div>
                    <div className="flex items-center gap-1">
                        <Button aria-label="上移" disabled={index === 0} onClick={(event) => {
                            event.stopPropagation();
                            onMoveUp();
                        }} size="icon-sm" type="button" variant="ghost">
                            <RiArrowUpLine/>
                        </Button>
                        <Button aria-label="下移" disabled={index >= total - 1} onClick={(event) => {
                            event.stopPropagation();
                            onMoveDown();
                        }} size="icon-sm" type="button" variant="ghost">
                            <RiArrowDownLine/>
                        </Button>
                        <Button aria-label="移除收藏" onClick={(event) => {
                            event.stopPropagation();
                            onRemove();
                        }} size="icon-sm" type="button" variant="ghost">
                            <RiDeleteBinLine/>
                        </Button>
                    </div>
                </div>
            </CardHeader>
            <CardBody className="space-y-3 bg-base-100">
                <div className="flex flex-wrap items-center gap-2 border-b border-base-content pb-3">
                    {detail.kind === "timer" ? (
                        <TimerSummary detail={detail} showProgress={view.showProgress} showHotkey={view.showHotkey}/>
                    ) : null}
                    {detail.kind === "counter" ? (
                        <CounterSummary detail={detail} showCounter={view.showCounter} showHotkey={view.showHotkey}/>
                    ) : null}
                    {detail.kind === "rapidfire" ? (
                        <RapidfireSummary detail={detail} showHotkey={view.showHotkey}/>
                    ) : null}
                </div>
                <div className="flex items-center justify-between gap-3">
                    <Badge variant="secondary">已收藏</Badge>
                    <span
                        className="flex items-center gap-1 font-mono text-caption font-semibold text-base-content/60">
            <RiStarFill className="size-3.5 text-primary"/>
            点击跳转
          </span>
                </div>
            </CardBody>
        </button>
    );
}

function TimerSummary({detail, showProgress, showHotkey}: {
    detail: TimerFavoriteDetail;
    showProgress: boolean;
    showHotkey: boolean
}) {
    const {card, run} = detail;
    const totalSeconds = card.segmentCount && card.segmentCount >= 2
        ? card.durationSeconds * card.segmentCount
        : card.durationSeconds;
    const progressPercent = run ? computeProgressPercent(card, run) : 0;
    const directionLabel = card.direction === "countup" ? "正计时" : "倒计时";
    return (
        <div className="flex w-full flex-col gap-2">
            <div
                className="flex flex-wrap items-center gap-2 font-mono text-caption font-bold text-base-content/60">
                <Badge variant="secondary">{directionLabel}</Badge>
                <span>{totalSeconds} 秒</span>
                {showHotkey ? <span>快捷键 · {card.hotkey || "未设置"}</span> : null}
                <span
                    className="ml-auto text-base-content">{run ? formatTimerCurrentValue(card, run) : "未运行"}</span>
            </div>
            {showProgress && run ? (
                <div className="h-2 w-full border border-base-content bg-base-200">
                    <div
                        className={cn(
                            "h-full origin-left bg-primary transition-transform duration-200 ease-[cubic-bezier(0.16,1,0.3,1)] motion-reduce:transition-none",
                            progressPercent > 0 ? "" : "opacity-0",
                        )}
                        style={{transform: `scaleX(${Math.max(0, Math.min(1, progressPercent / 100))})`}}
                    />
                </div>
            ) : null}
        </div>
    );
}

function CounterSummary({detail, showCounter, showHotkey}: {
    detail: CounterFavoriteDetail;
    showCounter: boolean;
    showHotkey: boolean
}) {
    const {card, run} = detail;
    return (
        <div
            className="flex w-full flex-wrap items-center gap-2 font-mono text-caption font-bold text-base-content/60">
            {showCounter ? (
                <Badge variant="secondary">
                    当前 {run?.value ?? card.startValue} · 起点 {card.startValue}
                </Badge>
            ) : (
                <Badge variant="secondary">起点 {card.startValue}</Badge>
            )}
            {showHotkey ? <span>快捷键 · {card.hotkey || "未设置"}</span> : null}
            <span className="ml-auto text-base-content">{card.enabled ? "启用" : "禁用"}</span>
        </div>
    );
}

function RapidfireSummary({detail, showHotkey}: { detail: RapidfireFavoriteDetail; showHotkey: boolean }) {
    const {card} = detail;
    const interval = card.intervalMs ? Number.parseInt(card.intervalMs, 10) : null;
    return (
        <div
            className="flex w-full flex-wrap items-center gap-2 font-mono text-caption font-bold text-base-content/60">
            <span>{card.triggerKey || "--"} → {card.targetKey || "--"}</span>
            {interval ? <Badge variant="secondary">{interval} ms</Badge> : null}
            <Badge variant={card.skipCompensation ? "outline" : "secondary"}>
                {card.skipCompensation ? "不追加" : "自动补齐"}
            </Badge>
            {showHotkey ? <span>触发 · {card.triggerKey || "未设置"}</span> : null}
            <span className="ml-auto text-base-content">{card.enabled ? "启用" : "禁用"}</span>
        </div>
    );
}

function kindLabel(kind: FavoriteCardKind): string {
    if (kind === "timer") return "计时器";
    if (kind === "counter") return "计数器";
    return "连发器";
}

function defaultName(detail: FavoriteDetail): string {
    if (detail.kind === "rapidfire") return "连发器";
    return kindLabel(detail.kind);
}

function computeProgressPercent(card: TimerItem, run: TimerRunState): number {
    if (!run.startedAtMs) {
        return 0;
    }
    if (card.segmentCount && card.segmentCount >= 2) {
        const duration = run.durationSeconds * 1000;
        const poolMs = Math.min(duration, run.recoveryStartPool * 1000 + (Date.now() - run.startedAtMs));
        return Math.max(0, Math.min(100, (poolMs / duration) * 100));
    }
    const durationMs = run.durationSeconds * 1000;
    if (card.direction === "countup") {
        return Math.max(0, Math.min(100, ((Date.now() - run.startedAtMs) / durationMs) * 100));
    }
    return Math.max(0, Math.min(100, ((run.startedAtMs + durationMs - Date.now()) / durationMs) * 100));
}

function formatTimerCurrentValue(card: TimerItem, run: TimerRunState): string {
    if (run.status === "finished") {
        if (card.direction === "countup") {
            return String(run.durationSeconds);
        }
        return "0";
    }
    if (card.segmentCount && card.segmentCount >= 2) {
        const durationMs = run.durationSeconds * 1000;
        const poolMs = Math.min(durationMs, run.recoveryStartPool * 1000 + (Date.now() - run.startedAtMs));
        return String(Math.floor(poolMs / 1000));
    }
    const durationMs = run.durationSeconds * 1000;
    if (card.direction === "countup") {
        const elapsed = Math.min(durationMs, Date.now() - run.startedAtMs);
        return String(Math.floor(elapsed / 1000));
    }
    const remaining = Math.max(0, run.startedAtMs + durationMs - Date.now());
    return String(Math.ceil(remaining / 1000));
}
