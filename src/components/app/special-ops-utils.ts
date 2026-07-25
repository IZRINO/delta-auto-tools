import type {
    AccountPlan,
    CalibrationTemplateTestResult,
    LoginRunSnapshot,
    SpecialOpsBootstrap,
    SpecialOpsSettings,
} from "@/components/app/special-ops-types";

export type SpecialOpsSaveRequest = {settings: SpecialOpsSettings; settingsRevision: number};
export type SpecialOpsBootstrapOrderState = {bootstrap: SpecialOpsBootstrap; responseSeq: number};
export type SpecialOpsBootstrapUpdate =
    | {type: "bootstrapResponse"; bootstrap: SpecialOpsBootstrap; requestSeq: number}
    | {type: "runChanged"; snapshot: LoginRunSnapshot};

function latestRunSnapshot(current: LoginRunSnapshot | null, incoming: LoginRunSnapshot | null) {
    if (current === null) return incoming;
    if (incoming === null) return current;
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
        .filter((account) => account.enabled && account.qqAccount.trim() !== "" && account.password.trim() !== "")
        .sort((left, right) => left.order - right.order);
}

export function applyExecutableSelection(current: string, selected: string | null): string {
    return selected ?? current;
}

export function formatCalibrationTemplateTestResult(label: string, result: CalibrationTemplateTestResult): string {
    const [first, second] = result.sampleSimilarities.map((value) => `${(value * 100).toFixed(1)}%`);
    return `${label}：双采样相似度 ${first} / ${second}，${result.passed ? "已通过" : "未通过"}`;
}
