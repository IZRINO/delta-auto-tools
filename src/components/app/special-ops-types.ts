import {invokeLogged} from "@/lib/logging";

export type StationKind = "technicalCenter" | "workbench" | "pharmacy" | "armorBench";
export type StationStatus = "idle" | "crafting" | "ready" | "uncertain";
export type AccountStatus = "ready" | "needsManualLogin" | "loginFailed" | "uncertain" | "isolated";
export type StationPlan = { kind: StationKind; enabled: boolean; itemName: string; durationMinutes: number; startedAtMs: number | null; finishesAtMs: number | null; status: StationStatus };
export type AmmoTarget = { id: string; name: string; enabled: boolean; seasonal: boolean; scrollSteps: number; order: number; lastSuccessDay: string | null; retryCount: number };
export type AccountFailure = {step: string; message: string; atMs: number};
export type AccountPlan = { id: string; qqAccount: string; enabled: boolean; initialized: boolean; order: number; status: AccountStatus; stations: StationPlan[]; ammoTargets: AmmoTarget[]; lastFailure: AccountFailure | null; loginTrialSignature: string | null };
export type CalibrationRect = {x: number; y: number; width: number; height: number};
export type CalibrationTargetKind = "clickPoint" | "inputRegion" | "recognitionRegion";
export type CalibrationRecognitionMethod = "template" | "ocr";
export type CalibrationTarget = {key: string; label: string; kind: CalibrationTargetKind; rect: CalibrationRect | null; referenceImagePath: string | null; recognitionMethod: CalibrationRecognitionMethod | null; guardAnyOf: string[]; matchThreshold: number; verifiedSignature: string | null; verifiedAtMs: number | null};
export type CalibrationTemplateTestResult = {sampleSimilarities: [number, number]; passed: boolean; verifiedAtMs: number | null};
export type SpecialOpsCalibrationTestArgs = {environmentId: string; targetKey: string; settingsRevision: number};
export type CalibrationEnvironment = {id: string; name: string; monitor: string; resolutionWidth: number; resolutionHeight: number; dpiScale: number; windowMode: string; targets: CalibrationTarget[]};
export type SpecialOpsSettings = { enabled: boolean; paused: boolean; dailyExchangeTime: string; emergencyHotkey: string; wegameExecutablePath: string; gameExecutablePath: string; accounts: AccountPlan[]; activeCalibrationId: string | null; calibrationEnvironments: CalibrationEnvironment[] };
export type LoginRunStatus = "starting" | "waiting" | "countdown" | "inputting" | "succeeded" | "failed" | "stopped";
export type LoginStep = "stopGame" | "stopWeGame" | "startWeGame" | "waitLoginChoice" | "openLoginForm" | "openAccountList" | "scanRememberedAccounts" | "selectRememberedAccount" | "verifySelectedAccount" | "submitLogin" | "waitGameEntry" | "openGameEntry" | "waitLaunchButton" | "launchGame" | "waitGameWindow";
export type LoginRunSnapshot = {runId: number; accountId: string; status: LoginRunStatus; currentStep: LoginStep | null; message: string; countdownSeconds: number | null; startedAtMs: number; updatedAtMs: number};
export type SpecialOpsBootstrap = { settings: SpecialOpsSettings; schedule: { dueAccounts: { accountId: string; stationKinds: StationKind[]; ammoTargetIds: string[] }[]; nextWakeAtMs: number | null }; settingsRevision: number; nowMs: number; runSnapshot: LoginRunSnapshot | null };
export type SpecialOpsStateChanged = {settingsRevision: number; nowMs: number};
export const STATION_LABELS: Record<StationKind, string> = { technicalCenter: "技术中心", workbench: "工作台", pharmacy: "制药台", armorBench: "防具台" };

export function testSpecialOpsCalibrationTarget(
    args: SpecialOpsCalibrationTestArgs,
): Promise<CalibrationTemplateTestResult> {
    return invokeLogged<CalibrationTemplateTestResult>("special_ops_test_calibration_target", args);
}

export async function runLatestSpecialOpsBootstrapRequest<T>(
    token: {current: number},
    request: () => Promise<T>,
    onSuccess: (value: T) => void,
    onError: (error: unknown) => void,
): Promise<void> {
    const requestToken = ++token.current;
    try {
        const value = await request();
        if (requestToken === token.current) onSuccess(value);
    } catch (error) {
        if (requestToken === token.current) onError(error);
    }
}

export function reloadSpecialOpsAfterStateChanged(
    _event: SpecialOpsStateChanged,
    reload: () => void,
): void {
    reload();
}
