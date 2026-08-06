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

export type InlineStationCorrectionDraft = {
    state: ManualStationState | null;
    hours: string;
    minutes: string;
};

export function createInlineStationCorrectionDraft(
    station: Pick<StationPlan, "finishesAtMs">,
    nowMs: number,
): InlineStationCorrectionDraft {
    const remaining = station.finishesAtMs !== null && station.finishesAtMs > nowMs
        ? Math.ceil((station.finishesAtMs - nowMs) / 60_000)
        : null;
    return {
        state: "crafting",
        hours: remaining === null ? "" : String(Math.floor(remaining / 60)),
        minutes: remaining === null ? "" : String(remaining % 60),
    };
}

export function buildInlineStationCorrection(
    state: ManualStationState,
    hours: string,
    minutes: string,
): Pick<StationCorrectionInput, "state" | "remainingMinutes"> | null {
    if (state !== "crafting") return {state, remainingMinutes: null};
    if (!/^\d+$/.test(hours) || !/^\d+$/.test(minutes)) return null;
    const parsedHours = Number(hours);
    const parsedMinutes = Number(minutes);
    if (!Number.isSafeInteger(parsedHours) || !Number.isSafeInteger(parsedMinutes)
        || parsedMinutes > 59) return null;
    const remainingMinutes = parsedHours * 60 + parsedMinutes;
    if (remainingMinutes < 1 || remainingMinutes > 10_080) return null;
    return {state, remainingMinutes};
}

export function buildTimelineHourSlots(nowMs: number): number[] {
    const hourMs = 60 * 60_000;
    const shanghaiOffsetMs = 8 * hourMs;
    const first = Math.floor((nowMs + shanghaiOffsetMs) / hourMs) * hourMs - shanghaiOffsetMs;
    return Array.from({length: 24}, (_, index) => first + index * hourMs);
}
