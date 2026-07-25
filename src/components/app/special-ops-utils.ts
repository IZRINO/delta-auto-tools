import type {
    AccountPlan,
    CalibrationTemplateTestResult,
    SpecialOpsBootstrap,
    SpecialOpsSettings,
} from "@/components/app/special-ops-types";

export type SpecialOpsSaveRequest = {settings: SpecialOpsSettings; settingsRevision: number};

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
