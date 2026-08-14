import {invokeLogged} from "@/lib/logging";

export type StationKind = "technicalCenter" | "workbench" | "pharmacy" | "armorBench";
export type StationStatus = "idle" | "crafting" | "ready" | "uncertain";
export type AccountStatus = "ready" | "needsManualLogin" | "loginFailed" | "manualCheckRequired" | "uncertain" | "isolated";
export type StationPlan = { kind: StationKind; enabled: boolean; itemName: string; durationMinutes: number; startedAtMs: number | null; finishesAtMs: number | null; status: StationStatus };
export type AmmoTarget = { id: string; name: string; enabled: boolean; seasonal: boolean; scrollSteps: number; order: number; lastSuccessDay: string | null; retryDay: string | null; retryCount: number; lastFailure: AccountFailure | null };
export type StationBusinessConfig = {kind: StationKind; enabled: boolean; durationMinutes: number; recipeNote: string};
export type ScrollDirection = "up" | "down";
export type AmmoBusinessTarget = {id: string; note: string; enabled: boolean; seasonal: boolean; clickPoint: CalibrationRect | null; scrollDirection: ScrollDirection; scrollSteps: number; order: number; profitRuleId: string | null};
export type AccountRecipePoint = {kind: StationKind; rect: CalibrationRect};
export type MarketBusinessConfig = {
    schemaVersion: number;
    enabled: boolean;
    purchaseCount: number;
    itemNote: string;
    productPoint: CalibrationRect | null;
    maxPrice: number;
};
export type BusinessConfig = {
    stations: StationBusinessConfig[];
    recipePoints: AccountRecipePoint[];
    ammoTargets: AmmoBusinessTarget[];
    market?: MarketBusinessConfig;
};
export type AccountFailure = {step: string; message: string; atMs: number; stationKind: StationKind | null; ammoTargetId: string | null};
export type AccountPlan = { id: string; qqAccount: string; enabled: boolean; initialized: boolean; order: number; status: AccountStatus; independentSettingsEnabled: boolean; independentBusinessConfig: BusinessConfig | null; stations: StationPlan[]; ammoTargets: AmmoTarget[]; lastFailure: AccountFailure | null; loginTrialSignature: string | null; limitedSupply?: LimitedSupplyAccountState; market?: MarketAccountState };
export type CalibrationRect = {x: number; y: number; width: number; height: number};
export type CalibrationTargetKind = "clickPoint" | "inputRegion" | "recognitionRegion";
export type CalibrationRecognitionMethod = "template" | "ocr";
export type CalibrationTarget = {key: string; label: string; kind: CalibrationTargetKind; rect: CalibrationRect | null; referenceImagePath: string | null; recognitionMethod: CalibrationRecognitionMethod | null; guardAnyOf: string[]; matchThreshold: number; verifiedSignature: string | null; verifiedAtMs: number | null};
export type CalibrationTemplateTestResult = {method: "template"; sampleSimilarities: [number, number]; passed: boolean; verifiedAtMs: number | null};
export type CalibrationOcrTestResult = {method: "ocr"; firstTexts: string[]; secondTexts: string[]; passed: boolean};
export type CalibrationTestResult = CalibrationTemplateTestResult | CalibrationOcrTestResult;
export type SpecialOpsCalibrationTestArgs = {environmentId: string; targetKey: string; settingsRevision: number};
export type CalibrationEnvironment = {id: string; name: string; monitor: string; resolutionWidth: number; resolutionHeight: number; dpiScale: number; windowMode: string; targets: CalibrationTarget[]};
export type AmmoProfitRule = {id: string; displayName: string; kkrbMatchName: string; moligodMatchName: string | null; minimumProfit: number};
export type ProfitSource = "kkrb" | "moligod";
export type ProfitAuditOutcome = "qualified" | "belowThreshold" | "targetMissing" | "sourceFailure" | "unconfigured";
export type AmmoProfitAudit = {ruleId: string; day: string; queriedAtMs: number; source: ProfitSource | null; attemptedSources: ProfitSource[]; sourceDataAt: string | null; sourceVersion: string | null; profit: number | null; threshold: number; outcome: ProfitAuditOutcome; detail: string; nextQueryAtMs: number | null};
export type ProfitCutoffSkipReason = "belowThreshold" | "queryUnavailable" | "unconfigured";
export type ProfitCutoffTarget = {accountId: string; targetId: string; ruleId: string | null; skipReason: ProfitCutoffSkipReason | null; decidedAtMs: number | null};
export type ProfitCutoffState = {day: string; targets: ProfitCutoffTarget[]};
export type ProfitFilterSettings = {enabled: boolean; cutoffTime: string; rules: AmmoProfitRule[]; audits: AmmoProfitAudit[]; cutoffState?: ProfitCutoffState | null};
export type ProfitTargetBinding = {accountId: string | null; targetId: string; profitRuleId: string | null};
export type ProfitConfigurationUpdate = {enabled: boolean; cutoffTime: string; rules: AmmoProfitRule[]; bindings: ProfitTargetBinding[]};
export type ProfitCatalogSnapshot = {names: string[]; sourceVersion: string | null; sourceDataAt: string | null};
export type MoligodBindingValidation = {exactName: string; profit: number};
export type ProfitRuntimePhase = "disabled" | "waitingExchange" | "querying" | "waitingNextQuery" | "activeRound" | "cutoffBypass" | "cutoffQuerying" | "waitingCutoffRetry" | "cutoffComplete" | "paused";
export type ProfitTargetKey = {accountId: string; targetId: string};
export type ProfitRuntimeSnapshot = {phase: ProfitRuntimePhase; nextQueryAtMs: number | null; queryAttempt: number | null; qualifiedRuleIds: string[]; currentSessionRuleIds: string[]; activeRoundTargets: ProfitTargetKey[]; lastSummary: string | null; configurationError: string | null};
export type LimitedSupplySettings = {enabled: boolean; researchDelayMs: number; readyTimeoutMs: number; colors: [[number, number, number], [number, number, number]]; colorTolerances: [number, number]};
export type LimitedSupplyOutcome = "pending" | "noHighValue" | "highValue" | "failed";
export type LimitedSupplyAccountState = {cycleId: string | null; outcome: LimitedSupplyOutcome; checkedAtMs: number | null; matchedRegion: number | null; matchedColor: [number, number, number] | null; acknowledged: boolean; lastError: string | null};
export type LimitedSupplyColorTestResult = {firstSampledColor: [number, number, number]; firstMatchedColor: [number, number, number] | null; firstTargetColor: [number, number, number]; firstTolerance: number; firstMatched: boolean; secondMatchedColor: [number, number, number] | null; secondSampledColor: [number, number, number]; secondTargetColor: [number, number, number]; secondTolerance: number; secondMatched: boolean; firstNearestDistance: number; secondNearestDistance: number; passed: boolean};
export type MarketPurchaseSettings = {enabled: boolean; entryDelayMs: number; purchaseCount: number; itemNote: string; windowStartMinute: number; windowEndMinute: number};
export type MarketTaskStatus = "pending" | "running" | "completed" | "priceRecognitionFailed" | "windowClosed";
export type MarketAccountState = {day: string | null; completedCount: number; status: MarketTaskStatus; lastError: string | null};
export type SpecialOpsSettings = { enabled: boolean; paused: boolean; pausedReason?: string | null; dailyExchangeTime: string; emergencyHotkey: string; navigationBeaconDelayMs: number; navigationSpaceDelayMs: number; navigationTabDelayMs: number; navigationSpecialOpsDelayMs: number; ammoSupplyDelayMs: number; ammoTacticalDelayMs: number; craftSpaceDelayMs: number; craftReopenDelayMs: number; craftConfirmPinnedDelayMs: number; wegameExecutablePath: string; gameExecutablePath: string; defaultBusinessConfig: BusinessConfig; profitFilter: ProfitFilterSettings; limitedSupply?: LimitedSupplySettings; marketPurchase?: MarketPurchaseSettings; accounts: AccountPlan[]; activeCalibrationId: string | null; calibrationEnvironments: CalibrationEnvironment[] };
export type ManualStationState = "immediateDue" | "crafting" | "idle";
export type StationCorrectionInput = {kind: StationKind; state: ManualStationState; remainingMinutes: number | null};
export type AmmoCorrectionInput = {targetId: string; succeededToday: boolean};
export type LoginRunStatus = "starting" | "waiting" | "countdown" | "inputting" | "stopping" | "succeeded" | "failed" | "stopped";
export type LoginRunKind = "login" | "navigation" | "craft" | "ammo" | "limitedSupply" | "market" | "round";
export type LoginStep = "stopGame" | "stopWeGame" | "startWeGame" | "waitLoginChoice" | "openLoginForm" | "openAccountList" | "scanRememberedAccounts" | "selectRememberedAccount" | "verifySelectedAccount" | "submitLogin" | "waitGameEntry" | "openGameEntry" | "waitLaunchButton" | "launchGame" | "waitGameWindow" | "waitModeReady" | "openBeaconMode" | "dismissActivityPopup" | "switchLobbyView" | "openSpecialOps" | "waitStationGrid";
export type RoundProgress = {accountIndex: number; accountTotal: number; qqAccount: string; stationKind: StationKind | null; stationIndex: number; stationTotal: number};
export type LoginRunSnapshot = {runId: number; accountId: string; runKind: LoginRunKind; status: LoginRunStatus; currentStep: LoginStep | null; message: string; countdownSeconds: number | null; roundProgress: RoundProgress | null; startedAtMs: number; updatedAtMs: number};
export type TimelineTaskKind = "craft" | "ammo" | "limitedSupplyCheck" | "marketPurchase";
export type TimelineProfitState = "waitingExchange" | "waitingQuery" | "unconfigured" | "qualified" | "activeRound" | "cutoffBypass";
export type TimelineTask = {id: string; accountId: string; qqAccount: string; kind: TimelineTaskKind; stationKind: StationKind | null; ammoTargetId: string | null; note: string; scheduledAtMs: number; overdue: boolean; accountStatus: AccountStatus; profitState?: TimelineProfitState | null; mayExecuteEarlier?: boolean; manualFailure: AccountFailure | null; limitedCycleId?: string | null; marketCompletedCount?: number | null; marketTargetCount?: number | null; marketStatus?: MarketTaskStatus | null};
export type ScheduleSnapshot = {dueAccounts: {accountId: string; stationKinds: StationKind[]; ammoTargetIds: string[]}[]; nextWakeAtMs: number | null; timelineStartMs: number; timelineEndMs: number; timelineTasks: TimelineTask[]};
export type SpecialOpsBootstrap = { settings: SpecialOpsSettings; schedule: ScheduleSnapshot; settingsRevision: number; nowMs: number; runSnapshot: LoginRunSnapshot | null; profitRuntime: ProfitRuntimeSnapshot};
export type SpecialOpsStateChanged = {settingsRevision: number; nowMs: number};
export const STATION_LABELS: Record<StationKind, string> = { technicalCenter: "技术中心", workbench: "工作台", pharmacy: "制药台", armorBench: "防具台" };

export function testSpecialOpsCalibrationTarget(
    args: SpecialOpsCalibrationTestArgs,
): Promise<CalibrationTestResult> {
    return invokeLogged<CalibrationTestResult>("special_ops_test_calibration_target", args);
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
