import type {
    AccountPlan,
    AmmoBusinessTarget,
    CalibrationTestResult,
    LoginRunSnapshot,
    ManualStationState,
    SpecialOpsBootstrap,
    SpecialOpsSettings,
    StationCorrectionInput,
    StationPlan,
    TimelineTask,
} from "@/components/app/special-ops-types";
import {STATION_LABELS} from "@/components/app/special-ops-types";

export function limitedColorToHex(color: [number, number, number]): string {
    return `#${color
        .map((channel) => Math.max(0, Math.min(255, Math.trunc(channel))).toString(16).padStart(2, "0"))
        .join("")}`;
}

export function parseLimitedColorHex(value: string): [number, number, number] | null {
    const normalized = value.trim().replace(/^#/, "");
    if (!/^[0-9a-fA-F]{6}$/.test(normalized)) return null;
    return [0, 2, 4].map((offset) => Number.parseInt(normalized.slice(offset, offset + 2), 16)) as [number, number, number];
}

function renumberAmmoTargets(targets: AmmoBusinessTarget[]): AmmoBusinessTarget[] {
    return targets.map((target, order) => ({...target, order}));
}

export function changeAmmoTargetSeasonal(
    targets: AmmoBusinessTarget[],
    id: string,
    seasonal: boolean,
): AmmoBusinessTarget[] {
    const target = targets.find((item) => item.id === id);
    if (!target || target.seasonal === seasonal) return targets;
    const next = targets.filter((item) => item.id !== id);
    const firstSeasonal = next.findIndex((item) => item.seasonal);
    const index = seasonal || firstSeasonal < 0 ? next.length : firstSeasonal;
    next.splice(index, 0, {...target, seasonal});
    return renumberAmmoTargets(next);
}

export function moveAmmoTargetWithinGroup(
    targets: AmmoBusinessTarget[],
    id: string,
    offset: -1 | 1,
): AmmoBusinessTarget[] {
    const index = targets.findIndex((item) => item.id === id);
    const nextIndex = index + offset;
    if (index < 0 || nextIndex < 0 || nextIndex >= targets.length
        || targets[index].seasonal !== targets[nextIndex].seasonal) return targets;
    const next = [...targets];
    [next[index], next[nextIndex]] = [next[nextIndex], next[index]];
    return renumberAmmoTargets(next);
}

export function insertNormalAmmoTarget(
    targets: AmmoBusinessTarget[],
    target: AmmoBusinessTarget,
): AmmoBusinessTarget[] {
    const next = [...targets];
    const firstSeasonal = next.findIndex((item) => item.seasonal);
    next.splice(firstSeasonal < 0 ? next.length : firstSeasonal, 0, {...target, seasonal: false});
    return renumberAmmoTargets(next);
}

export type SpecialOpsSaveRequest = {settings: SpecialOpsSettings; settingsRevision: number};
export type SpecialOpsBootstrapOrderState = {bootstrap: SpecialOpsBootstrap; responseSeq: number};
export type SpecialOpsBootstrapUpdate =
    | {type: "bootstrapResponse"; bootstrap: SpecialOpsBootstrap; requestSeq: number}
    | {type: "runChanged"; snapshot: LoginRunSnapshot};

type SpecialOpsErrorUpdate = {
    updateType: SpecialOpsBootstrapUpdate["type"];
    responseAccepted: boolean;
    completedCurrentDraft: boolean;
    dirtyBefore: boolean;
    revisionChanged: boolean;
};

const terminalRunStatuses = new Set<LoginRunSnapshot["status"]>(["succeeded", "failed", "stopped"]);

export function hasActiveSpecialOpsRun(snapshot: LoginRunSnapshot | null): boolean {
    return snapshot !== null;
}

function latestRunSnapshot(current: LoginRunSnapshot | null, incoming: LoginRunSnapshot | null) {
    if (incoming === null) return current && terminalRunStatuses.has(current.status) ? null : current;
    if (current === null) return incoming;
    if (current.runId > incoming.runId) return current;
    if (current.runId === incoming.runId && current.updatedAtMs > incoming.updatedAtMs) return current;
    return incoming;
}

export function mergeSpecialOpsBootstrap(
    current: SpecialOpsBootstrap,
    incoming: SpecialOpsBootstrap,
): SpecialOpsBootstrap {
    if (incoming.settingsRevision < current.settingsRevision) return current;

    const runSnapshot = latestRunSnapshot(current.runSnapshot, incoming.runSnapshot);
    return runSnapshot === incoming.runSnapshot ? incoming : {...incoming, runSnapshot};
}

export function applySpecialOpsBootstrapUpdate(
    current: SpecialOpsBootstrapOrderState,
    update: SpecialOpsBootstrapUpdate,
): SpecialOpsBootstrapOrderState {
    if (update.type === "runChanged") {
        return {
            ...current,
            bootstrap: mergeSpecialOpsBootstrap(current.bootstrap, {
                ...current.bootstrap,
                runSnapshot: update.snapshot,
            }),
        };
    }

    const incoming = update.bootstrap;
    const revisionDelta = incoming.settingsRevision - current.bootstrap.settingsRevision;
    if (revisionDelta < 0) return current;
    if (revisionDelta === 0 && update.requestSeq < current.responseSeq) {
        const runSnapshot = latestRunSnapshot(current.bootstrap.runSnapshot, incoming.runSnapshot);
        return runSnapshot === current.bootstrap.runSnapshot
            ? current
            : {...current, bootstrap: {...current.bootstrap, runSnapshot}};
    }

    return {
        bootstrap: mergeSpecialOpsBootstrap(current.bootstrap, incoming),
        responseSeq: update.requestSeq,
    };
}

export function specialOpsErrorAfterUpdate(
    currentError: string | null,
    update: SpecialOpsErrorUpdate,
): string | null {
    if (update.dirtyBefore && update.revisionChanged && !update.completedCurrentDraft) {
        return "运行状态已更新，未保存的编辑已被放弃，请重新检查";
    }
    if (update.updateType === "bootstrapResponse" && update.responseAccepted
        && (update.completedCurrentDraft || !update.dirtyBefore)) {
        return null;
    }
    return currentError;
}

export async function persistSpecialOpsSaveRequest(
    request: SpecialOpsSaveRequest,
    save: (request: SpecialOpsSaveRequest) => Promise<SpecialOpsBootstrap>,
    reload: () => void,
): Promise<SpecialOpsBootstrap> {
    try {
        return await save(request);
    } catch (error) {
        if (String(error).includes("配置保存已陈旧")) reload();
        throw error;
    }
}

export function eligibleLoginTrialAccounts(accounts: AccountPlan[]): AccountPlan[] {
    return accounts
        .filter((account) => account.enabled && /^\d+$/.test(account.qqAccount.trim()))
        .sort((left, right) => left.order - right.order);
}

export function applyExecutableSelection(current: string, selected: string | null): string {
    return selected ?? current;
}

export function parseNavigationDelayMs(value: string): number | null {
    if (!/^\d+$/.test(value)) return null;
    const parsed = Number(value);
    return Number.isSafeInteger(parsed) && parsed <= 60_000 ? parsed : null;
}

export function formatCalibrationTemplateTestResult(label: string, result: CalibrationTestResult): string {
    if (result.method === "ocr") {
        const formatTexts = (texts: string[]) => texts.length > 0 ? texts.join("、") : "未识别到纯数字账号";
        return `${label}：OCR 双采样 ${formatTexts(result.firstTexts)} / ${formatTexts(result.secondTexts)}，${result.passed ? "已通过" : "未通过"}`;
    }
    const [first, second] = result.sampleSimilarities.map((value) => `${(value * 100).toFixed(1)}%`);
    return `${label}：双采样相似度 ${first} / ${second}，${result.passed ? "已通过" : "未通过"}`;
}

export type TimelineTaskGroup = {anchorAtMs: number; tasks: TimelineTask[]};

export function groupTimelineTasks(tasks: TimelineTask[], windowMs = 10 * 60_000): TimelineTaskGroup[] {
    const sorted = [...tasks].sort((left, right) => left.scheduledAtMs - right.scheduledAtMs || left.id.localeCompare(right.id));
    const groups: TimelineTaskGroup[] = [];
    for (const task of sorted) {
        const current = groups[groups.length - 1];
        if (current && task.scheduledAtMs - current.anchorAtMs < windowMs) current.tasks.push(task);
        else groups.push({anchorAtMs: task.scheduledAtMs, tasks: [task]});
    }
    return groups;
}

export function timelineDelayMinutes(task: TimelineTask, nowMs: number): number {
    return Math.max(0, Math.ceil((task.scheduledAtMs - nowMs) / 60_000));
}

export function timelineTaskLabel(task: Pick<TimelineTask, "kind" | "stationKind">): string {
    if (task.kind === "craft" && task.stationKind) return STATION_LABELS[task.stationKind];
    if (task.kind === "ammo") return "子弹兑换";
    if (task.kind === "limitedSupplyCheck") return "限时商品检查";
    return "交易行购买";
}

export type InlineStationCorrectionDraft = {
    state: ManualStationState | null;
    hours: string;
    minutes: string;
};

export function createStationRemainingTimeDraft(
    station: Pick<StationPlan, "finishesAtMs">,
    nowMs: number,
): Pick<InlineStationCorrectionDraft, "hours" | "minutes"> {
    const remaining = station.finishesAtMs !== null && station.finishesAtMs > nowMs
        ? Math.ceil((station.finishesAtMs - nowMs) / 60_000)
        : null;
    if (remaining === null || remaining > 10_080) return {hours: "", minutes: ""};
    return {
        hours: String(Math.floor(remaining / 60)),
        minutes: String(remaining % 60),
    };
}

export function createInlineStationCorrectionDraft(
    station: Pick<StationPlan, "finishesAtMs">,
    nowMs: number,
): InlineStationCorrectionDraft {
    const remaining = createStationRemainingTimeDraft(station, nowMs);
    return {
        state: "crafting",
        ...remaining,
    };
}

/// 时间戳 -> Asia/Shanghai 的 `YYYY-MM-DD`。
/// 对齐 Rust `local_day_and_minute` 的固定 UTC+8 偏移，不跟随宿主机时区。
export function shanghaiDay(atMs: number): string {
    const shifted = new Date(atMs + 8 * 60 * 60 * 1000);
    const year = shifted.getUTCFullYear();
    const month = String(shifted.getUTCMonth() + 1).padStart(2, "0");
    const day = String(shifted.getUTCDate()).padStart(2, "0");
    return `${year}-${month}-${day}`;
}

/// 账号是否存在可被一键恢复的异常残留。
/// 与后端 `restore_account_state` 的 `changed` 判定保持一致，避免按钮点下去只拿到「没有需要恢复的异常状态」。
/// 当天已兑换成功的子弹目标也算可恢复项：后端会清当天 `lastSuccessDay` 让目标回到未兑换。
export function accountRestorable(account: AccountPlan, currentDay: string): boolean {
    if (account.status !== "ready" || account.lastFailure) return true;
    if (account.stations.some((station) => station.status === "uncertain")) return true;
    if (account.ammoTargets.some((target) => target.lastFailure
        || target.retryCount > 0
        || target.lastSuccessDay === currentDay)) return true;
    return account.limitedSupply?.outcome === "failed";
}

/// 任务栏单项人工判定的显示条件。
/// 登录环节卡住的账号只能在账号页处理（后端也会拒绝单项判定）；
/// 其余情况下带定位失败、Uncertain 制作台、或账号处于需人工验证都要给出入口。
export function timelineTaskAllowsInlineCorrection(
    task: Pick<TimelineTask, "kind" | "accountStatus" | "manualFailure">,
    station: Pick<StationPlan, "status"> | null,
): boolean {
    if (task.kind !== "craft" && task.kind !== "ammo") return false;
    if (task.accountStatus === "needsManualLogin" || task.accountStatus === "loginFailed") return false;
    if (task.manualFailure) return true;
    if (task.kind === "craft") return station?.status === "uncertain";
    return task.accountStatus === "manualCheckRequired";
}

export function buildInlineStationCorrection(
    state: ManualStationState,
    hours: string,
    minutes: string,
): Pick<StationCorrectionInput, "state" | "remainingMinutes"> | null {
    if (state !== "crafting") return {state, remainingMinutes: null};
    // 留空或 0 表示继承异常前的存量剩余时间，由后端读 finishesAtMs 还原；
    // 后端没有可继承值时才报错，前端不再直接把提交按钮锁死。
    const normalizedHours = hours.trim() === "" ? "0" : hours.trim();
    const normalizedMinutes = minutes.trim() === "" ? "0" : minutes.trim();
    if (!/^\d+$/.test(normalizedHours) || !/^\d+$/.test(normalizedMinutes)) return null;
    const parsedHours = Number(normalizedHours);
    const parsedMinutes = Number(normalizedMinutes);
    if (!Number.isSafeInteger(parsedHours) || !Number.isSafeInteger(parsedMinutes)
        || parsedMinutes > 59) return null;
    const remainingMinutes = parsedHours * 60 + parsedMinutes;
    if (remainingMinutes === 0) return {state, remainingMinutes: null};
    if (remainingMinutes > 10_080) return null;
    return {state, remainingMinutes};
}

export function buildTimelineHourSlots(nowMs: number): number[] {
    const hourMs = 60 * 60_000;
    const shanghaiOffsetMs = 8 * hourMs;
    const first = Math.floor((nowMs + shanghaiOffsetMs) / hourMs) * hourMs - shanghaiOffsetMs;
    return Array.from({length: 24}, (_, index) => first + index * hourMs);
}
