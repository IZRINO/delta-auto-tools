import { useCallback, useEffect, useMemo, useState } from "react";
import {
  RiArrowDownLine,
  RiArrowUpLine,
  RiDeleteBinLine,
  RiSpeedUpLine,
  RiStarFill,
  RiStarLine,
  RiTimerLine,
} from "@remixicon/react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { toast } from "sonner";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { CardHeader } from "@/components/ui/card";
import { Empty, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle } from "@/components/ui/empty";
import { Field, FieldLabel } from "@/components/ui/field";
import { Switch } from "@/components/ui/switch";
import {
  AppPage,
  CardBody,
  ControlTile,
  PageHero,
  SignalTile,
  TacticalCard,
} from "@/components/app/app-ui";
import type { RapidfireBootstrap, RapidfireCardForm } from "@/components/app/rapidfire-types";
import { rapidfireSettingsToForm } from "@/components/app/rapidfire-types";
import type {
  CounterItem,
  CounterRunState,
  TimerBootstrap,
  TimerItem,
  TimerRunState,
} from "@/components/app/timer-types";
import { timerSettingsToForm } from "@/components/app/timer-utils";
import { useFavorites } from "@/hooks/use-favorites";
import { useNativeShell } from "@/hooks/use-native-shell";
import { cn } from "@/lib/utils";
import { favoriteKey, type FavoriteCardKind, type FavoriteItem, type FavoritesView } from "@/components/app/favorites-utils";

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

export function FavoritesPage({ onNavigate }: FavoritesPageProps) {
  const isNativeShell = useNativeShell();
  const favorites = useFavorites();
  const { items, view, toggleFavorite, moveFavorite, updateView, prune } = favorites;

  const [timerBootstrap, setTimerBootstrap] = useState<TimerBootstrap | null>(null);
  const [rapidfireBootstrap, setRapidfireBootstrap] = useState<RapidfireBootstrap | null>(null);
  const [loading, setLoading] = useState(isNativeShell);

  useEffect(() => {
    if (!isNativeShell) {
      return;
    }
    let disposed = false;
    let unlistenTimerState: (() => void) | undefined;
    let unlistenRapidfireState: (() => void) | undefined;

    setLoading(true);

    void invoke<TimerBootstrap>("timer_get_bootstrap")
      .then((next) => {
        if (!disposed) {
          setTimerBootstrap(next);
        }
      })
      .catch(() => {
        // 浏览器预览 / 调用失败时静默忽略
      });

    void invoke<RapidfireBootstrap>("rapidfire_get_bootstrap")
      .then((next) => {
        if (!disposed) {
          setRapidfireBootstrap(next);
        }
      })
      .catch(() => {
        // 浏览器预览 / 调用失败时静默忽略
      });

    void listen<TimerBootstrap>("timer://state-changed", (event) => {
      if (!disposed) {
        setTimerBootstrap(event.payload);
      }
    }).then((dispose) => {
      unlistenTimerState = dispose;
    });

    void listen<RapidfireBootstrap>("rapidfire://state-changed", (event) => {
      if (!disposed) {
        setRapidfireBootstrap(event.payload);
      }
    }).then((dispose) => {
      unlistenRapidfireState = dispose;
    });

    return () => {
      disposed = true;
      unlistenTimerState?.();
      unlistenRapidfireState?.();
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
    for (const run of timerBootstrap?.runs ?? []) {
      map.set(run.id, run);
    }
    return map;
  }, [timerBootstrap?.runs]);

  const counterRunsById = useMemo(() => {
    const map = new Map<string, CounterRunState>();
    for (const run of timerBootstrap?.counterRuns ?? []) {
      map.set(run.id, run);
    }
    return map;
  }, [timerBootstrap?.counterRuns]);

  // 解析每个收藏项的详情；孤儿收藏（卡片已删除）会被自动清理。
  const details = useMemo(() => {
    const result: Array<{ item: FavoriteItem; detail: FavoriteDetail | null }> = [];
    if (timerSettingsForm === null && rapidfireSettingsForm === null) {
      return result;
    }
    for (const item of items) {
      if (item.kind === "timer") {
        const card = timerSettingsForm?.timers.find((timer) => timer.id === item.cardId);
        if (!card) {
          result.push({ item, detail: null });
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
          detail: { kind: "timer", card: timerItem, run: timerRunsById.get(card.id) },
        });
      } else if (item.kind === "counter") {
        const card = timerSettingsForm?.counters.find((counter) => counter.id === item.cardId);
        if (!card) {
          result.push({ item, detail: null });
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
          detail: { kind: "counter", card: counterItem, run: counterRunsById.get(card.id) },
        });
      } else {
        const card = rapidfireSettingsForm?.cards.find((c) => c.id === item.cardId);
        if (!card) {
          result.push({ item, detail: null });
          continue;
        }
        result.push({ item, detail: { kind: "rapidfire", card } });
      }
    }
    return result;
  }, [items, timerSettingsForm, rapidfireSettingsForm, timerRunsById, counterRunsById]);

  // 在 detail 变化时清理孤儿收藏。
  // 注意：数据未加载完成时（timerSettingsForm 和 rapidfireSettingsForm 均为 null）
  // 不能执行 prune，否则会把所有收藏项当作孤儿清空。
  useEffect(() => {
    // 数据尚未加载完成，跳过 prune
    if (timerSettingsForm === null && rapidfireSettingsForm === null) {
      return;
    }
    const validKeys = new Set<string>();
    for (const entry of details) {
      if (entry.detail) {
        validKeys.add(favoriteKey(entry.item.kind, entry.item.cardId));
      }
    }
    if (validKeys.size === items.length && items.every((item) => validKeys.has(favoriteKey(item.kind, item.cardId)))) {
      return;
    }
    prune(validKeys);
  }, [details, items, prune, timerSettingsForm, rapidfireSettingsForm]);

  const handleMoveUp = useCallback((index: number) => {
    if (index <= 0) {
      return;
    }
    const prev = details[index - 1];
    const current = details[index];
    if (!prev || !current) {
      return;
    }
    moveFavorite(current.item.kind, current.item.cardId, { before: prev.item.cardId });
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
    moveFavorite(current.item.kind, current.item.cardId, { after: next.item.cardId });
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
    return { timerCount, counterCount, rapidfireCount, total: items.length };
  }, [items]);

  if (details.length === 0) {
    return (
      <AppPage>
        <PageHero
          eyebrow="Cross-Tool Collection"
          title="收藏"
          description="在计时\计数器与连发器工具里点击卡片头部的星标即可加入收藏；这里汇总所有收藏、可以跳转和排序。"
          badges={
            <>
              <Badge variant="secondary">跨工具</Badge>
              <Badge variant="outline">{summary.total} 项</Badge>
            </>
          }
          stats={
            <>
              <SignalTile label="计时器" value={summary.timerCount} detail="已收藏的计时器数量" />
              <SignalTile label="计数器" value={summary.counterCount} detail="已收藏的计数器数量" />
              <SignalTile label="连发器" value={summary.rapidfireCount} detail="已收藏的连发器数量" />
            </>
          }
        />
        <TacticalCard>
          <Empty className="min-h-[320px] rounded-xl border border-dashed border-[var(--surface-border)] bg-[linear-gradient(145deg,var(--surface-card-strong),var(--surface-tile))] backdrop-blur-md">
            <EmptyMedia variant="icon">
              <RiStarLine />
            </EmptyMedia>
            <EmptyHeader>
              <EmptyTitle>还没有收藏</EmptyTitle>
              <EmptyDescription>去计时\计数器或连发器工具里，点击卡片头部的星标即可加入收藏。</EmptyDescription>
            </EmptyHeader>
          </Empty>
        </TacticalCard>
      </AppPage>
    );
  }

  return (
    <AppPage>
      <PageHero
        eyebrow="Cross-Tool Collection"
        title="收藏"
        description="在计时\计数器与连发器工具里点击卡片头部的星标即可加入收藏；这里汇总所有收藏、可以跳转和排序。"
        badges={
          <>
            <Badge variant="secondary">跨工具</Badge>
            <Badge variant="outline">{summary.total} 项</Badge>
            {loading ? <Badge variant="outline">同步中…</Badge> : null}
          </>
        }
        stats={
          <>
            <SignalTile label="计时器" value={summary.timerCount} detail="已收藏的计时器数量" />
            <SignalTile label="计数器" value={summary.counterCount} detail="已收藏的计数器数量" />
            <SignalTile label="连发器" value={summary.rapidfireCount} detail="已收藏的连发器数量" />
          </>
        }
        actions={
          <Field orientation="horizontal" className="flex flex-wrap items-center gap-3">
            <Switch
              id="fav-show-hotkey"
              checked={view.showHotkey}
              onCheckedChange={(checked) => updateView({ showHotkey: checked })}
            />
            <FieldLabel htmlFor="fav-show-hotkey" className="cursor-pointer">显示快捷键</FieldLabel>
            <Switch
              id="fav-compact"
              checked={view.compactMode}
              onCheckedChange={(checked) => updateView({ compactMode: checked })}
            />
            <FieldLabel htmlFor="fav-compact" className="cursor-pointer">紧凑模式</FieldLabel>
          </Field>
        }
      />

      <div className="grid gap-3 md:grid-cols-3">
        <ControlTile className="flex flex-col gap-1">
          <div className="flex items-center gap-2">
            <Switch
              id="fav-show-progress"
              checked={view.showProgress}
              onCheckedChange={(checked) => updateView({ showProgress: checked })}
            />
            <span className="text-sm font-medium text-foreground">显示计时器进度</span>
          </div>
          <p className="text-xs text-muted-foreground">开启后计时器收藏卡片会显示当前进度条与剩余秒数。</p>
        </ControlTile>
        <ControlTile className="flex flex-col gap-1">
          <div className="flex items-center gap-2">
            <Switch
              id="fav-show-counter"
              checked={view.showCounter}
              onCheckedChange={(checked) => updateView({ showCounter: checked })}
            />
            <span className="text-sm font-medium text-foreground">显示计数器当前值</span>
          </div>
          <p className="text-xs text-muted-foreground">开启后计数器收藏卡片会显示当前值与起始数。</p>
        </ControlTile>
        <ControlTile className="flex flex-col gap-1">
          <div className="flex items-center gap-2">
            <Badge variant="secondary">跨工具</Badge>
            <span className="text-sm font-medium text-foreground">{summary.total} 项已收藏</span>
          </div>
          <p className="text-xs text-muted-foreground">点击卡片跳转回对应工具页面并高亮该卡片。</p>
        </ControlTile>
      </div>

      <section className="grid gap-3 xl:grid-cols-2">
        {details.map(({ item, detail }, index) => (
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

function FavoriteCard({ detail, index, item, onMoveDown, onMoveUp, onNavigate, onRemove, total, view }: FavoriteCardProps) {
  if (!detail) {
    return (
      <TacticalCard className="opacity-60">
        <CardBody className="flex flex-col gap-3">
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-2">
              <RiStarLine className="text-muted-foreground" />
              <span className="text-sm font-medium text-foreground">已失效的收藏</span>
              <Badge variant="outline">{kindLabel(item.kind)}</Badge>
            </div>
            <Button size="icon-sm" variant="ghost" onClick={onRemove} aria-label="移除收藏">
              <RiDeleteBinLine />
            </Button>
          </div>
          <p className="text-xs text-muted-foreground">该卡片已被删除。点击移除清理此条目。</p>
        </CardBody>
      </TacticalCard>
    );
  }

  return (
    <button
      className="group flex flex-col rounded-xl border border-[var(--surface-border)] bg-[linear-gradient(145deg,var(--surface-card-strong),var(--surface-tile))] p-0 text-left transition-all hover:border-primary/40 hover:shadow-md focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary/60"
      onClick={onNavigate}
      type="button"
    >
      <CardHeader className="flex flex-row items-center justify-between gap-3 border-b border-[var(--surface-border)] bg-[linear-gradient(180deg,var(--surface-muted),transparent)] p-4 pb-3">
        <div className="flex min-w-0 items-center gap-3">
          <span className="flex size-9 shrink-0 items-center justify-center rounded-lg bg-primary text-primary-foreground">
            {detail.kind === "rapidfire" ? <RiSpeedUpLine /> : <RiTimerLine />}
          </span>
          <div className="min-w-0">
            <p className="truncate text-sm font-semibold text-foreground">{detail.card.name || defaultName(detail)}</p>
            <p className="mt-0.5 font-mono text-[0.62rem] tracking-[0.18em] text-muted-foreground uppercase">
              {kindLabel(detail.kind)} · {String(index + 1).padStart(2, "0")} / {String(total).padStart(2, "0")}
            </p>
          </div>
        </div>
        <div className="flex items-center gap-1">
          <Button aria-label="上移" disabled={index === 0} onClick={(event) => { event.stopPropagation(); onMoveUp(); }} size="icon-sm" type="button" variant="ghost">
            <RiArrowUpLine />
          </Button>
          <Button aria-label="下移" disabled={index >= total - 1} onClick={(event) => { event.stopPropagation(); onMoveDown(); }} size="icon-sm" type="button" variant="ghost">
            <RiArrowDownLine />
          </Button>
          <Button aria-label="移除收藏" onClick={(event) => { event.stopPropagation(); onRemove(); }} size="icon-sm" type="button" variant="ghost">
            <RiDeleteBinLine />
          </Button>
        </div>
      </CardHeader>
      <CardBody className="flex flex-col gap-2">
        <div className="flex flex-wrap items-center gap-2">
          {detail.kind === "timer" ? (
            <TimerSummary detail={detail} showProgress={view.showProgress} showHotkey={view.showHotkey} />
          ) : null}
          {detail.kind === "counter" ? (
            <CounterSummary detail={detail} showCounter={view.showCounter} showHotkey={view.showHotkey} />
          ) : null}
          {detail.kind === "rapidfire" ? (
            <RapidfireSummary detail={detail} showHotkey={view.showHotkey} />
          ) : null}
          <span className="ml-auto opacity-0 transition-opacity group-hover:opacity-100">
            <Badge variant="secondary">
              <RiStarFill className="mr-1" />
              点击跳转
            </Badge>
          </span>
        </div>
      </CardBody>
    </button>
  );
}

function TimerSummary({ detail, showProgress, showHotkey }: { detail: TimerFavoriteDetail; showProgress: boolean; showHotkey: boolean }) {
  const { card, run } = detail;
  const totalSeconds = card.segmentCount && card.segmentCount >= 2
    ? card.durationSeconds * card.segmentCount
    : card.durationSeconds;
  const progressPercent = run ? computeProgressPercent(card, run) : 0;
  const directionLabel = card.direction === "countup" ? "正计时" : "倒计时";
  return (
    <div className="flex w-full flex-col gap-1.5">
      <div className="flex flex-wrap items-center gap-2 text-xs text-muted-foreground">
        <Badge variant="secondary">{directionLabel}</Badge>
        <span className="font-mono">{totalSeconds} 秒</span>
        {showHotkey ? <span className="font-mono">快捷键 · {card.hotkey || "未设置"}</span> : null}
        <span className="ml-auto text-foreground">{run ? formatTimerCurrentValue(card, run) : "未运行"}</span>
      </div>
      {showProgress && run ? (
        <div className="h-1.5 w-full overflow-hidden rounded-full bg-[var(--surface-border)]">
          <div
            className={cn("h-full rounded-full bg-primary transition-[width]", progressPercent > 0 ? "" : "opacity-0")}
            style={{ width: `${Math.max(0, Math.min(100, progressPercent))}%` }}
          />
        </div>
      ) : null}
    </div>
  );
}

function CounterSummary({ detail, showCounter, showHotkey }: { detail: CounterFavoriteDetail; showCounter: boolean; showHotkey: boolean }) {
  const { card, run } = detail;
  return (
    <div className="flex w-full flex-wrap items-center gap-2 text-xs text-muted-foreground">
      {showCounter ? (
        <Badge variant="secondary">
          当前 {run?.value ?? card.startValue} · 起点 {card.startValue}
        </Badge>
      ) : (
        <Badge variant="secondary">起点 {card.startValue}</Badge>
      )}
      {showHotkey ? <span className="font-mono">快捷键 · {card.hotkey || "未设置"}</span> : null}
      <span className="ml-auto text-foreground">{card.enabled ? "启用" : "禁用"}</span>
    </div>
  );
}

function RapidfireSummary({ detail, showHotkey }: { detail: RapidfireFavoriteDetail; showHotkey: boolean }) {
  const { card } = detail;
  const interval = card.intervalMs ? Number.parseInt(card.intervalMs, 10) : null;
  return (
    <div className="flex w-full flex-wrap items-center gap-2 text-xs text-muted-foreground">
      <span className="font-mono">{card.triggerKey || "--"} → {card.targetKey || "--"}</span>
      {interval ? <Badge variant="secondary">{interval} ms</Badge> : null}
      <Badge variant={card.skipCompensation ? "outline" : "secondary"}>
        {card.skipCompensation ? "不追加" : "自动补齐"}
      </Badge>
      {showHotkey ? <span className="font-mono">触发 · {card.triggerKey || "未设置"}</span> : null}
      <span className="ml-auto text-foreground">{card.enabled ? "启用" : "禁用"}</span>
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
