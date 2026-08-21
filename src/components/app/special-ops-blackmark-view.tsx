import {RiPauseLine, RiPlayLine, RiRefreshLine} from "@remixicon/react";
import type {ReactNode} from "react";

import {BlackmarkPage, BlackmarkSpec} from "@/components/app/blackmark-page";
import type {
    AccountPlan,
    SpecialOpsBootstrap,
    StationCorrectionInput,
    TimelineTask,
} from "@/components/app/special-ops-types";
import {STATION_LABELS} from "@/components/app/special-ops-types";
import {
    accountRestorable,
    timelineDelayMinutes,
    timelineTaskAllowsInlineCorrection,
    timelineTaskLabel,
} from "@/components/app/special-ops-utils";

const shanghaiTimeFormatter = new Intl.DateTimeFormat("zh-CN", {
    timeZone: "Asia/Shanghai",
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
    hour12: false,
});

const accountStatusLabels: Record<AccountPlan["status"], string> = {
    ready: "就绪",
    needsManualLogin: "需手动登录",
    loginFailed: "登录失败",
    manualCheckRequired: "需人工检查",
    uncertain: "不确定",
    isolated: "已隔离",
};

type SpecialOpsBlackmarkViewProps = {
    accountActionError: {accountId: string; message: string} | null;
    bootstrap: SpecialOpsBootstrap;
    children: ReactNode;
    controlsLocked: boolean;
    currentDay: string;
    error: string | null;
    hasActiveRun: boolean;
    isActiveRound: boolean;
    isNativeShell: boolean;
    nowMs: number;
    onConfirmAmmo: (task: TimelineTask, succeededToday: boolean) => Promise<SpecialOpsBootstrap>;
    onAddAccount: () => void;
    onConfirmManualCheck: (accountId: string) => void;
    onConfirmStation: (task: TimelineTask, correction: StationCorrectionInput) => Promise<SpecialOpsBootstrap>;
    onPause: () => void;
    onReload: () => void;
    onRestore: (accountId: string | null) => void;
    pauseTransition: boolean;
};

export function SpecialOpsBlackmarkView({
    accountActionError,
    bootstrap,
    children,
    controlsLocked,
    currentDay,
    error,
    hasActiveRun,
    isActiveRound,
    isNativeShell,
    nowMs,
    onAddAccount,
    onConfirmAmmo,
    onConfirmManualCheck,
    onConfirmStation,
    onPause,
    onReload,
    onRestore,
    pauseTransition,
}: SpecialOpsBlackmarkViewProps) {
    const paused = bootstrap.settings.paused;
    const tasks = bootstrap.schedule.timelineTasks;
    const next = tasks[0] ?? null;
    const spec = buildSpecs(bootstrap, next, nowMs);
    const pauseLabel = paused && pauseTransition
        ? "正在继续"
        : pauseTransition
            ? "正在暂停"
            : isActiveRound && !paused
                ? "当前账号结束后暂停"
                : paused
                    ? "继续"
                    : "暂停";
    const copy = paused
        ? next
            ? `调度已暂停。下一件是${timelineTaskLabel(next)}，然后按任务栏顺序执行。`
            : "调度已暂停。配好账号后点继续，到期项会出现在这里。"
        : next
            ? `正在值班。下一件是${timelineTaskLabel(next)}。`
            : "正在值班。未来 24 小时暂无任务。";

    return (
        <BlackmarkPage
            actions={
                <div className="flex flex-wrap items-center gap-3">
                    <button
                        className="bm-btn inline-flex items-center gap-2"
                        disabled={pauseTransition || (hasActiveRun && !isActiveRound)}
                        onClick={onPause}
                        type="button"
                    >
                        {paused
                            ? <RiPlayLine className="size-4" aria-hidden="true"/>
                            : <RiPauseLine className="size-4" aria-hidden="true"/>}
                        {pauseLabel}
                    </button>
                    <button className="bm-btn-ghost inline-flex items-center gap-2" onClick={onReload} type="button">
                        <RiRefreshLine className="size-4" aria-hidden="true"/>
                        刷新
                    </button>
                </div>
            }
            copy={copy}
            specs={
                <section className="bm-spec-grid grid gap-px sm:grid-cols-2 xl:grid-cols-4">
                    <BlackmarkSpec label={spec.remainingLabel} readout value={spec.remaining}/>
                    <BlackmarkSpec label={spec.nextLabel} value={spec.next}/>
                    <BlackmarkSpec label="启用账号" readout value={spec.enabled}/>
                    <BlackmarkSpec label={spec.checkLabel} value={spec.check} warning={spec.checkWarning}/>
                </section>
            }
            title="特勤处"
        >
            {error ? <div className="bm-alert mt-0" data-tone="error">{error}</div> : null}
            {paused && bootstrap.settings.pausedReason ? (
                <div className="bm-alert mt-4" data-tone="warning">{bootstrap.settings.pausedReason}</div>
            ) : null}

            <section className="px-8 py-16">
                <h2 className="text-2xl font-bold tracking-tight uppercase">24 小时时间轴</h2>
                <p className="bm-muted mt-2 max-w-[60ch] text-sm font-light">
                    到期按账号分桶，交易行排最后。失败行左侧交叉切口。单项判定在行内。
                </p>
                {bootstrap.settings.accounts.length === 0 ? (
                    <div className="mt-8">
                        <p className="bm-copy text-sm font-light">加一个 QQ 账号，选好 WeGame 和游戏，再框选点击点。</p>
                        <button className="bm-btn mt-6" onClick={onAddAccount} type="button">添加账号</button>
                    </div>
                ) : tasks.length === 0 ? (
                    <p className="bm-copy mt-8 text-sm font-light">未来 24 小时暂无任务。点继续后，到期项会出现在这里。</p>
                ) : (
                    <div className="bm-table-shell mt-8 overflow-x-auto">
                        <table className="bm-table">
                            <thead>
                            <tr>
                                <th>时间</th>
                                <th>账号</th>
                                <th>任务</th>
                                <th>状态</th>
                            </tr>
                            </thead>
                            <tbody>
                            {tasks.map((task) => {
                                const station = task.stationKind
                                    ? bootstrap.settings.accounts.find(({id}) => id === task.accountId)?.stations.find(({kind}) => kind === task.stationKind) ?? null
                                    : null;
                                const fail = Boolean(task.manualFailure)
                                    || task.accountStatus === "manualCheckRequired"
                                    || task.accountStatus === "isolated"
                                    || task.accountStatus === "uncertain";
                                const due = task.scheduledAtMs <= nowMs;
                                const status = fail
                                    ? "失败"
                                    : due
                                        ? "到期"
                                        : `${timelineDelayMinutes(task, nowMs)} 分钟后`;
                                return (
                                    <tr data-state={fail ? "fail" : due ? "due" : "next"} key={task.id}>
                                        <td className="strong bm-readout">
                                            {due ? "现在" : shanghaiTimeFormatter.format(task.scheduledAtMs)}
                                        </td>
                                        <td>{task.qqAccount || task.accountId}</td>
                                        <td className="strong">
                                            {timelineTaskLabel(task)}
                                            {task.note ? ` · ${task.note}` : ""}
                                            {task.kind === "marketPurchase" && task.marketStatus
                                                ? ` · 已购买 ${task.marketCompletedCount ?? 0}/${task.marketTargetCount ?? 0}`
                                                : ""}
                                        </td>
                                        <td>
                                            <div>{status}</div>
                                            {task.manualFailure ? (
                                                <div className="bm-muted mt-1 text-xs">{task.manualFailure.step}：{task.manualFailure.message}</div>
                                            ) : null}
                                            {timelineTaskAllowsInlineCorrection(task, station) ? (
                                                <BlackmarkRowCorrection
                                                    disabled={controlsLocked}
                                                    onConfirmAmmo={onConfirmAmmo}
                                                    onConfirmStation={onConfirmStation}
                                                    stationKind={task.stationKind}
                                                    task={task}
                                                />
                                            ) : null}
                                        </td>
                                    </tr>
                                );
                            })}
                            </tbody>
                        </table>
                    </div>
                )}
            </section>

            <section className="px-8 pb-10">
                <div className="flex flex-wrap items-end justify-between gap-4">
                    <div>
                        <h2 className="text-2xl font-bold tracking-tight uppercase">账号</h2>
                        <p className="bm-muted mt-2 max-w-[60ch] text-sm font-light">
                            已人工检查与一键恢复在本页第一击，失败原因写在按钮旁。
                        </p>
                    </div>
                    <button
                        className="bm-btn-ghost"
                        disabled={!bootstrap.settings.accounts.some((account) => accountRestorable(account, currentDay)) || !isNativeShell}
                        onClick={() => onRestore(null)}
                        type="button"
                    >
                        全部一键恢复
                    </button>
                </div>
                <div className="bm-table-shell mt-8 overflow-x-auto">
                    <table className="bm-table">
                        <thead>
                        <tr>
                            <th>账号</th>
                            <th>状态</th>
                            <th>动作</th>
                        </tr>
                        </thead>
                        <tbody>
                        {bootstrap.settings.accounts.map((account, index) => {
                            const needsCheck = account.status !== "ready";
                            return (
                                <tr data-state={needsCheck ? "fail" : undefined} key={account.id}>
                                    <td className="strong">
                                        {index + 1} · {account.qqAccount || account.id}
                                        {account.enabled ? "" : " · 关闭"}
                                    </td>
                                    <td>{accountStatusLabels[account.status]}</td>
                                    <td>
                                        <div className="flex flex-wrap gap-2">
                                            {needsCheck ? (
                                                <button
                                                    className="bm-btn-ghost h-10 px-5"
                                                    disabled={!isNativeShell}
                                                    onClick={() => onConfirmManualCheck(account.id)}
                                                    type="button"
                                                >
                                                    已人工检查
                                                </button>
                                            ) : null}
                                            <button
                                                className="bm-btn-ghost h-10 px-5"
                                                disabled={!accountRestorable(account, currentDay) || !isNativeShell}
                                                onClick={() => onRestore(account.id)}
                                                type="button"
                                            >
                                                一键恢复
                                            </button>
                                        </div>
                                        {accountActionError?.accountId === account.id ? (
                                            <div className="bm-muted mt-2 text-xs">{accountActionError.message}</div>
                                        ) : null}
                                    </td>
                                </tr>
                            );
                        })}
                        </tbody>
                    </table>
                </div>
            </section>

            <section className="px-8 pb-16">
                <h2 className="text-2xl font-bold tracking-tight uppercase">配置</h2>
                <p className="bm-muted mt-2 mb-8 max-w-[60ch] text-sm font-light">
                    校准、利润、限时与交易行仍在本页，不拆子页。
                </p>
                {children}
            </section>
        </BlackmarkPage>
    );
}

function buildSpecs(bootstrap: SpecialOpsBootstrap, next: TimelineTask | null, nowMs: number) {
    const enabled = bootstrap.settings.accounts.filter((account) => account.enabled).length;
    const checkAccount = bootstrap.settings.accounts.find((account) => account.status !== "ready");
    let remaining = "--";
    let remainingLabel = "制作剩余";
    const crafting = bootstrap.settings.accounts
        .flatMap((account) => account.stations.map((station) => ({account, station})))
        .filter((item) => item.station.status === "crafting" && item.station.finishesAtMs != null)
        .sort((a, b) => (a.station.finishesAtMs ?? 0) - (b.station.finishesAtMs ?? 0))[0];
    if (crafting?.station.finishesAtMs) {
        remaining = formatHms(Math.max(0, crafting.station.finishesAtMs - nowMs));
        remainingLabel = `${STATION_LABELS[crafting.station.kind]} 剩余`;
    }
    return {
        remaining,
        remainingLabel,
        next: next ? timelineTaskLabel(next) : "无",
        nextLabel: next ? `下一任务 · ${timelineDelayMinutes(next, nowMs)} 分钟` : "下一任务",
        enabled,
        check: checkAccount ? accountStatusLabels[checkAccount.status] : "就绪",
        checkLabel: checkAccount
            ? `账号 ${checkAccount.qqAccount || checkAccount.id} 需检查`
            : "人工检查",
        checkWarning: Boolean(checkAccount),
    };
}

function formatHms(ms: number): string {
    const total = Math.floor(ms / 1000);
    const h = String(Math.floor(total / 3600)).padStart(2, "0");
    const m = String(Math.floor((total % 3600) / 60)).padStart(2, "0");
    const s = String(total % 60).padStart(2, "0");
    return `${h}:${m}:${s}`;
}

function BlackmarkRowCorrection({
    disabled,
    onConfirmAmmo,
    onConfirmStation,
    stationKind,
    task,
}: {
    disabled: boolean;
    onConfirmAmmo: (task: TimelineTask, succeededToday: boolean) => Promise<SpecialOpsBootstrap>;
    onConfirmStation: (task: TimelineTask, correction: StationCorrectionInput) => Promise<SpecialOpsBootstrap>;
    stationKind: TimelineTask["stationKind"];
    task: TimelineTask;
}) {
    const submitStation = (state: "immediateDue" | "idle") => {
        if (!stationKind) return;
        void onConfirmStation(task, {kind: stationKind, state, remainingMinutes: null});
    };
    return (
        <div className="mt-3 flex flex-wrap gap-2">
            {task.kind === "craft" ? (
                <>
                    <button className="bm-btn-ghost h-10 px-5" disabled={disabled} onClick={() => submitStation("immediateDue")} type="button">立即到期</button>
                    <button
                        className="bm-btn-ghost h-10 px-5"
                        disabled={disabled || !stationKind}
                        onClick={() => stationKind && void onConfirmStation(task, {kind: stationKind, state: "crafting", remainingMinutes: null})}
                        type="button"
                    >
                        正在制作
                    </button>
                    <button className="bm-btn-ghost h-10 px-5" disabled={disabled} onClick={() => submitStation("idle")} type="button">空闲中</button>
                </>
            ) : (
                <>
                    <button className="bm-btn-ghost h-10 px-5" disabled={disabled} onClick={() => void onConfirmAmmo(task, true)} type="button">已兑换</button>
                    <button className="bm-btn-ghost h-10 px-5" disabled={disabled} onClick={() => void onConfirmAmmo(task, false)} type="button">未兑换</button>
                </>
            )}
        </div>
    );
}
