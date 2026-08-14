import {Fragment, type ComponentProps, useEffect, useRef, useState} from "react";
import {open} from "@tauri-apps/plugin-dialog";
import {
    RiAddLine,
    RiArrowDownLine,
    RiArrowUpLine,
    RiCrosshair2Line,
    RiDeleteBinLine,
    RiFolderOpenLine,
    RiPauseLine,
    RiPlayLine,
    RiRefreshLine,
    RiShieldCheckLine,
    RiStopCircleLine,
} from "@remixicon/react";

import {Button} from "@/components/ui/button";
import {Input} from "@/components/ui/input";
import {Switch} from "@/components/ui/switch";
import {LatestSaveQueue} from "@/hooks/autosave-queue";
import {useHotkeyRecorder} from "@/hooks/use-hotkey-recorder";
import {useNativeShell} from "@/hooks/use-native-shell";
import {invokeLogged as invoke} from "@/lib/logging";
import {SPECIAL_OPS_EVENTS} from "@/lib/tauri-events";
import {subscribeTauriEvent} from "@/lib/tauri-listener";
import {
    STATION_LABELS,
    reloadSpecialOpsAfterStateChanged,
    testSpecialOpsCalibrationTarget,
     type AccountPlan,
     type AmmoCorrectionInput,
     type AmmoBusinessTarget,
    type CalibrationEnvironment,
    type CalibrationTarget,
     type LoginRunSnapshot,
     type LimitedSupplyColorTestResult,
     type LimitedSupplyOutcome,
     type LimitedSupplySettings,
     type MarketBusinessConfig,
     type MarketPurchaseSettings,
    type ProfitConfigurationUpdate,
    type SpecialOpsBootstrap,
    type SpecialOpsSettings,
     type SpecialOpsStateChanged,
     type StationKind,
     type StationBusinessConfig,
     type TimelineTask,
    type ManualStationState,
    type StationCorrectionInput,
    type StationPlan,
} from "@/components/app/special-ops-types";
import {formatRecordedHotkey} from "@/components/app/morse-utils";
import {SpecialOpsProfitFilter} from "@/components/app/special-ops-profit-filter";
import {
     accountRestorable,
     applySpecialOpsBootstrapUpdate,
     applyExecutableSelection,
     buildInlineStationCorrection,
     buildTimelineHourSlots,
     changeAmmoTargetSeasonal,
     createInlineStationCorrectionDraft,
     createStationRemainingTimeDraft,
     eligibleLoginTrialAccounts,
     formatCalibrationTemplateTestResult,
     groupTimelineTasks,
     hasActiveSpecialOpsRun,
     insertNormalAmmoTarget,
     limitedColorToHex,
     moveAmmoTargetWithinGroup,
     timelineTaskAllowsInlineCorrection,
    parseNavigationDelayMs,
    parseLimitedColorHex,
    persistSpecialOpsSaveRequest,
    shanghaiDay,
     specialOpsErrorAfterUpdate,
     timelineDelayMinutes,
    type SpecialOpsBootstrapUpdate,
    type SpecialOpsSaveRequest,
} from "@/components/app/special-ops-utils";

const SAVE_DELAY_MS = 400;

const stationKinds: StationKind[] = ["technicalCenter", "workbench", "pharmacy", "armorBench"];
const emptyBootstrap: SpecialOpsBootstrap = {
    settings: {
        enabled: true,
        paused: true,
        dailyExchangeTime: "08:00",
        emergencyHotkey: "Ctrl+Shift+F12",
        navigationBeaconDelayMs: 3000,
        navigationSpaceDelayMs: 3000,
        navigationTabDelayMs: 3000,
        navigationSpecialOpsDelayMs: 3000,
        ammoSupplyDelayMs: 3000,
        ammoTacticalDelayMs: 3000,
        craftSpaceDelayMs: 3000,
        craftReopenDelayMs: 3000,
        craftConfirmPinnedDelayMs: 3000,
         wegameExecutablePath: "",
         gameExecutablePath: "",
         defaultBusinessConfig: {
             stations: stationKinds.map((kind) => ({kind, enabled: true, durationMinutes: 60, recipeNote: ""})),
             recipePoints: [],
             ammoTargets: [],
             market: {schemaVersion: 1, enabled: false, purchaseCount: 1, itemNote: "", productPoint: null, maxPrice: 1},
         },
         profitFilter: {enabled: false, cutoffTime: "17:00", rules: [], audits: [], cutoffState: null},
         limitedSupply: {enabled: false, researchDelayMs: 3000, readyTimeoutMs: 10000, colors: [[0, 0, 0], [255, 255, 255]], colorTolerances: [30, 30]},
         marketPurchase: {enabled: false, entryDelayMs: 3000, purchaseCount: 1, itemNote: "", windowStartMinute: 120, windowEndMinute: 1200},
         accounts: [],
        activeCalibrationId: null,
        calibrationEnvironments: [],
    },
    schedule: {dueAccounts: [], nextWakeAtMs: null, timelineStartMs: Date.now(), timelineEndMs: Date.now() + 24 * 60 * 60_000, timelineTasks: []},
    settingsRevision: 0,
    nowMs: Date.now(),
    runSnapshot: null,
    profitRuntime: {
        phase: "disabled",
        nextQueryAtMs: null,
        queryAttempt: null,
        qualifiedRuleIds: [],
        currentSessionRuleIds: [],
        activeRoundTargets: [],
        lastSummary: null,
        configurationError: null,
    },
};

function createStation(kind: StationKind): StationPlan {
    return {
        kind,
        enabled: false,
        itemName: "",
        durationMinutes: 240,
        startedAtMs: null,
        finishesAtMs: null,
        status: "idle",
    };
}

function createAccount(order: number): AccountPlan {
    return {
        id: crypto.randomUUID(),
        qqAccount: "",
        enabled: true,
        initialized: false,
        order,
         status: "ready",
         independentSettingsEnabled: false,
         independentBusinessConfig: null,
         stations: stationKinds.map(createStation),
        ammoTargets: [],
        lastFailure: null,
        loginTrialSignature: null,
    };
}

function createAmmoTarget(order: number): AmmoBusinessTarget {
    return {
        id: crypto.randomUUID(),
        note: "",
        enabled: true,
        seasonal: false,
        clickPoint: null,
        scrollDirection: "down",
        scrollSteps: 0,
        order,
        profitRuleId: null,
    };
}

type StationCorrectionDraft = {
    state: ManualStationState | null;
    hours: string;
    minutes: string;
};

/// 剩余时间预填异常前的存量计时，避免人工判定选「正在制作」时丢掉制作进度。
function createCorrectionDraft(
    stations: StationPlan[] = [],
    nowMs = Date.now(),
): Record<StationKind, StationCorrectionDraft> {
    return Object.fromEntries(stationKinds.map((kind) => {
        const station = stations.find((candidate) => candidate.kind === kind);
        return [kind, {
            state: null,
            ...createStationRemainingTimeDraft(station ?? {finishesAtMs: null}, nowMs),
        }];
    })) as Record<StationKind, StationCorrectionDraft>;
}

function minutesToTime(minutes: number): string {
    const h = Math.floor(minutes / 60) % 24;
    const m = minutes % 60;
    return `${String(h).padStart(2, "0")}:${String(m).padStart(2, "0")}`;
}

function timeToMinutes(value: string): number {
    const [hStr, mStr] = value.split(":");
    return (parseInt(hStr ?? "0", 10) || 0) * 60 + (parseInt(mStr ?? "0", 10) || 0);
}

function buildCorrectionPayload(
    draft: Record<StationKind, StationCorrectionDraft>,
): StationCorrectionInput[] | null {
    const payload: StationCorrectionInput[] = [];
    for (const kind of stationKinds) {
        const item = draft[kind];
        if (!item.state) continue;
        const correction = buildInlineStationCorrection(item.state, item.hours, item.minutes);
        if (!correction) return null;
        payload.push({kind, ...correction});
    }
    return payload;
}

function enabledAmmoTargets(settings: SpecialOpsSettings, account: AccountPlan): AmmoBusinessTarget[] {
    const business = account.independentSettingsEnabled
        ? account.independentBusinessConfig
        : settings.defaultBusinessConfig;
    return (business?.ammoTargets ?? []).filter((target) => target.enabled);
}

function buildAmmoCorrectionPayload(
    targets: AmmoBusinessTarget[],
    draft: Record<string, boolean | null>,
): AmmoCorrectionInput[] {
    const payload: AmmoCorrectionInput[] = [];
    for (const target of targets) {
        const succeededToday = draft[target.id];
        if (succeededToday === null || succeededToday === undefined) continue;
        payload.push({targetId: target.id, succeededToday});
    }
    return payload;
}

function DraftInput({value, onCommit, ...props}: Omit<ComponentProps<typeof Input>, "value" | "onChange"> & {
    value: string;
    onCommit: (value: string) => void;
}) {
    const [draft, setDraft] = useState(value);
    useEffect(() => setDraft(value), [value]);
    return <Input {...props} value={draft} onChange={(event) => setDraft(event.target.value)} onBlur={() => draft !== value && onCommit(draft)}/>;
}

function AmmoTargetEditor({
    title,
    targets,
    pendingCount,
    onChange,
    onSelectPoint,
}: {
    title: string;
    targets: AmmoBusinessTarget[];
    pendingCount?: number;
    onChange: (targets: AmmoBusinessTarget[]) => void;
    onSelectPoint: (target: AmmoBusinessTarget) => void;
}) {
    const update = (target: AmmoBusinessTarget, patch: Partial<AmmoBusinessTarget>) => {
        onChange(targets.map((item) => item.id === target.id ? {...item, ...patch} : item));
    };
    const remove = (target: AmmoBusinessTarget) => {
        onChange(targets.filter((item) => item.id !== target.id).map((item, order) => ({...item, order})));
    };
    return <div className="mt-3 border-t border-base-300 pt-3">
        <div className="flex flex-wrap items-center justify-between gap-2">
            <div className="flex items-center gap-2"><RiShieldCheckLine/><h4 className="text-sm font-medium">{title}</h4>{pendingCount !== undefined && <span className="text-xs text-base-content/60">{pendingCount} 个待处理</span>}</div>
            <Button size="sm" variant="outline" onClick={() => onChange(insertNormalAmmoTarget(targets, createAmmoTarget(targets.length)))}><RiAddLine data-icon="inline-start"/>添加子弹</Button>
        </div>
        {targets.length === 0 ? <p className="mt-2 text-xs text-base-content/60">未配置兑换目标</p> : (
            <ul className="list mt-2">
                {targets.map((target, targetIndex) => <li key={target.id} className="list-row items-center gap-2 border-t border-base-300 px-0">
                    <span className="font-mono text-xs text-base-content/50">{String(targetIndex + 1).padStart(2, "0")}</span>
                    <div className="list-col-grow grid min-w-0 gap-2 sm:grid-cols-2 xl:grid-cols-[minmax(10rem,1fr)_9rem_10rem_auto_auto] xl:items-end">
                        <label className="form-control gap-1"><span className="label-text text-xs">子弹备注</span><DraftInput value={target.note} placeholder="例如：5.45×39mm BT" onCommit={(note) => update(target, {note})}/></label>
                        <div className="form-control gap-1"><span className="label-text text-xs">指定点击点</span><Button size="sm" variant="outline" onClick={() => onSelectPoint(target)}><RiCrosshair2Line data-icon="inline-start"/>{target.clickPoint ? `${target.clickPoint.x}, ${target.clickPoint.y}` : "选择"}</Button></div>
                        <label className="form-control gap-1"><span className="label-text text-xs">A/D 重置后向下滚动次数（0 表示不滚动）</span><DraftInput type="number" min={0} step={1} value={String(target.scrollSteps)} onCommit={(value) => update(target, {scrollSteps: Math.max(0, Math.trunc(Number(value) || 0))})}/></label>
                        <label className="flex h-9 items-center gap-2 text-xs"><Switch checked={target.seasonal} onCheckedChange={(seasonal) => onChange(changeAmmoTargetSeasonal(targets, target.id, seasonal))}/>赛季限定</label>
                        <label className="flex h-9 items-center gap-2 text-xs"><Switch checked={target.enabled} onCheckedChange={(enabled) => update(target, {enabled})}/>启用</label>
                    </div>
                    <div className="join">
                        <Button className="join-item" disabled={targetIndex === 0 || targets[targetIndex - 1].seasonal !== target.seasonal} size="icon-sm" title="上移" variant="ghost" onClick={() => onChange(moveAmmoTargetWithinGroup(targets, target.id, -1))}><RiArrowUpLine data-icon="inline-start"/></Button>
                        <Button className="join-item" disabled={targetIndex === targets.length - 1 || targets[targetIndex + 1].seasonal !== target.seasonal} size="icon-sm" title="下移" variant="ghost" onClick={() => onChange(moveAmmoTargetWithinGroup(targets, target.id, 1))}><RiArrowDownLine data-icon="inline-start"/></Button>
                        <Button className="join-item" size="icon-sm" title="删除子弹" variant="ghost" onClick={() => remove(target)}><RiDeleteBinLine data-icon="inline-start"/></Button>
                    </div>
                </li>)}
            </ul>
        )}
    </div>;
}

const accountStatusLabels: Record<TimelineTask["accountStatus"], string> = {
    ready: "就绪",
    needsManualLogin: "需人工登录",
    loginFailed: "登录失败",
    manualCheckRequired: "需人工验证",
    uncertain: "状态不确定",
    isolated: "已隔离",
};

const timelineProfitLabels: Record<NonNullable<TimelineTask["profitState"]>, string> = {
    waitingExchange: "等待每日兑换时间",
    waitingQuery: "等待利润查询",
    unconfigured: "利润规则未配置",
    qualified: "已达利润条件，等待轮次",
    activeRound: "当前轮次",
    cutoffBypass: "已到截止时间，忽略利润",
};

/// 人工校正面板里的限时商品结果文案。
/// 任务栏没有这组文案：当前周期一旦检查完（任何终态）就出栏，结果与人工入口都只在这里。
/// 重新检查按钮就在旁边，不需要再指路。
const correctionLimitedOutcomeLabels: Record<LimitedSupplyOutcome, string> = {
    pending: "尚未检查，等待自动执行",
    noHighValue: "本周期已检查：未发现高价值",
    highValue: "已发现高价值",
    failed: "本周期检查失败",
};

/// 交易行任务在任务栏的状态文案。
/// 购买进度必须显示：改了购买次数后任务栏要能立刻看出目标次数变了，否则用户看到的就是
/// 「改了次数任务栏毫无变化」。
const marketStatusLabels: Record<NonNullable<TimelineTask["marketStatus"]>, string> = {
    pending: "尚未开始",
    running: "进行中",
    completed: "已完成本次配置",
    priceRecognitionFailed: "价格识别失败",
    windowClosed: "窗口已关闭",
};

const shanghaiTimeFormatter = new Intl.DateTimeFormat("zh-CN", {
    timeZone: "Asia/Shanghai",
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
    hour12: false,
});

type InlineCorrectionState = {
    selected: ManualStationState | null;
    hours: string;
    minutes: string;
    submitting: boolean;
    error: string | null;
};

function TimelineManualCorrection({
    task,
    station,
    nowMs,
    disabled,
    onConfirmStation,
    onConfirmAmmo,
}: {
    task: TimelineTask;
    station: StationPlan | null;
    nowMs: number;
    disabled: boolean;
    onConfirmStation: (task: TimelineTask, correction: StationCorrectionInput) => Promise<SpecialOpsBootstrap>;
    onConfirmAmmo: (task: TimelineTask, succeededToday: boolean) => Promise<SpecialOpsBootstrap>;
}) {
    const [state, setState] = useState<InlineCorrectionState>(() => {
        const draft = createInlineStationCorrectionDraft(station ?? {finishesAtMs: null}, nowMs);
        return {selected: null, hours: draft.hours, minutes: draft.minutes, submitting: false, error: null};
    });
    if (!timelineTaskAllowsInlineCorrection(task, station)) return null;

    const submitStation = async (selected: ManualStationState) => {
        if (!task.stationKind) {
            setState((current) => ({...current, error: "制作任务缺少制作台类型"}));
            return;
        }
        const correction = buildInlineStationCorrection(selected, state.hours, state.minutes);
        if (!correction) {
            setState((current) => ({...current, selected, error: "剩余时间须留空继承或填 1 分钟至 168 小时的整数"}));
            return;
        }
        setState((current) => ({...current, selected, submitting: true, error: null}));
        try {
            await onConfirmStation(task, {kind: task.stationKind, ...correction});
        } catch (cause) {
            setState((current) => ({...current, error: `人工判定提交失败：${String(cause)}`}));
        } finally {
            setState((current) => ({...current, submitting: false}));
        }
    };
    const submitAmmo = async (succeededToday: boolean) => {
        setState((current) => ({...current, submitting: true, error: null}));
        try {
            await onConfirmAmmo(task, succeededToday);
        } catch (cause) {
            setState((current) => ({...current, error: `人工判定提交失败：${String(cause)}`}));
        } finally {
            setState((current) => ({...current, submitting: false}));
        }
    };
    const locked = disabled || state.submitting;

    return <div className="mt-2 space-y-2">
        {task.kind === "craft" ? <>
            <div className="join flex flex-wrap">
                <Button className="join-item" disabled={locked} size="xs" variant="outline" onClick={() => void submitStation("immediateDue")}>立即到期</Button>
                <Button className="join-item" disabled={locked} size="xs" variant="outline" onClick={() => setState((current) => ({...current, selected: "crafting", error: null}))}>正在制作</Button>
                <Button className="join-item" disabled={locked} size="xs" variant="outline" onClick={() => void submitStation("idle")}>空闲中</Button>
            </div>
            {state.selected === "crafting" && <div className="flex flex-wrap items-end gap-2">
                <label className="fieldset w-24"><span className="fieldset-legend">剩余小时</span><Input type="number" min={0} max={168} step={1} value={state.hours} disabled={locked} onChange={(event) => setState((current) => ({...current, hours: event.target.value, error: null}))}/></label>
                <label className="fieldset w-24"><span className="fieldset-legend">剩余分钟</span><Input type="number" min={0} max={59} step={1} value={state.minutes} disabled={locked} onChange={(event) => setState((current) => ({...current, minutes: event.target.value, error: null}))}/></label>
                <Button disabled={locked} size="xs" onClick={() => void submitStation("crafting")}>{state.submitting ? "正在保存" : "保存"}</Button>
                <span className="text-xs text-base-content/60">留空继承异常前剩余时间</span>
            </div>}
        </> : <div className="join">
            <Button className="join-item" disabled={locked} size="xs" variant="outline" onClick={() => void submitAmmo(true)}>已兑换</Button>
            <Button className="join-item" disabled={locked} size="xs" variant="outline" onClick={() => void submitAmmo(false)}>未兑换</Button>
        </div>}
        {state.error && <div role="alert" className="alert alert-error alert-soft py-2 text-xs"><span>{state.error}</span></div>}
    </div>;
}

/// 任务栏内限时商品高价值确认行：仅在 `limitedOutcome === "highValue"` 时渲染，
/// 提供与账号人工校正面板相同的「已查看高价值商品」按钮，避免用户必须进账号页才能确认。
function TimelineLimitedAcknowledge({
    task,
    disabled,
    onAcknowledge,
}: {
    task: TimelineTask;
    disabled: boolean;
    onAcknowledge: (accountId: string, cycleId: string) => Promise<SpecialOpsBootstrap>;
}) {
    const [submitting, setSubmitting] = useState(false);
    const [error, setError] = useState<string | null>(null);
    const cycleId = task.limitedCycleId ?? null;
    const submit = async () => {
        if (!cycleId) return;
        setSubmitting(true);
        setError(null);
        try {
            await onAcknowledge(task.accountId, cycleId);
        } catch (cause) {
            setError(String(cause));
        } finally {
            setSubmitting(false);
        }
    };
    return <div className="mt-1">
        <Button size="xs" variant="outline" disabled={disabled || submitting || !cycleId} onClick={() => void submit()}>
            {submitting ? "正在确认" : "已查看高价值商品"}
        </Button>
        {error && <div role="alert" className="alert alert-error alert-soft mt-1 py-1 text-xs"><span>{error}</span></div>}
    </div>;
}

/// 人工校正面板里的限时商品分区：展示本周期判定结果 + 确认高价值 + 重新检查入口。
/// 任务栏内也有「已查看高价值商品」按钮（`TimelineLimitedAcknowledge`）——两者功能相同，
/// 不同入口满足不同使用场景，共用 `acknowledgeLimitedSupply` 后端命令。
/// 与制作台/子弹校正各自独立提交——两个动作都只碰限时商品状态，不参与那份原子覆盖，
/// 所以放在核对流程之外，点了立刻生效。
function CorrectionLimitedSupply({
    account,
    disabled,
    onRecheck,
    onAcknowledge,
}: {
    account: AccountPlan;
    disabled: boolean;
    onRecheck: (accountId: string, cycleId: string) => Promise<SpecialOpsBootstrap>;
    onAcknowledge: (accountId: string, cycleId: string) => Promise<SpecialOpsBootstrap>;
}) {
    const [submitting, setSubmitting] = useState<"recheck" | "acknowledge" | null>(null);
    const [error, setError] = useState<string | null>(null);
    const limited = account.limitedSupply;
    const cycleId = limited?.cycleId ?? null;
    const checked = limited !== undefined && limited.outcome !== "pending";
    // 高价值提醒只在这里确认：任务栏检查完就出栏，没有别的入口。
    const needsAcknowledge = limited?.outcome === "highValue" && !limited.acknowledged;
    const submit = async (kind: "recheck" | "acknowledge") => {
        if (!cycleId) return;
        setSubmitting(kind);
        setError(null);
        try {
            await (kind === "recheck" ? onRecheck(account.id, cycleId) : onAcknowledge(account.id, cycleId));
        } catch (cause) {
            setError(String(cause));
        } finally {
            setSubmitting(null);
        }
    };
    return <div className="mt-4 rounded-box border border-base-300 p-3">
        <div className="flex flex-wrap items-center justify-between gap-2">
            <h4 className="font-medium">本周期限时商品</h4>
            <div className="flex flex-wrap items-center gap-2">
                {needsAcknowledge && <Button
                    size="sm"
                    variant="outline"
                    disabled={disabled || submitting !== null || !cycleId}
                    onClick={() => void submit("acknowledge")}
                >{submitting === "acknowledge" ? "正在确认" : "已查看高价值商品"}</Button>}
                <Button
                    size="sm"
                    variant="outline"
                    disabled={disabled || submitting !== null || !checked || !cycleId}
                    title={checked && cycleId ? "把本周期判定复位到未检查，任务重新回到任务栏并立刻可执行" : "本周期尚未检查，无需重新检查"}
                    onClick={() => void submit("recheck")}
                ><RiRefreshLine data-icon="inline-start"/>{submitting === "recheck" ? "正在复位" : "重新检查"}</Button>
            </div>
        </div>
        <p className="mt-2 text-sm text-base-content/70">
            {limited === undefined
                ? "当前账号没有限时商品记录"
                : correctionLimitedOutcomeLabels[limited.outcome]}
            {limited?.checkedAtMs ? ` · 检查于 ${shanghaiTimeFormatter.format(limited.checkedAtMs)}` : ""}
        </p>
        {limited?.lastError && <p className="mt-1 text-xs text-error">{limited.lastError}</p>}
        {error && <div role="alert" className="alert alert-error alert-soft mt-2 py-2 text-xs"><span>{error}</span></div>}
    </div>;
}

function SpecialOpsTimeline({
    bootstrap,
    nowMs,
    disabled,
    onConfirmStation,
    onConfirmAmmo,
    onAcknowledge,
}: {
    bootstrap: SpecialOpsBootstrap;
    nowMs: number;
    disabled: boolean;
    onConfirmStation: (task: TimelineTask, correction: StationCorrectionInput) => Promise<SpecialOpsBootstrap>;
    onConfirmAmmo: (task: TimelineTask, succeededToday: boolean) => Promise<SpecialOpsBootstrap>;
    onAcknowledge: (accountId: string, cycleId: string) => Promise<SpecialOpsBootstrap>;
}) {
    const slots = buildTimelineHourSlots(nowMs);
    const groups = groupTimelineTasks(bootstrap.schedule.timelineTasks);
    const accountNumbers = new Map(bootstrap.settings.accounts.map((account, index) => [account.id, String(index + 1).padStart(2, "0")]));
    const firstSlot = slots[0];
    const hourMs = 60 * 60_000;
    const groupsBySlot = slots.map(() => [] as typeof groups);
    for (const group of groups) {
        const index = group.anchorAtMs <= nowMs
            ? 0
            : Math.min(23, Math.max(0, Math.floor((group.anchorAtMs - firstSlot) / hourMs)));
        groupsBySlot[index].push(group);
    }
    return <section className="card card-border bg-base-100">
        <div className="card-body gap-3">
            <div><h2 className="card-title text-lg">未来 24 小时任务</h2><p className="text-xs text-base-content/60">10 分钟内任务合并显示，计划时间不变；暂停期间到期任务显示 0 分钟后。</p></div>
            {groups.length === 0 ? <div className="rounded-box border border-dashed border-base-300 p-6 text-center text-sm text-base-content/60">未来 24 小时暂无任务</div> : <div className="max-h-[38rem] overflow-y-auto rounded-box border border-base-300">
                {slots.map((slot, index) => <div key={slot} className="grid grid-cols-[5rem_minmax(0,1fr)] border-b border-base-300 last:border-b-0">
                    <time className="border-r border-base-300 bg-base-200 px-3 py-3 text-xs font-medium">{shanghaiTimeFormatter.format(slot).replace(" ", "\n")}</time>
                    <div className="min-h-16 space-y-2 p-2">
                        {groupsBySlot[index].map((group) => <div key={`${group.anchorAtMs}-${group.tasks[0].id}`} className="card card-xs bg-base-200">
                            <div className="card-body py-2">
                                <ul className="list">
                                    {group.tasks.map((task) => {
                                        const needsManualCorrection = task.accountStatus === "uncertain" || task.accountStatus === "isolated" || task.accountStatus === "manualCheckRequired";
                                        const station = task.stationKind
                                            ? bootstrap.settings.accounts.find(({id}) => id === task.accountId)?.stations.find(({kind}) => kind === task.stationKind) ?? null
                                            : null;
                                        const inlineCorrectable = timelineTaskAllowsInlineCorrection(task, station);
                                        return <li key={task.id} className="list-row px-0 py-1">
                                            <div className="list-col-grow min-w-0">
                                                <div className="truncate text-sm font-medium">账号 {accountNumbers.get(task.accountId) ?? "--"} {task.qqAccount || task.accountId} · {task.kind === "craft" && task.stationKind ? STATION_LABELS[task.stationKind] : task.kind === "limitedSupplyCheck" ? "限时商品检查" : task.kind === "marketPurchase" ? "交易行购买" : "子弹兑换"}{task.note ? ` · ${task.note}` : ""}</div>
                                                <div className="text-xs text-base-content/60">计划 {shanghaiTimeFormatter.format(task.scheduledAtMs)} · {timelineDelayMinutes(task, nowMs)} 分钟后</div>
                                                {task.profitState && <div className="text-xs text-base-content/60">{timelineProfitLabels[task.profitState]}{task.mayExecuteEarlier ? "；最晚执行，利润达标后可能提前" : ""}</div>}
                                                {task.manualFailure && <div className="text-xs text-error">{task.manualFailure.step}：{task.manualFailure.message}</div>}
                                                {task.kind === "marketPurchase" && task.marketStatus
                                                    && <div className={`text-xs ${task.marketStatus === "priceRecognitionFailed" ? "text-error" : "text-base-content/60"}`}>已购买 {task.marketCompletedCount ?? 0}/{task.marketTargetCount ?? 0} · {marketStatusLabels[task.marketStatus]}</div>}
                                                <TimelineManualCorrection task={task} station={station} nowMs={nowMs} disabled={disabled} onConfirmStation={onConfirmStation} onConfirmAmmo={onConfirmAmmo}/>
                                                {task.kind === "limitedSupplyCheck" && task.limitedOutcome === "highValue" && task.limitedCycleId && <TimelineLimitedAcknowledge task={task} disabled={disabled} onAcknowledge={onAcknowledge}/>}
                                                {needsManualCorrection && !inlineCorrectable && <div className="text-xs text-error">请在账号页处理</div>}
                                            </div>
                                            <div className="flex shrink-0 flex-col items-end gap-1">
                                                <span className={`badge badge-sm ${task.accountStatus === "ready" ? "badge-success badge-soft" : needsManualCorrection ? "badge-error badge-soft" : "badge-warning badge-soft"}`}>{accountStatusLabels[task.accountStatus]}</span>
                                            </div>
                                        </li>;
                                    })}
                                </ul>
                            </div>
                        </div>)}
                    </div>
                </div>)}
            </div>}
        </div>
    </section>;
}

type AutomationDelayField =
    | "navigationBeaconDelayMs"
    | "navigationSpaceDelayMs"
    | "navigationTabDelayMs"
    | "navigationSpecialOpsDelayMs"
    | "ammoSupplyDelayMs"
    | "ammoTacticalDelayMs"
    | "ammoSeasonalEntryDelayMs"
    | "craftSpaceDelayMs"
    | "craftReopenDelayMs"
    | "craftConfirmPinnedDelayMs";

export function SpecialOpsPage() {
    const isNativeShell = useNativeShell();
    const [bootstrap, setBootstrap] = useState(emptyBootstrap);
    const [timelineNowMs, setTimelineNowMs] = useState(Date.now());
    const [error, setError] = useState<string | null>(null);
    const [testingTargetKey, setTestingTargetKey] = useState<string | null>(null);
    const [calibrationTestResult, setCalibrationTestResult] = useState<string | null>(null);
    const [limitedColorFeedback, setLimitedColorFeedback] = useState<string | null>(null);
    const [selectedAccountId, setSelectedAccountId] = useState<string | null>(null);
    const [selectedCraftStation, setSelectedCraftStation] = useState<StationKind>("technicalCenter");
    const [hotkeyStatus, setHotkeyStatus] = useState<string | null>(null);
    const [correctionAccountId, setCorrectionAccountId] = useState<string | null>(null);
    const [correctionDraft, setCorrectionDraft] = useState(createCorrectionDraft);
    const [correctionAmmoDraft, setCorrectionAmmoDraft] = useState<Record<string, boolean | null>>({});
    const [correctionConfirming, setCorrectionConfirming] = useState(false);
    const [correctionSubmitting, setCorrectionSubmitting] = useState(false);
    const [correctionError, setCorrectionError] = useState<string | null>(null);
    // 账号级动作（已人工检查 / 一键恢复）的失败原因。顶部横幅在账号列表里看不见——
    // 用户点了按钮、报错滚出视口，看起来就是「点了没反应」。错误必须落在按钮旁边。
    const [accountActionError, setAccountActionError] = useState<{accountId: string; message: string} | null>(null);
    const [pauseTransition, setPauseTransition] = useState(false);
    const bootstrapRef = useRef(emptyBootstrap);
    const appliedResponseSequenceRef = useRef(0);
    const requestSequenceRef = useRef(0);
    const disposedRef = useRef(false);
    const settingsDraftRef = useRef(emptyBootstrap.settings);
    const settingsDirtyRef = useRef(false);
    const pendingSaveRef = useRef<SpecialOpsSaveRequest | null>(null);
    const pendingSaveTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
    const lastSavePromiseRef = useRef<Promise<void> | null>(null);
    const saveQueueRef = useRef<LatestSaveQueue<SpecialOpsSaveRequest> | null>(null);
    saveQueueRef.current ??= new LatestSaveQueue();

    const applyUpdate = (update: SpecialOpsBootstrapUpdate, completedSettings?: SpecialOpsSettings) => {
        const previous = bootstrapRef.current;
        const dirtyBefore = settingsDirtyRef.current;
        const ordered = applySpecialOpsBootstrapUpdate({
            bootstrap: previous,
            responseSeq: appliedResponseSequenceRef.current,
        }, update);
        const next = ordered.bootstrap;
        const responseAccepted = update.type === "bootstrapResponse" && (
            update.bootstrap.settingsRevision > previous.settingsRevision
            || (update.bootstrap.settingsRevision === previous.settingsRevision
                && update.requestSeq >= appliedResponseSequenceRef.current)
        );
        const revisionChanged = next.settingsRevision !== previous.settingsRevision;
        const completedCurrentDraft = update.type === "bootstrapResponse" && responseAccepted && (
            completedSettings === settingsDraftRef.current
            || JSON.stringify(update.bootstrap.settings) === JSON.stringify(settingsDraftRef.current)
        );
        bootstrapRef.current = next;
        appliedResponseSequenceRef.current = ordered.responseSeq;
        if (completedCurrentDraft) {
            settingsDirtyRef.current = false;
            pendingSaveRef.current = null;
            settingsDraftRef.current = next.settings;
            setBootstrap(next);
        } else if (settingsDirtyRef.current && !revisionChanged) {
            setBootstrap({...next, settings: settingsDraftRef.current});
        } else {
            if (revisionChanged && pendingSaveTimerRef.current !== null) {
                clearTimeout(pendingSaveTimerRef.current);
                pendingSaveTimerRef.current = null;
            }
            settingsDirtyRef.current = false;
            pendingSaveRef.current = null;
            settingsDraftRef.current = next.settings;
            setBootstrap(next);
        }
        setError((current) => specialOpsErrorAfterUpdate(current, {
            updateType: update.type,
            responseAccepted,
            completedCurrentDraft,
            dirtyBefore,
            revisionChanged,
        }));
    };
    const applyResult = (
        incoming: SpecialOpsBootstrap,
        requestSeq: number,
        completedSettings?: SpecialOpsSettings,
    ) => {
        applyUpdate({type: "bootstrapResponse", bootstrap: incoming, requestSeq}, completedSettings);
    };
    const applyRunSnapshot = (snapshot: LoginRunSnapshot) => {
        applyUpdate({type: "runChanged", snapshot});
    };
    const requestBootstrap = async (request: () => Promise<SpecialOpsBootstrap>) => {
        const requestSeq = ++requestSequenceRef.current;
        try {
            const next = await request();
            if (!disposedRef.current) applyResult(next, requestSeq);
            return next;
        } catch (cause) {
            if (!disposedRef.current && requestSeq === requestSequenceRef.current) setError(String(cause));
            throw cause;
        }
    };
    const runBootstrapRequest = (request: () => Promise<SpecialOpsBootstrap>) => {
        void requestBootstrap(request).catch(() => undefined);
    };
    const reload = () => {
        if (!isNativeShell) return;
        runBootstrapRequest(() => invoke<SpecialOpsBootstrap>("special_ops_get_bootstrap"));
    };
    useEffect(() => {
        setTimelineNowMs(Date.now());
        const interval = window.setInterval(() => {
            setTimelineNowMs(Date.now());
            if (isNativeShell) reload();
        }, 60_000);
        return () => window.clearInterval(interval);
    }, [isNativeShell]);
    useEffect(() => {
        disposedRef.current = false;
        reload();
        if (!isNativeShell) return;
        const unsubscribeState = subscribeTauriEvent<SpecialOpsStateChanged>(SPECIAL_OPS_EVENTS.stateChanged, (event) => {
            reloadSpecialOpsAfterStateChanged(event.payload, reload);
        });
        const unsubscribeRun = subscribeTauriEvent<LoginRunSnapshot>(SPECIAL_OPS_EVENTS.runChanged, (event) => {
            applyRunSnapshot(event.payload);
        });
        return () => {
            disposedRef.current = true;
            if (pendingSaveTimerRef.current !== null) clearTimeout(pendingSaveTimerRef.current);
            requestSequenceRef.current += 1;
            unsubscribeState();
            unsubscribeRun();
        };
    }, [isNativeShell]);

    const performSave = async ({settings, settingsRevision}: SpecialOpsSaveRequest) => {
        const requestSeq = ++requestSequenceRef.current;
        const next = await persistSpecialOpsSaveRequest(
            {settings, settingsRevision},
            (request) => invoke<SpecialOpsBootstrap>("special_ops_save_settings", {
                settingsValue: request.settings,
                settingsRevision: request.settingsRevision,
            }),
            reload,
        );
        if (!disposedRef.current) applyResult(next, requestSeq, settings);
    };
    const enqueueSave = (request: SpecialOpsSaveRequest) => {
        const task = saveQueueRef.current!.enqueue(request, performSave);
        lastSavePromiseRef.current = task;
        void task.then(
            () => {
                if (lastSavePromiseRef.current === task) lastSavePromiseRef.current = null;
            },
            () => {
                if (lastSavePromiseRef.current === task) lastSavePromiseRef.current = null;
            },
        );
        return task;
    };
    const save = (settings: SpecialOpsSettings) => {
        if (hasActiveSpecialOpsRun(bootstrapRef.current.runSnapshot)) return;
        settingsDraftRef.current = settings;
        settingsDirtyRef.current = true;
        pendingSaveRef.current = {settings, settingsRevision: bootstrapRef.current.settingsRevision};
        setBootstrap((current) => ({...current, settings}));
        if (pendingSaveTimerRef.current !== null) clearTimeout(pendingSaveTimerRef.current);
        pendingSaveTimerRef.current = setTimeout(() => {
            pendingSaveTimerRef.current = null;
            const request = pendingSaveRef.current;
            pendingSaveRef.current = null;
            if (request) void enqueueSave(request).catch((cause) => setError(String(cause)));
        }, SAVE_DELAY_MS);
    };
    const flushSettings = async () => {
        if (!settingsDirtyRef.current) return bootstrapRef.current;
        if (pendingSaveTimerRef.current !== null) {
            clearTimeout(pendingSaveTimerRef.current);
            pendingSaveTimerRef.current = null;
            const request = pendingSaveRef.current;
            pendingSaveRef.current = null;
            if (request) await enqueueSave(request);
        } else if (lastSavePromiseRef.current) {
            await lastSavePromiseRef.current;
        } else {
            await enqueueSave({
                settings: settingsDraftRef.current,
                settingsRevision: bootstrapRef.current.settingsRevision,
            });
        }
        return bootstrapRef.current;
    };
    const saveProfitSettings = async (update: ProfitConfigurationUpdate) => {
        const saved = await flushSettings();
        return requestBootstrap(() => invoke<SpecialOpsBootstrap>("special_ops_save_profit_settings", {
            update,
            settingsRevision: saved.settingsRevision,
        }));
    };
    const setPaused = (paused: boolean) => {
        if (pauseTransition) return;
        setPauseTransition(true);
        void (async () => {
            try {
                const saved = await flushSettings();
                await requestBootstrap(() => invoke<SpecialOpsBootstrap>("special_ops_set_paused", {
                    paused,
                    settingsRevision: saved.settingsRevision,
                }));
            } catch (cause) {
                if (!disposedRef.current) setError(String(cause));
            } finally {
                if (!disposedRef.current) setPauseTransition(false);
            }
        })();
    };
    const updateAccount = (account: AccountPlan, patch: Partial<AccountPlan>) => save({
        ...settingsDraftRef.current,
        accounts: settingsDraftRef.current.accounts.map((item) => item.id === account.id ? {...item, ...patch} : item),
    });
    const updateDefaultStation = (station: StationBusinessConfig, patch: Partial<StationBusinessConfig>) => save({
        ...settingsDraftRef.current,
        defaultBusinessConfig: {
            ...settingsDraftRef.current.defaultBusinessConfig,
            stations: settingsDraftRef.current.defaultBusinessConfig.stations.map((item) => item.kind === station.kind ? {...item, ...patch} : item),
        },
    });
    const setIndependentSettings = (account: AccountPlan, enabled: boolean) => {
        if (!enabled && !window.confirm(`关闭账号 ${account.qqAccount || account.id} 的独立设置？独立业务配置将永久删除。`)) return;
        updateAccount(account, {
            independentSettingsEnabled: enabled,
            independentBusinessConfig: enabled
                ? structuredClone(settingsDraftRef.current.defaultBusinessConfig)
                : null,
        });
    };
    const updateIndependentBusiness = (account: AccountPlan, patch: Partial<NonNullable<AccountPlan["independentBusinessConfig"]>>) => {
        if (!account.independentBusinessConfig) return;
        updateAccount(account, {
            independentBusinessConfig: {...account.independentBusinessConfig, ...patch},
        });
    };
    const updateIndependentStation = (account: AccountPlan, station: StationBusinessConfig, patch: Partial<StationBusinessConfig>) => {
        if (!account.independentBusinessConfig) return;
        updateIndependentBusiness(account, {
            stations: account.independentBusinessConfig.stations.map((item) => item.kind === station.kind ? {...item, ...patch} : item),
        });
    };
    const addAccount = () => save({
        ...settingsDraftRef.current,
        accounts: [...settingsDraftRef.current.accounts, createAccount(settingsDraftRef.current.accounts.length)],
    });
    const removeAccount = (account: AccountPlan) => {
        if (!window.confirm(`删除账号 ${account.qqAccount || "未命名账号"}？`)) return;
        save({...settingsDraftRef.current, accounts: settingsDraftRef.current.accounts.filter((item) => item.id !== account.id)});
    };
    const eligibleAccounts = eligibleLoginTrialAccounts(bootstrap.settings.accounts);
    useEffect(() => {
        setSelectedAccountId((current) => eligibleAccounts.some(({id}) => id === current)
            ? current
            : eligibleAccounts[0]?.id ?? null);
    }, [bootstrap.settings.accounts]);
    const runSnapshot = bootstrap.runSnapshot;
    const hasActiveRun = hasActiveSpecialOpsRun(runSnapshot);
    const controlsLocked = hasActiveRun || pauseTransition;
    const isActiveRound = runSnapshot?.runKind === "round";
    // 一键恢复要清当天 lastSuccessDay，按 Asia/Shanghai 取当天，和后端 local_day_and_minute 对齐。
    const currentDay = shanghaiDay(bootstrap.nowMs);
    const anyAccountRestorable = bootstrap.settings.accounts.some((account) => accountRestorable(account, currentDay));
    const selectedAccount = bootstrap.settings.accounts.find(({id}) => id === selectedAccountId) ?? null;
    const correctionAccount = bootstrap.settings.accounts.find(({id}) => id === correctionAccountId) ?? null;
    const correctionPayload = buildCorrectionPayload(correctionDraft);
    const correctionAmmoTargets = correctionAccount ? enabledAmmoTargets(bootstrap.settings, correctionAccount) : [];
    const correctionAmmoPayload = buildAmmoCorrectionPayload(correctionAmmoTargets, correctionAmmoDraft);
    const openCorrection = (accountId: string) => {
        if (controlsLocked) return;
        const account = bootstrap.settings.accounts.find(({id}) => id === accountId);
        if (!account) return;
        setCorrectionAccountId(account.id);
        setCorrectionDraft(createCorrectionDraft(account.stations, bootstrap.nowMs));
        setCorrectionAmmoDraft(Object.fromEntries(enabledAmmoTargets(bootstrap.settings, account).map((target) => [target.id, null])));
        setCorrectionConfirming(false);
        setCorrectionSubmitting(false);
        setCorrectionError(null);
    };
    const closeCorrection = () => {
        setCorrectionAccountId(null);
        setCorrectionConfirming(false);
        setCorrectionSubmitting(false);
        setCorrectionError(null);
    };
    const updateCorrection = (kind: StationKind, patch: Partial<StationCorrectionDraft>) => {
        setCorrectionDraft((current) => ({
            ...current,
            [kind]: {...current[kind], ...patch},
        }));
        setCorrectionConfirming(false);
        setCorrectionError(null);
    };
    const submitCorrection = async () => {
        if (!isNativeShell) {
            setCorrectionError("人工校正提交失败：浏览器预览不能写入配置，请使用桌面开发版");
            return;
        }
        if (!correctionAccountId || !correctionPayload || (correctionPayload.length === 0 && correctionAmmoPayload.length === 0) || controlsLocked) {
            setCorrectionError("人工校正提交失败：页面状态已变化，请返回修改后重新核对");
            return;
        }
        try {
            setError(null);
            setCorrectionError(null);
            setCorrectionSubmitting(true);
            const saved = await flushSettings();
            await requestBootstrap(() => invoke<SpecialOpsBootstrap>("special_ops_confirm_account_station_states", {
                accountId: correctionAccountId,
                stations: correctionPayload,
                ammoTargets: correctionAmmoPayload,
                settingsRevision: saved.settingsRevision,
            }));
            closeCorrection();
        } catch (cause) {
            setError(String(cause));
            setCorrectionError(`人工校正提交失败：${String(cause)}`);
        } finally {
            setCorrectionSubmitting(false);
        }
    };
    const confirmTimelineStation = async (
        task: TimelineTask,
        correction: StationCorrectionInput,
    ) => {
        if (!isNativeShell) throw new Error("浏览器预览不能写入配置，请使用桌面开发版");
        try {
            const saved = await flushSettings();
            return await requestBootstrap(() => invoke<SpecialOpsBootstrap>(
                "special_ops_confirm_station_state",
                {accountId: task.accountId, correction, settingsRevision: saved.settingsRevision},
            ));
        } catch (cause) {
            if (String(cause).includes("配置保存已陈旧")) reload();
            throw cause;
        }
    };
    const confirmTimelineAmmo = async (task: TimelineTask, succeededToday: boolean) => {
        if (!isNativeShell) throw new Error("浏览器预览不能写入配置，请使用桌面开发版");
        if (!task.ammoTargetId) throw new Error("子弹任务缺少目标 ID");
        try {
            const saved = await flushSettings();
            return await requestBootstrap(() => invoke<SpecialOpsBootstrap>(
                "special_ops_confirm_ammo_state",
                {
                    accountId: task.accountId,
                    correction: {targetId: task.ammoTargetId, succeededToday},
                    settingsRevision: saved.settingsRevision,
                },
            ));
        } catch (cause) {
            if (String(cause).includes("配置保存已陈旧")) reload();
            throw cause;
        }
    };
    const recorder = useHotkeyRecorder({
        formatKey: formatRecordedHotkey,
        onCommit: (emergencyHotkey) => {
            save({...settingsDraftRef.current, emergencyHotkey});
            setHotkeyStatus(`新的紧急停止热键：${emergencyHotkey}`);
        },
        onCancel: () => undefined,
        onStatusMessage: setHotkeyStatus,
    });
    const pickExecutable = async (field: "wegameExecutablePath" | "gameExecutablePath") => {
        if (!isNativeShell || controlsLocked) return;
        try {
            const picked = await open({
                multiple: false,
                directory: false,
                filters: [{name: "可执行文件", extensions: ["exe"]}],
            });
            const current = settingsDraftRef.current[field];
            const selected = applyExecutableSelection(current, typeof picked === "string" ? picked : null);
            if (selected !== current) save({...settingsDraftRef.current, [field]: selected});
        } catch (cause) {
            setError(String(cause));
        }
    };
    const startLoginTrial = async () => {
        if (!isNativeShell || !selectedAccountId || controlsLocked) return;
        try {
            setError(null);
            const saved = await flushSettings();
            const snapshot = await invoke<LoginRunSnapshot>("special_ops_start_login_trial", {
                accountId: selectedAccountId,
                settingsRevision: saved.settingsRevision,
            });
            applyRunSnapshot(snapshot);
        } catch (cause) {
            setError(String(cause));
        }
    };
    const startNavigationTrial = async () => {
        if (!isNativeShell || !selectedAccountId || controlsLocked) return;
        try {
            setError(null);
            const saved = await flushSettings();
            const snapshot = await invoke<LoginRunSnapshot>("special_ops_start_navigation_trial", {
                accountId: selectedAccountId,
                settingsRevision: saved.settingsRevision,
            });
            applyRunSnapshot(snapshot);
        } catch (cause) {
            setError(String(cause));
        }
    };
    const startCraftTrial = async () => {
        if (!isNativeShell || !selectedAccountId || controlsLocked) return;
        try {
            setError(null);
            const saved = await flushSettings();
            const snapshot = await invoke<LoginRunSnapshot>("special_ops_start_craft_trial", {
                accountId: selectedAccountId,
                stationKind: selectedCraftStation,
                settingsRevision: saved.settingsRevision,
            });
            applyRunSnapshot(snapshot);
        } catch (cause) {
            setError(String(cause));
        }
    };
    const startCraftBatchTrial = async () => {
        if (!isNativeShell || !selectedAccountId || controlsLocked) return;
        try {
            setError(null);
            const saved = await flushSettings();
            const snapshot = await invoke<LoginRunSnapshot>("special_ops_start_craft_batch_trial", {
                accountId: selectedAccountId,
                settingsRevision: saved.settingsRevision,
            });
            applyRunSnapshot(snapshot);
        } catch (cause) {
            setError(String(cause));
        }
    };
    const startAmmoTrial = async () => {
        if (!isNativeShell || !selectedAccountId || controlsLocked) return;
        try {
            setError(null);
            const saved = await flushSettings();
            const snapshot = await invoke<LoginRunSnapshot>("special_ops_start_ammo_trial", {
                accountId: selectedAccountId,
                settingsRevision: saved.settingsRevision,
            });
            applyRunSnapshot(snapshot);
        } catch (cause) {
            setError(String(cause));
        }
    };
    const confirmAccountManualCheck = async (accountId: string) => {
        if (!isNativeShell) return;
        try {
            setAccountActionError(null);
            const saved = await flushSettings();
            await requestBootstrap(() => invoke<SpecialOpsBootstrap>("special_ops_confirm_account_manual_check", {accountId, settingsRevision: saved.settingsRevision}));
        } catch (cause) {
            setAccountActionError({accountId, message: `人工检查提交失败：${String(cause)}`});
            setError(`人工检查提交失败：${String(cause)}`);
        }
    };
    /// 一键恢复：accountId 为 null 时恢复全部异常账号。
    const restoreAccountState = async (accountId: string | null) => {
        if (!isNativeShell) return;
        try {
            setError(null);
            setAccountActionError(null);
            const saved = await flushSettings();
            await requestBootstrap(() => invoke<SpecialOpsBootstrap>("special_ops_restore_account_state", {accountId, settingsRevision: saved.settingsRevision}));
        } catch (cause) {
            if (accountId) setAccountActionError({accountId, message: `一键恢复失败：${String(cause)}`});
            setError(`一键恢复失败：${String(cause)}`);
        }
    };
    /// 确认限时商品高价值提醒：确认后不再提示，但不影响「本周期已检查」这个事实。
    /// 入口与重新检查同处账号人工校正面板——任务栏在检查完成后就出栏，没有别处可挂。
    const acknowledgeLimitedSupply = async (accountId: string, cycleId: string) => {
        if (!isNativeShell) throw new Error("浏览器预览不能写入配置，请使用桌面开发版");
        try {
            const saved = await flushSettings();
            return await requestBootstrap(() => invoke<SpecialOpsBootstrap>(
                "special_ops_acknowledge_limited_supply",
                {accountId, cycleId, settingsRevision: saved.settingsRevision},
            ));
        } catch (cause) {
            if (String(cause).includes("配置保存已陈旧")) reload();
            throw cause;
        }
    };
    /// 重新检查本周期限时商品：后端把状态复位到 `pending`，任务回到任务栏并立刻可执行
    /// （任务栏出栏条件与 planner 的 `limited_supply_due` 同源，两个 gate 一起重新放行）。
    /// 自动调度「每周期一次」的语义不变，重跑只由这里触发。
    /// 入口在账号人工校正面板，所以取账号自己的 `limitedSupply.cycleId`，不依赖任务栏任务。
    const recheckLimitedSupply = async (accountId: string, cycleId: string) => {
        if (!isNativeShell) throw new Error("浏览器预览不能写入配置，请使用桌面开发版");
        try {
            const saved = await flushSettings();
            return await requestBootstrap(() => invoke<SpecialOpsBootstrap>(
                "special_ops_recheck_limited_supply",
                {accountId, cycleId, settingsRevision: saved.settingsRevision},
            ));
        } catch (cause) {
            if (String(cause).includes("配置保存已陈旧")) reload();
            throw cause;
        }
    };
    const startLimitedSupplyTrial = async () => {
        if (!isNativeShell || !selectedAccountId || controlsLocked) return;
        try {
            const saved = await flushSettings();
            const snapshot = await invoke<LoginRunSnapshot>("special_ops_start_limited_supply_trial", {accountId: selectedAccountId, settingsRevision: saved.settingsRevision});
            applyRunSnapshot(snapshot);
        } catch (cause) {
            setError(String(cause));
        }
    };
    const startMarketTrial = async () => {
        if (!isNativeShell || !selectedAccountId || controlsLocked) return;
        try {
            const saved = await flushSettings();
            const snapshot = await invoke<LoginRunSnapshot>("special_ops_start_market_trial", {accountId: selectedAccountId, mode: "realSingleAttempt", settingsRevision: saved.settingsRevision});
            applyRunSnapshot(snapshot);
        } catch (cause) {
            setError(String(cause));
        }
    };
    const cancelLoginTrial = async () => {
        if (!isNativeShell || !hasActiveRun || runSnapshot?.status === "stopping") return;
        try {
            const snapshot = await invoke<LoginRunSnapshot>("special_ops_cancel_login_trial");
            applyRunSnapshot(snapshot);
        } catch (cause) {
            setError(String(cause));
        }
    };
    const activeEnvironment = bootstrap.settings.calibrationEnvironments[0];
    const limitedSupply: LimitedSupplySettings = bootstrap.settings.limitedSupply ?? emptyBootstrap.settings.limitedSupply!;
    const marketPurchase: MarketPurchaseSettings = bootstrap.settings.marketPurchase ?? emptyBootstrap.settings.marketPurchase!;
    const defaultMarket: MarketBusinessConfig = settingsDraftRef.current.defaultBusinessConfig.market ?? emptyBootstrap.settings.defaultBusinessConfig.market!;
    const updateLimitedSupply = (patch: Partial<LimitedSupplySettings>) => save({
        ...settingsDraftRef.current,
        limitedSupply: {...limitedSupply, ...patch},
    });
    const updateMarketPurchase = (patch: Partial<MarketPurchaseSettings>) => save({
        ...settingsDraftRef.current,
        marketPurchase: {...marketPurchase, ...patch},
    });
    const updateLimitedColor = (colorIndex: number, color: [number, number, number]) => {
        const colors = [...limitedSupply.colors] as [[number, number, number], [number, number, number]];
        colors[colorIndex] = color;
        updateLimitedSupply({colors});
    };
    const commitLimitedColorHex = (colorIndex: number, value: string) => {
        const color = parseLimitedColorHex(value);
        if (!color) {
            setError("颜色必须使用 #RRGGBB 格式");
            return;
        }
        setError(null);
        updateLimitedColor(colorIndex, color);
    };
    const testLimitedColors = async () => {
        if (!isNativeShell || !activeEnvironment || controlsLocked) return;
        try {
            const saved = await flushSettings();
            const results: LimitedSupplyColorTestResult[] = [];
            for (let regionIndex = 1; regionIndex <= 9; regionIndex += 1) {
                results.push(await invoke<LimitedSupplyColorTestResult>("special_ops_test_limited_supply_colors", {environmentId: activeEnvironment.id, regionIndex, settingsRevision: saved.settingsRevision}));
            }
            // 全部 9 个区域必须连续命中才算高价值（与 compare_samples 语义一致）
            const passed = results.length > 0 && results.every((result) => result.passed);
            const passedRegions = results
                .map((result, i) => result.passed ? i + 1 : null)
                .filter((i): i is number => i !== null);
            const regionDetail = results
                .map((r, i) => `区${i + 1}${r.passed ? "✓" : "✗"}(距${r.firstNearestDistance.toFixed(0)}/${r.secondNearestDistance.toFixed(0)})`)
                .join(" ");
            setLimitedColorFeedback(
                `${passed ? `识色通过（全部9区命中）` : `识色未通过（${passedRegions.length}/9 区命中：${passedRegions.length ? passedRegions.join("、") : "无"}）`} — ${regionDetail}`
            );
        } catch (cause) {
            setLimitedColorFeedback(`限时商品识色测试失败：${String(cause)}`);
        }
    };
    const beginCalibration = async (
        environment: CalibrationEnvironment,
        targetKey: string,
        accountId: string | null = null,
    ) => {
        if (!isNativeShell || controlsLocked) return;
        setCalibrationTestResult(null);
        try {
            const saved = await flushSettings();
            await invoke("special_ops_begin_calibration_selection", {
                environmentId: environment.id,
                targetKey,
                accountId,
                settingsRevision: saved.settingsRevision,
            });
        } catch (cause) {
            setError(String(cause));
        }
    };
    const updateCalibrationTarget = (
        environment: CalibrationEnvironment,
        target: CalibrationTarget,
        patch: Partial<CalibrationTarget>,
    ) => {
        setCalibrationTestResult(null);
        save({
            ...settingsDraftRef.current,
            calibrationEnvironments: settingsDraftRef.current.calibrationEnvironments.map((item) => item.id === environment.id
                ? {...item, targets: item.targets.map((candidate) => candidate.key === target.key ? {...candidate, ...patch} : candidate)}
                : item),
        });
    };
    const updateAutomationDelay = (field: AutomationDelayField, rawValue: string) => {
        const value = parseNavigationDelayMs(rawValue);
        if (value === null) {
            setError("等待时间必须为 0–60000 的整数毫秒");
            return;
        }
        setError(null);
        save({...settingsDraftRef.current, [field]: value});
    };
    const pickReferenceImage = async (environment: CalibrationEnvironment, target: CalibrationTarget) => {
        if (!isNativeShell || controlsLocked) return;
        try {
            const picked = await open({
                multiple: false,
                directory: false,
                filters: [{name: "图片文件", extensions: ["png", "jpg", "jpeg", "webp", "bmp"]}],
            });
            if (typeof picked === "string") updateCalibrationTarget(environment, target, {referenceImagePath: picked});
        } catch (cause) {
            setError(String(cause));
        }
    };
    const testCalibrationTarget = async (environment: CalibrationEnvironment, target: CalibrationTarget) => {
        if (!isNativeShell || controlsLocked) return;
        setTestingTargetKey(target.key);
        setCalibrationTestResult(null);
        setError(null);
        try {
            const saved = await flushSettings();
            const result = await testSpecialOpsCalibrationTarget({
                environmentId: environment.id,
                targetKey: target.key,
                settingsRevision: saved.settingsRevision,
            });
            setCalibrationTestResult(formatCalibrationTemplateTestResult(target.label, result));
            reload();
        } catch (cause) {
            setCalibrationTestResult(`${target.label}：测试失败：${String(cause)}`);
            setError(String(cause));
        } finally {
            setTestingTargetKey(null);
        }
    };

    return <main className="space-y-4">
        <header className="flex flex-wrap items-center justify-between gap-3">
            <div><h1 className="text-2xl font-semibold">特勤处自动化</h1><p className="text-sm text-base-content/60">账号、制作台与兑换调度配置</p></div>
            <div className="flex items-center gap-2">
                <span className="text-sm">总开关</span>
                <Switch disabled={controlsLocked} checked={bootstrap.settings.enabled} onCheckedChange={(enabled) => save({...settingsDraftRef.current, enabled})}/>
                <Button disabled={pauseTransition || (hasActiveRun && !isActiveRound)} size="sm" onClick={() => setPaused(!bootstrap.settings.paused)}>
                    {bootstrap.settings.paused ? <RiPlayLine data-icon="inline-start"/> : <RiPauseLine data-icon="inline-start"/>}
                    {bootstrap.settings.paused && pauseTransition ? "正在继续" : pauseTransition ? "正在暂停" : isActiveRound && !bootstrap.settings.paused ? "当前账号结束后暂停" : bootstrap.settings.paused ? "继续" : "暂停"}
                </Button>
                <Button variant="outline" size="sm" onClick={reload}><RiRefreshLine data-icon="inline-start"/>刷新</Button>
            </div>
        </header>

        {error && <div role="alert" className="alert alert-error"><span>{error}</span></div>}

        {bootstrap.settings.paused && bootstrap.settings.pausedReason && <div role="alert" className="alert alert-warning alert-soft">
            <span>自动化已暂停：{bootstrap.settings.pausedReason}。排查后点击「继续」恢复。</span>
        </div>}

        <fieldset disabled={controlsLocked} className="contents">
        <section className="card card-border bg-base-100">
            <div className="card-body gap-4">
                <h2 className="card-title">全局配置</h2>
                <div className="grid gap-3 md:grid-cols-2">
                    <fieldset className="fieldset">
                        <legend className="fieldset-legend">WeGame 可执行文件</legend>
                        <div className="flex gap-2">
                            <Input readOnly value={bootstrap.settings.wegameExecutablePath} placeholder="请选择 WeGame.exe"/>
                            <Button size="sm" variant="outline" onClick={() => void pickExecutable("wegameExecutablePath")}><RiFolderOpenLine data-icon="inline-start"/>选择</Button>
                        </div>
                    </fieldset>
                    <fieldset className="fieldset">
                        <legend className="fieldset-legend">游戏可执行文件</legend>
                        <div className="flex gap-2">
                            <Input readOnly value={bootstrap.settings.gameExecutablePath} placeholder="请选择游戏 .exe"/>
                            <Button size="sm" variant="outline" onClick={() => void pickExecutable("gameExecutablePath")}><RiFolderOpenLine data-icon="inline-start"/>选择</Button>
                        </div>
                    </fieldset>
                    <fieldset className="fieldset">
                        <legend className="fieldset-legend">每日兑换时间（Asia/Shanghai）</legend>
                        <DraftInput value={bootstrap.settings.dailyExchangeTime} placeholder="08:00" onCommit={(dailyExchangeTime) => save({...settingsDraftRef.current, dailyExchangeTime})}/>
                    </fieldset>
                    <fieldset className="fieldset">
                        <legend className="fieldset-legend">紧急停止热键</legend>
                        <Button
                            size="sm"
                            variant="outline"
                            onBlur={recorder.handleBlur}
                            onClick={() => recorder.beginRecording(bootstrap.settings.emergencyHotkey)}
                            onKeyDown={recorder.handleKeyDown}
                        >
                            {recorder.isRecording ? "请按组合键" : `录制紧急停止热键（${bootstrap.settings.emergencyHotkey}）`}
                        </Button>
                        {hotkeyStatus && <p className="label">{hotkeyStatus}</p>}
                    </fieldset>
                </div>
            </div>
        </section>
        </fieldset>

        <section className="card card-border bg-base-100">
            <div className="card-body gap-4">
                <div className="flex flex-wrap items-center justify-between gap-2">
                    <div><h2 className="card-title">单账号试运行</h2><p className="text-sm text-base-content/60">可单独测试登录或从当前游戏进入四制作台页面</p></div>
                    <div className="stat w-auto p-0"><div className="stat-title">待处理账号</div><div className="stat-value text-2xl">{bootstrap.schedule.dueAccounts.length}</div></div>
                </div>
                <div role="alert" className="alert alert-warning alert-soft">
                    <span>先点击“继续”解除暂停。轮次按账号合并到期制作与当天子弹，所有必需模板必须先测试通过。</span>
                </div>
                <div className="grid gap-3 md:grid-cols-2 md:items-end xl:grid-cols-[minmax(0,1fr)_minmax(10rem,auto)_auto_auto_auto_auto_auto]">
                    <fieldset className="fieldset">
                        <legend className="fieldset-legend">试运行账号</legend>
                        <select
                            className="select select-sm w-full"
                            disabled={eligibleAccounts.length === 0 || controlsLocked}
                            value={selectedAccountId ?? ""}
                            onChange={(event) => setSelectedAccountId(event.target.value || null)}
                        >
                            {eligibleAccounts.length === 0 && <option value="">无启用且 QQ 为纯数字的账号</option>}
                            {eligibleAccounts.map((account) => <option key={account.id} value={account.id}>{account.qqAccount}</option>)}
                        </select>
                    </fieldset>
                    <Button disabled={!isNativeShell || !selectedAccountId || controlsLocked} onClick={() => void startLoginTrial()}>
                        <RiPlayLine data-icon="inline-start"/>运行所选账号一次
                    </Button>
                    <Button disabled={!isNativeShell || !selectedAccountId || controlsLocked} variant="outline" onClick={() => void startNavigationTrial()}>
                        <RiPlayLine data-icon="inline-start"/>游戏内导航试运行
                    </Button>
                    <fieldset className="fieldset">
                        <legend className="fieldset-legend">制作试运行目标</legend>
                        <select className="select select-sm w-full" disabled={!selectedAccountId || controlsLocked} value={selectedCraftStation} onChange={(event) => setSelectedCraftStation(event.target.value as StationKind)}>
                            {stationKinds.map((kind) => <option key={kind} value={kind}>{STATION_LABELS[kind]}</option>)}
                        </select>
                    </fieldset>
                    <Button disabled={!isNativeShell || !selectedAccountId || controlsLocked} variant="outline" onClick={() => void startCraftTrial()}>
                        <RiPlayLine data-icon="inline-start"/>制作试运行
                    </Button>
                    <Button disabled={!isNativeShell || !selectedAccountId || controlsLocked} variant="outline" onClick={() => void startCraftBatchTrial()}>
                        <RiPlayLine data-icon="inline-start"/>当前账号四制作台批处理试运行
                    </Button>
                    <Button disabled={!isNativeShell || !selectedAccountId || controlsLocked} variant="outline" onClick={() => void startAmmoTrial()}>
                        <RiPlayLine data-icon="inline-start"/>子弹兑换试运行
                    </Button>
                    <Button disabled={!isNativeShell || !selectedAccountId || controlsLocked} variant="outline" onClick={() => void startLimitedSupplyTrial()}>
                        <RiPlayLine data-icon="inline-start"/>限时商品试运行
                    </Button>
                    <Button disabled={!isNativeShell || !selectedAccountId || controlsLocked} variant="outline" onClick={() => void startMarketTrial()}>
                        <RiPlayLine data-icon="inline-start"/>交易行试运行
                    </Button>
                    {!isActiveRound && <Button disabled={!hasActiveRun || runSnapshot?.status === "stopping"} variant="outline" onClick={() => void cancelLoginTrial()}>
                        <RiStopCircleLine data-icon="inline-start"/>取消本次试运行
                    </Button>}
                </div>
                {runSnapshot && <div className="grid gap-2 rounded-box bg-base-200 p-3 text-sm sm:grid-cols-2 lg:grid-cols-4">
                    <div><span className="text-base-content/60">步骤</span><p className="font-medium">{runSnapshot.currentStep ?? "准备"}</p></div>
                    <div><span className="text-base-content/60">消息</span><p className="font-medium">{runSnapshot.message}</p></div>
                    <div><span className="text-base-content/60">倒计时</span><p className="font-medium">{runSnapshot.countdownSeconds === null ? "-" : `${runSnapshot.countdownSeconds} 秒`}</p></div>
                    <div><span className="text-base-content/60">状态</span><p className="font-medium">{runSnapshot.status}</p></div>
                </div>}
                {selectedAccount?.lastFailure && <div role="alert" className="alert alert-error alert-soft">
                    <span>{selectedAccount.lastFailure.step}：{selectedAccount.lastFailure.message}（{new Date(selectedAccount.lastFailure.atMs).toLocaleString("zh-CN")}）</span>
                </div>}
            </div>
        </section>

        <SpecialOpsTimeline
            bootstrap={bootstrap}
            nowMs={timelineNowMs}
            disabled={controlsLocked}
            onConfirmStation={confirmTimelineStation}
            onConfirmAmmo={confirmTimelineAmmo}
            onAcknowledge={acknowledgeLimitedSupply}
        />

        <SpecialOpsProfitFilter
            bootstrap={bootstrap}
            isNativeShell={isNativeShell}
            onSave={saveProfitSettings}
        />

        <fieldset disabled={controlsLocked} className="contents">
        <section className="card card-border bg-base-100">
            <div className="card-body gap-3">
                <div className="flex flex-wrap items-center justify-between gap-2">
                    <div><h2 className="card-title">限时商品通用设置</h2><p className="text-xs text-base-content/60">12:00、20:00 固定检查；颜色 1/2 共用全局配置。</p></div>
                    <label className="flex items-center gap-2 text-sm"><Switch checked={limitedSupply.enabled} onCheckedChange={(enabled) => updateLimitedSupply({enabled})}/>启用</label>
                </div>
                <div className="grid gap-3 sm:grid-cols-2">
                    <label className="form-control gap-1"><span className="label-text text-xs">研发部门页面等待（ms）</span><DraftInput type="number" min={0} max={60000} value={String(limitedSupply.researchDelayMs)} onCommit={(value) => updateLimitedSupply({researchDelayMs: Math.max(0, Math.min(60000, Math.trunc(Number(value) || 0)))})}/><span className="text-xs text-base-content/60">点完研发部门后先等这段时间再识别页面，太短会在上一页误判</span></label>
                    <label className="form-control gap-1"><span className="label-text text-xs">页面就绪超时（ms）</span><DraftInput type="number" min={1000} max={60000} value={String(limitedSupply.readyTimeoutMs)} onCommit={(value) => updateLimitedSupply({readyTimeoutMs: Math.max(1000, Math.min(60000, Math.trunc(Number(value) || 0)))})}/></label>
                </div>
                <div className="grid gap-3 lg:grid-cols-2">
                    {[0, 1].map((colorIndex) => {
                        const color = limitedSupply.colors[colorIndex] ?? [0, 0, 0];
                        return <div key={colorIndex} className="rounded-box border border-base-300 p-3">
                            <h3 className="font-medium">颜色 {colorIndex + 1}</h3>
                            <div className="mt-2 flex items-end gap-2">
                                <label className="form-control gap-1">
                                    <span className="label-text text-xs">目标颜色</span>
                                    <input
                                        type="color"
                                        value={limitedColorToHex(color)}
                                        onChange={(event) => {
                                            const next = parseLimitedColorHex(event.target.value);
                                            if (next) updateLimitedColor(colorIndex, next);
                                        }}
                                        className="h-9 w-12 cursor-pointer border border-base-300 bg-transparent p-0"
                                        aria-label={`颜色 ${colorIndex + 1}`}
                                    />
                                </label>
                                <label className="form-control min-w-0 flex-1 gap-1">
                                    <span className="label-text text-xs">Hex</span>
                                    <DraftInput className="font-mono" value={limitedColorToHex(color)} onCommit={(value) => commitLimitedColorHex(colorIndex, value)}/>
                                </label>
                                <label className="form-control w-28 gap-1">
                                    <span className="label-text text-xs">容差</span>
                                    <DraftInput type="number" min={0} max={255} value={String(limitedSupply.colorTolerances[colorIndex])} onCommit={(value) => {
                                        const tolerances = [...limitedSupply.colorTolerances] as [number, number];
                                        tolerances[colorIndex] = Math.max(0, Math.min(255, Math.trunc(Number(value) || 0)));
                                        updateLimitedSupply({colorTolerances: tolerances});
                                    }}/>
                                </label>
                            </div>
                        </div>;
                    })}
                </div>
                <div className="flex flex-wrap items-center gap-2"><Button size="sm" variant="outline" disabled={!activeEnvironment || controlsLocked} onClick={() => void testLimitedColors()}><RiPlayLine data-icon="inline-start"/>测试限时商品识色</Button>{limitedColorFeedback && <span role="alert" className="text-xs text-base-content/70">{limitedColorFeedback}</span>}</div>
            </div>
        </section>
        </fieldset>


        <fieldset disabled={controlsLocked} className="contents">
        <section className="space-y-3 rounded-box border border-base-300 bg-base-100 p-4">
            <div><h2 className="text-lg font-semibold">默认账号配置</h2><p className="text-xs text-base-content/60">独立设置关闭的账号统一继承。修改时长不重算当前制作完成时间，下次重做后生效。</p></div>
            <div className="grid gap-2 sm:grid-cols-2 lg:grid-cols-4">
                {bootstrap.settings.defaultBusinessConfig.stations.map((station) => {
                    const recipeTarget = activeEnvironment?.targets.find((target) => target.key === `craft.recipe.${station.kind}`);
                    return <div key={station.kind} className="rounded-box bg-base-200 p-3">
                        <div className="flex items-center justify-between"><span className="text-sm font-medium">{STATION_LABELS[station.kind]}</span><Switch checked={station.enabled} onCheckedChange={(enabled) => updateDefaultStation(station, {enabled})}/></div>
                        <div className="mt-2 grid grid-cols-2 gap-2">
                            <label className="text-xs">小时<DraftInput type="number" min={0} max={168} value={String(Math.floor(station.durationMinutes / 60))} disabled={!station.enabled} onCommit={(hours) => updateDefaultStation(station, {durationMinutes: Number(hours) * 60 + station.durationMinutes % 60})}/></label>
                            <label className="text-xs">分钟<DraftInput type="number" min={0} max={59} value={String(station.durationMinutes % 60)} disabled={!station.enabled} onCommit={(minutes) => updateDefaultStation(station, {durationMinutes: Math.floor(station.durationMinutes / 60) * 60 + Number(minutes)})}/></label>
                        </div>
                        <label className="form-control mt-2 gap-1"><span className="label-text text-xs">制作物品备注</span><DraftInput value={station.recipeNote} onCommit={(recipeNote) => updateDefaultStation(station, {recipeNote})}/></label>
                        <div className="mt-2 flex items-center justify-between gap-2">
                            <span className="text-xs text-base-content/60">制作物品选择点击点：{recipeTarget?.rect ? `${recipeTarget.rect.x}, ${recipeTarget.rect.y}` : "未配置"}</span>
                            <Button disabled={!activeEnvironment} size="sm" variant="outline" onClick={() => activeEnvironment && void beginCalibration(activeEnvironment, `craft.recipe.${station.kind}`)}><RiCrosshair2Line data-icon="inline-start"/>{recipeTarget?.rect ? "重选" : "选择"}</Button>
                        </div>
                    </div>;
                })}
            </div>
            <details className="collapse collapse-arrow">
                <summary className="collapse-title">默认交易行购买</summary>
                <div className="collapse-content grid gap-3 sm:grid-cols-2">
                    <p className="text-xs text-base-content/60 sm:col-span-2">时间窗口适用于所有账号</p>
                    <label className="form-control gap-1"><span className="label-text text-xs">开放开始时间</span><DraftInput type="time" value={minutesToTime(marketPurchase.windowStartMinute)} onCommit={(value) => updateMarketPurchase({windowStartMinute: timeToMinutes(value)})}/></label>
                    <label className="form-control gap-1"><span className="label-text text-xs">开放结束时间</span><DraftInput type="time" value={minutesToTime(marketPurchase.windowEndMinute)} onCommit={(value) => updateMarketPurchase({windowEndMinute: timeToMinutes(value)})}/></label>
                    <label className="form-control gap-1"><span className="label-text text-xs">进入后等待（ms）</span><DraftInput type="number" min={0} max={60000} value={String(marketPurchase.entryDelayMs)} onCommit={(value) => updateMarketPurchase({entryDelayMs: Math.max(0, Math.min(60000, Math.trunc(Number(value) || 0)))})}/><span className="text-xs text-base-content/60">点击进入交易行大厅后等待页面稳定的时间</span></label>
                    <p className="text-xs text-base-content/60 sm:col-span-2 mt-1">以下为默认购买配置，独立设置关闭的账号继承</p>
                    <label className="flex items-center gap-2 text-sm"><Switch checked={defaultMarket.enabled} onCheckedChange={(enabled) => save({...settingsDraftRef.current, defaultBusinessConfig: {...settingsDraftRef.current.defaultBusinessConfig, market: {...defaultMarket, enabled}}})}/>启用默认购买</label>
                    <label className="form-control gap-1"><span className="label-text text-xs">购买次数</span><DraftInput type="number" min={1} value={String(defaultMarket.purchaseCount)} onCommit={(value) => save({...settingsDraftRef.current, defaultBusinessConfig: {...settingsDraftRef.current.defaultBusinessConfig, market: {...defaultMarket, purchaseCount: Math.max(1, Math.trunc(Number(value) || 1))}}})}/></label>
                    <label className="form-control gap-1"><span className="label-text text-xs">最高价</span><DraftInput type="number" min={1} value={String(defaultMarket.maxPrice)} onCommit={(value) => save({...settingsDraftRef.current, defaultBusinessConfig: {...settingsDraftRef.current.defaultBusinessConfig, market: {...defaultMarket, maxPrice: Math.max(1, Math.trunc(Number(value) || 1))}}})}/></label>
                    <label className="form-control gap-1"><span className="label-text text-xs">商品备注</span><DraftInput value={defaultMarket.itemNote} onCommit={(itemNote) => save({...settingsDraftRef.current, defaultBusinessConfig: {...settingsDraftRef.current.defaultBusinessConfig, market: {...defaultMarket, itemNote}}})}/></label>
                    <div className="flex items-center justify-between gap-2 sm:col-span-2">
                        <span className="text-xs text-base-content/60">商品入口点击点：{defaultMarket.productPoint ? `${defaultMarket.productPoint.x}, ${defaultMarket.productPoint.y}` : "未配置"}</span>
                        <Button disabled={!activeEnvironment} size="sm" variant="outline" onClick={() => activeEnvironment && void beginCalibration(activeEnvironment, "business.market.product")}><RiCrosshair2Line data-icon="inline-start"/>{defaultMarket.productPoint ? "重选" : "选择"}</Button>
                    </div>
                </div>
            </details>
            <details className="collapse collapse-arrow">
                <summary className="collapse-title">默认子弹兑换顺序</summary>
                <div className="collapse-content">
                    <AmmoTargetEditor
                        title="子弹兑换顺序"
                        targets={bootstrap.settings.defaultBusinessConfig.ammoTargets}
                        onChange={(ammoTargets) => save({
                            ...settingsDraftRef.current,
                            defaultBusinessConfig: {
                                ...settingsDraftRef.current.defaultBusinessConfig,
                                ammoTargets,
                            },
                        })}
                        onSelectPoint={(target) => activeEnvironment && void beginCalibration(activeEnvironment, `business.ammo.${target.id}`)}
                    />
                </div>
            </details>
        </section>
        </fieldset>

        <section className="space-y-3">
            <div className="flex flex-wrap items-center justify-between gap-2">
                <h2 className="text-lg font-semibold">账号</h2>
                <Button
                    size="sm"
                    variant="outline"
                    disabled={!anyAccountRestorable || !isNativeShell}
                    title={anyAccountRestorable ? "清除全部账号的异常状态" : "当前没有需要恢复的异常状态"}
                    onClick={() => void restoreAccountState(null)}
                ><RiRefreshLine data-icon="inline-start"/>全部一键恢复</Button>
            </div>
            {bootstrap.settings.accounts.length === 0 && <div className="rounded-box border border-dashed border-base-300 p-8 text-center text-sm text-base-content/60">暂无账号，点击“添加账号”开始配置</div>}
            {bootstrap.settings.accounts.map((account, index) => {
                const due = bootstrap.schedule.dueAccounts.find((item) => item.accountId === account.id);
                const business = account.independentBusinessConfig;
                const manualCheckRequired = account.status === "needsManualLogin" || account.status === "loginFailed" || account.status === "manualCheckRequired" || account.status === "uncertain" || account.status === "isolated";
                return <article key={account.id} className="rounded-box border border-base-300 bg-base-100 p-4">
                    <div className="flex flex-wrap items-center justify-between gap-2">
                        <div><h3 className="font-semibold">账号 {index + 1}</h3><p className="text-xs text-base-content/60">状态：{accountStatusLabels[account.status]}</p></div>
                        <div className="flex flex-wrap items-center gap-2 text-sm">
                            <fieldset disabled={controlsLocked} className="contents">
                                <Button size="sm" variant="outline" onClick={() => openCorrection(account.id)}>人工校正制作与子弹状态</Button>
                            </fieldset>
                            {manualCheckRequired && <Button size="sm" variant="outline" disabled={!isNativeShell} onClick={() => void confirmAccountManualCheck(account.id)}>已人工检查</Button>}
                            <Button
                                size="sm"
                                variant="outline"
                                disabled={!accountRestorable(account, currentDay) || !isNativeShell}
                                title={accountRestorable(account, currentDay)
                                    ? "清除异常状态：制作台按异常前剩余时间恢复，失败与当天已兑换子弹回未兑换"
                                    : "当前账号没有需要恢复的异常状态"}
                                onClick={() => void restoreAccountState(account.id)}
                            ><RiRefreshLine data-icon="inline-start"/>一键恢复状态</Button>
                            <fieldset disabled={controlsLocked} className="contents">
                                <span>启用</span>
                                <Switch checked={account.enabled} onCheckedChange={(enabled) => updateAccount(account, {enabled})}/>
                                <Button variant="ghost" size="icon-sm" title="删除账号" onClick={() => removeAccount(account)}><RiDeleteBinLine/></Button>
                            </fieldset>
                        </div>
                    </div>
                    {accountActionError?.accountId === account.id && <div role="alert" className="alert alert-error alert-soft mt-2 py-2 text-xs"><span>{accountActionError.message}</span></div>}
                    <fieldset disabled={controlsLocked} className="contents">
                    <div className="mt-3 grid gap-3 md:grid-cols-2">
                        <label className="form-control gap-1"><span className="label-text">QQ 账号（纯数字）</span><DraftInput value={account.qqAccount} onCommit={(qqAccount) => updateAccount(account, {qqAccount})}/><span className="text-xs text-base-content/60">需提前在 WeGame 登录并勾选“记住密码”</span></label>
                        <label className="flex items-center gap-2 self-end text-sm"><Switch checked={account.independentSettingsEnabled} onCheckedChange={(enabled) => setIndependentSettings(account, enabled)}/>独立设置</label>
                    </div>
                    {!account.independentSettingsEnabled ? <div className="mt-3 rounded-box bg-base-200 p-3 text-sm">
                        <span className="font-medium">继承默认配置</span>
                        <span className="ml-2 text-base-content/60">关闭独立设置时隐藏制作时长、配方点击点与子弹目标编辑器。</span>
                    </div> : business ? <details className="collapse collapse-arrow mt-3">
                    <summary className="collapse-title">独立设置</summary>
                    <div className="collapse-content">
                    <div className="grid gap-2 sm:grid-cols-2 lg:grid-cols-4">
                        {business.stations.map((station) => {
                            const runtime = account.stations.find((item) => item.kind === station.kind);
                            const recipePoint = business.recipePoints.find((item) => item.kind === station.kind);
                            return <div key={station.kind} className="rounded-box bg-base-200 p-3">
                                <div className="flex items-center justify-between"><span className="text-sm font-medium">{STATION_LABELS[station.kind]}</span><Switch checked={station.enabled} onCheckedChange={(enabled) => updateIndependentStation(account, station, {enabled})}/></div>
                                <div className="mt-2 grid grid-cols-2 gap-2">
                                    <label className="text-xs">小时<DraftInput type="number" min={0} max={168} value={String(Math.floor(station.durationMinutes / 60))} disabled={!station.enabled} onCommit={(hours) => updateIndependentStation(account, station, {durationMinutes: Number(hours) * 60 + station.durationMinutes % 60})}/></label>
                                    <label className="text-xs">分钟<DraftInput type="number" min={0} max={59} value={String(station.durationMinutes % 60)} disabled={!station.enabled} onCommit={(minutes) => updateIndependentStation(account, station, {durationMinutes: Math.floor(station.durationMinutes / 60) * 60 + Number(minutes)})}/></label>
                                </div>
                                <label className="form-control mt-2 gap-1"><span className="label-text text-xs">制作物品备注</span><DraftInput value={station.recipeNote} onCommit={(recipeNote) => updateIndependentStation(account, station, {recipeNote})}/></label>
                                <div className="mt-2 flex items-center justify-between gap-2">
                                    <span className="text-xs text-base-content/60">账号级制作物品选择点击点：{recipePoint ? `${recipePoint.rect.x}, ${recipePoint.rect.y}` : "继承全局"}</span>
                                    <Button
                                        disabled={!activeEnvironment}
                                        size="sm"
                                        variant="outline"
                                        onClick={() => activeEnvironment && void beginCalibration(activeEnvironment, `craft.recipe.${station.kind}`, account.id)}
                                    ><RiCrosshair2Line data-icon="inline-start"/>{recipePoint ? "重选" : "选择"}</Button>
                                </div>
                                <div className="mt-2 text-xs text-base-content/60">{runtime?.status ?? "unknown"}{due?.stationKinds.includes(station.kind) ? " · 到期" : ""}</div>
                            </div>;
                        })}
                    </div>
                    <AmmoTargetEditor
                        title="子弹兑换顺序"
                        targets={business.ammoTargets}
                        pendingCount={due?.ammoTargetIds.length ?? 0}
                        onChange={(ammoTargets) => updateIndependentBusiness(account, {ammoTargets})}
                        onSelectPoint={(target) => activeEnvironment && void beginCalibration(activeEnvironment, `business.ammo.${target.id}`, account.id)}
                    />
                    {business.market && <details className="collapse collapse-arrow mt-3 border border-base-300">
                        <summary className="collapse-title">独立交易行配置</summary>
                        <div className="collapse-content grid gap-3 sm:grid-cols-2">
                            <label className="flex items-center gap-2 text-sm"><Switch checked={business.market?.enabled ?? false} onCheckedChange={(enabled) => updateIndependentBusiness(account, {market: {...business.market!, enabled}})}/>启用独立交易行购买</label>
                            <label className="form-control gap-1"><span className="label-text text-xs">购买次数</span><DraftInput type="number" min={1} value={String(business.market?.purchaseCount ?? 1)} onCommit={(value) => updateIndependentBusiness(account, {market: {...business.market!, purchaseCount: Math.max(1, Math.trunc(Number(value) || 1))}})}/></label>
                            <label className="form-control gap-1"><span className="label-text text-xs">最高价</span><DraftInput type="number" min={1} value={String(business.market?.maxPrice ?? 1)} onCommit={(value) => updateIndependentBusiness(account, {market: {...business.market!, maxPrice: Math.max(1, Math.trunc(Number(value) || 1))}})}/></label>
                            <label className="form-control gap-1"><span className="label-text text-xs">商品备注</span><DraftInput value={business.market?.itemNote ?? ""} onCommit={(itemNote) => updateIndependentBusiness(account, {market: {...business.market!, itemNote}})}/></label>
                            <div className="flex items-center justify-between gap-2 sm:col-span-2">
                                <span className="text-xs text-base-content/60">商品入口点击点：{business.market.productPoint ? `${business.market.productPoint.x}, ${business.market.productPoint.y}` : "未配置"}</span>
                                <Button disabled={!activeEnvironment} size="sm" variant="outline" onClick={() => activeEnvironment && void beginCalibration(activeEnvironment, "business.market.product", account.id)}><RiCrosshair2Line data-icon="inline-start"/>{business.market.productPoint ? "重选" : "选择"}</Button>
                            </div>
                            <span className="text-xs text-base-content/60">独立配置未开启时继承默认交易行购买；业务点仍按显示环境全局校准。</span>
                        </div>
                    </details>}
                    </div>
                    </details> : <div role="alert" className="alert alert-error mt-3"><span>独立设置已开启，但独立业务配置缺失。请关闭后重新开启。</span></div>}
                    </fieldset>
                </article>;
            })}
            <fieldset disabled={controlsLocked} className="contents">
                <Button size="sm" onClick={addAccount}><RiAddLine data-icon="inline-start"/>添加账号</Button>
            </fieldset>
        </section>

        <fieldset disabled={controlsLocked} className="contents">
        <section className="space-y-3 rounded-box border border-base-300 bg-base-100 p-4">
            <div className="flex flex-wrap items-center justify-between gap-2">
                <div><h2 className="text-lg font-semibold">点击区域校准</h2><p className="text-xs text-base-content/60">坐标按当前显示环境全局保存，不按账号复制。显示环境变化后重新校准。</p></div>
            </div>
            {calibrationTestResult && <div role="alert" className="alert alert-info"><span>{calibrationTestResult}</span></div>}
            {activeEnvironment && <>
                <div className="overflow-x-auto rounded-box border border-base-300">
                    <table className="table table-sm">
                        <thead><tr><th>步骤</th><th>类型</th><th>坐标</th><th>参考图</th><th className="text-right">操作</th></tr></thead>
                        <tbody>{activeEnvironment.targets.map((target) => <Fragment key={target.key}><tr>
                            <td><div className="font-medium">{target.label}</div><div className="font-mono text-xs text-base-content/50">{target.key}</div>{target.guardAnyOf.length > 0 && <div className="mt-1 text-xs text-base-content/60">前置：{target.guardAnyOf.join(" / ")}</div>}</td>
                            <td>{target.kind === "clickPoint" ? "点击点" : target.kind === "inputRegion" ? "输入区域" : target.recognitionMethod === "ocr" ? "OCR 区域" : "模板识别区域"}</td>
                            <td className="font-mono text-xs">{target.rect ? `${target.rect.x}, ${target.rect.y}, ${target.rect.width}×${target.rect.height}` : "未配置"}</td>
                            <td className="max-w-40 truncate text-xs" title={target.referenceImagePath ?? undefined}>
                                {target.recognitionMethod === "template" ? target.referenceImagePath?.split(/[\\/]/).pop() ?? "未上传" : target.recognitionMethod === "ocr" ? "按业务配置比对文本" : "-"}
                            </td>
                            <td className="text-right">
                                <div className="join">
                                    {target.recognitionMethod === "template" && <Button className="join-item" size="sm" variant="outline" onClick={() => void pickReferenceImage(activeEnvironment, target)}><RiFolderOpenLine data-icon="inline-start"/>{target.referenceImagePath ? "替换" : "上传"}</Button>}
                                    {target.recognitionMethod === "template" && target.referenceImagePath && <Button aria-label="清除参考图" className="join-item" size="icon-sm" title="清除参考图" variant="outline" onClick={() => updateCalibrationTarget(activeEnvironment, target, {referenceImagePath: null})}><RiDeleteBinLine data-icon="inline-start"/></Button>}
                                    {target.recognitionMethod && <Button className="join-item" disabled={testingTargetKey === target.key} size="sm" title={["game.", "craft.", "ammo."].some((prefix) => target.key.startsWith(prefix)) ? "游戏内模板测试将在 3 秒后切换到游戏窗口" : undefined} variant="outline" onClick={() => void testCalibrationTarget(activeEnvironment, target)}><RiPlayLine data-icon="inline-start"/>{testingTargetKey === target.key ? "测试中" : "测试"}</Button>}
                                    <Button className="join-item" size="sm" variant={target.rect ? "outline" : "default"} onClick={() => void beginCalibration(activeEnvironment, target.key)}><RiCrosshair2Line data-icon="inline-start"/>{target.rect ? "重新框选" : "框选"}</Button>
                                </div>
                                {target.key === "game.specialOps" && <label className="mt-2 flex items-center justify-end gap-2 text-xs">
                                    <span>点击前等待（ms）</span>
                                    <DraftInput className="w-28" inputMode="numeric" value={String(bootstrap.settings.navigationSpecialOpsDelayMs)} onCommit={(value) => updateAutomationDelay("navigationSpecialOpsDelayMs", value)}/>
                                </label>}
                            </td>
                        </tr>
                        {target.key === "ammo.supply" && <tr className="bg-base-200/50">
                            <td><div className="font-medium">点击军需处前等待</div></td>
                            <td colSpan={4}><label className="flex items-center gap-2 text-xs"><span>等待时间（ms）</span><DraftInput className="w-28" inputMode="numeric" value={String(bootstrap.settings.ammoSupplyDelayMs)} onCommit={(value) => updateAutomationDelay("ammoSupplyDelayMs", value)}/><span className="text-base-content/60">0–60000</span></label></td>
                        </tr>}
                        {target.key === "ammo.enterSupply" && <tr className="bg-base-200/50">
                            <td><div className="font-medium">点击进入军需处前等待</div></td>
                            <td colSpan={4}><label className="flex items-center gap-2 text-xs"><span>等待时间（ms）</span><DraftInput className="w-28" inputMode="numeric" value={String(bootstrap.settings.ammoTacticalDelayMs)} onCommit={(value) => updateAutomationDelay("ammoTacticalDelayMs", value)}/><span className="text-base-content/60">0–60000</span></label></td>
                        </tr>}
                        {target.key === "ammo.seasonal" && <tr className="bg-base-200/50">
                            <td><div className="font-medium">点击赛季限定入口后等待</div><div className="text-xs text-base-content/60">开始子弹兑换前等待界面稳定</div></td>
                            <td colSpan={4}><label className="flex items-center gap-2 text-xs"><span>等待时间（ms）</span><DraftInput className="w-28" inputMode="numeric" value={String(bootstrap.settings.ammoSeasonalEntryDelayMs)} onCommit={(value) => updateAutomationDelay("ammoSeasonalEntryDelayMs", value)}/><span className="text-base-content/60">0–60000</span></label></td>
                        </tr>}
                        {target.key === "craft.confirmPinned" && <>
                            <tr className="bg-base-200/50">
                                <td><div className="font-medium">收取点击后按 Space 等待</div></td>
                                <td colSpan={4}><label className="flex items-center gap-2 text-xs"><span>等待时间（ms）</span><DraftInput className="w-28" inputMode="numeric" value={String(bootstrap.settings.craftSpaceDelayMs)} onCommit={(value) => updateAutomationDelay("craftSpaceDelayMs", value)}/><span className="text-base-content/60">0–60000</span></label></td>
                            </tr>
                            <tr className="bg-base-200/50">
                                <td><div className="font-medium">Space 后再次点击制作台等待</div></td>
                                <td colSpan={4}><label className="flex items-center gap-2 text-xs"><span>等待时间（ms）</span><DraftInput className="w-28" inputMode="numeric" value={String(bootstrap.settings.craftReopenDelayMs)} onCommit={(value) => updateAutomationDelay("craftReopenDelayMs", value)}/><span className="text-base-content/60">0–60000</span></label></td>
                            </tr>
                            <tr className="bg-base-200/50">
                                <td><div className="font-medium">再次点击后确认置顶等待</div></td>
                                <td colSpan={4}><label className="flex items-center gap-2 text-xs"><span>等待时间（ms）</span><DraftInput className="w-28" inputMode="numeric" value={String(bootstrap.settings.craftConfirmPinnedDelayMs)} onCommit={(value) => updateAutomationDelay("craftConfirmPinnedDelayMs", value)}/><span className="text-base-content/60">0–60000</span></label></td>
                            </tr>
                        </>}
                        {target.key === "game.beaconMode" && <>
                            <tr className="bg-base-200/50">
                                <td><div className="font-medium">点击烽火地带前等待</div><div className="text-xs text-base-content/60">模式选择识别成功后</div></td>
                                <td colSpan={4}><label className="flex items-center gap-2 text-xs"><span>等待时间（ms）</span><DraftInput className="w-28" inputMode="numeric" value={String(bootstrap.settings.navigationBeaconDelayMs)} onCommit={(value) => updateAutomationDelay("navigationBeaconDelayMs", value)}/><span className="text-base-content/60">0–60000</span></label></td>
                            </tr>
                            <tr className="bg-base-200/50">
                                <td><div className="font-medium">Space 前等待</div><div className="text-xs text-base-content/60">点击烽火地带后</div></td>
                                <td colSpan={4}><label className="flex items-center gap-2 text-xs"><span>等待时间（ms）</span><DraftInput className="w-28" inputMode="numeric" value={String(bootstrap.settings.navigationSpaceDelayMs)} onCommit={(value) => updateAutomationDelay("navigationSpaceDelayMs", value)}/><span className="text-base-content/60">0–60000</span></label></td>
                            </tr>
                            <tr className="bg-base-200/50">
                                <td><div className="font-medium">Tab 前等待</div><div className="text-xs text-base-content/60">按 Space 后</div></td>
                                <td colSpan={4}><label className="flex items-center gap-2 text-xs"><span>等待时间（ms）</span><DraftInput className="w-28" inputMode="numeric" value={String(bootstrap.settings.navigationTabDelayMs)} onCommit={(value) => updateAutomationDelay("navigationTabDelayMs", value)}/><span className="text-base-content/60">0–60000</span></label></td>
                            </tr>
                        </>}
                        </Fragment>)}</tbody>
                    </table>
                </div>
            </>}
        </section>
        </fieldset>
        {correctionAccount && <dialog open className="modal modal-middle">
            <div className="modal-box max-w-3xl">
                <h3 className="text-lg font-semibold">人工校正制作与子弹状态</h3>
                <p className="mt-1 text-sm text-base-content/60">账号 {correctionAccount.qqAccount || correctionAccount.id}。选中项提交后原子恢复调度，未选中项保持不变。</p>
                {correctionError && <div role="alert" className="alert alert-error mt-3"><span>{correctionError}</span></div>}
                {correctionConfirming && correctionPayload ? (
                    <div className="mt-4">
                        <div role="alert" className="alert alert-warning"><span>确认后将覆盖所选项的制作计时与子弹状态，并清除对应失败记录。</span></div>
                        <ul className="list mt-3">
                            {correctionPayload.map((item) => <li key={item.kind} className="list-row border-t border-base-300 px-0">
                                <span className="font-medium">{STATION_LABELS[item.kind]}</span>
                                <span className="list-col-grow text-sm text-base-content/70">
                                    {item.state === "immediateDue" ? "立即到期" : item.state === "idle" ? "空闲" : `正在制作，剩余 ${item.remainingMinutes} 分钟`}
                                </span>
                            </li>)}
                            {correctionAmmoPayload.map((item) => <li key={item.targetId} className="list-row border-t border-base-300 px-0">
                                <span className="font-medium">{correctionAmmoTargets.find((target) => target.id === item.targetId)?.note || item.targetId}</span>
                                <span className="list-col-grow text-sm text-base-content/70">{item.succeededToday ? "当天已成功兑换" : "当天未成功兑换"}</span>
                            </li>)}
                        </ul>
                    </div>
                ) : (
                    <><div className="mt-4 grid gap-3 sm:grid-cols-2">
                        {stationKinds.map((kind) => {
                            const item = correctionDraft[kind];
                            return <fieldset key={kind} className="fieldset rounded-box border border-base-300 p-3">
                                <legend className="fieldset-legend">{STATION_LABELS[kind]}</legend>
                                <label className="label cursor-pointer justify-start gap-2">
                                    <input className="radio radio-sm" type="radio" name={`correction-${kind}`} checked={item.state === "immediateDue"} onChange={() => updateCorrection(kind, {state: "immediateDue"})}/>
                                    立即到期
                                </label>
                                <label className="label cursor-pointer justify-start gap-2">
                                    <input className="radio radio-sm" type="radio" name={`correction-${kind}`} checked={item.state === "crafting"} onChange={() => updateCorrection(kind, {state: "crafting"})}/>
                                    正在制作
                                </label>
                                {item.state === "crafting" && <div className="grid grid-cols-2 gap-2">
                                    <label className="fieldset"><span className="fieldset-legend">剩余小时</span><input className="input input-sm" type="number" min={0} max={168} step={1} value={item.hours} onChange={(event) => updateCorrection(kind, {hours: event.target.value})}/></label>
                                    <label className="fieldset"><span className="fieldset-legend">剩余分钟</span><input className="input input-sm" type="number" min={0} max={59} step={1} value={item.minutes} onChange={(event) => updateCorrection(kind, {minutes: event.target.value})}/></label>
                                </div>}
                                <label className="label cursor-pointer justify-start gap-2">
                                    <input className="radio radio-sm" type="radio" name={`correction-${kind}`} checked={item.state === "idle"} onChange={() => updateCorrection(kind, {state: "idle"})}/>
                                    空闲
                                </label>
                                <label className="label cursor-pointer justify-start gap-2">
                                    <input className="radio radio-sm" type="radio" name={`correction-${kind}`} checked={item.state === null} onChange={() => updateCorrection(kind, {state: null})}/>
                                    不修改
                                </label>
                            </fieldset>;
                        })}
                    </div>
                    <div className="mt-4 rounded-box border border-base-300 p-3">
                        <h4 className="font-medium">当天子弹兑换状态</h4>
                        {correctionAmmoTargets.length === 0 ? <p className="mt-2 text-sm text-base-content/60">当前账号没有启用的子弹目标</p> : <ul className="list mt-2">
                            {correctionAmmoTargets.map((target) => <li key={target.id} className="list-row items-center border-t border-base-300 px-0">
                                <span className="list-col-grow text-sm font-medium">{target.note || target.id}</span>
                                <label className="label cursor-pointer gap-2"><input className="radio radio-sm" type="radio" name={`ammo-correction-${target.id}`} checked={correctionAmmoDraft[target.id] === true} onChange={() => { setCorrectionAmmoDraft((current) => ({...current, [target.id]: true})); setCorrectionConfirming(false); }}/>当天已成功兑换</label>
                                <label className="label cursor-pointer gap-2"><input className="radio radio-sm" type="radio" name={`ammo-correction-${target.id}`} checked={correctionAmmoDraft[target.id] === false} onChange={() => { setCorrectionAmmoDraft((current) => ({...current, [target.id]: false})); setCorrectionConfirming(false); }}/>当天未成功兑换</label>
                                <label className="label cursor-pointer gap-2"><input className="radio radio-sm" type="radio" name={`ammo-correction-${target.id}`} checked={correctionAmmoDraft[target.id] === null || correctionAmmoDraft[target.id] === undefined} onChange={() => { setCorrectionAmmoDraft((current) => ({...current, [target.id]: null})); setCorrectionConfirming(false); }}/>不修改</label>
                            </li>)}
                        </ul>}
                    </div>
                    <CorrectionLimitedSupply
                        account={correctionAccount}
                        disabled={controlsLocked || !isNativeShell || correctionSubmitting}
                        onRecheck={recheckLimitedSupply}
                        onAcknowledge={acknowledgeLimitedSupply}
                    /></>
                )}
                <div className="modal-action">
                    <Button variant="ghost" onClick={closeCorrection}>取消</Button>
                    {correctionConfirming ? <>
                        <Button disabled={correctionSubmitting} variant="outline" onClick={() => setCorrectionConfirming(false)}>返回修改</Button>
                        <Button disabled={correctionSubmitting} onClick={() => void submitCorrection()}>{correctionSubmitting ? "正在保存" : "确认制作台与子弹状态并保存"}</Button>
                    </> : <Button disabled={!correctionPayload || (correctionPayload.length === 0 && correctionAmmoPayload.length === 0)} onClick={() => setCorrectionConfirming(true)}>核对制作台与子弹状态</Button>}
                </div>
            </div>
        </dialog>}
    </main>;
}
