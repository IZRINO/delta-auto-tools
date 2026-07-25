import {type ComponentProps, useEffect, useRef, useState} from "react";
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
    type AmmoTarget,
    type CalibrationEnvironment,
    type CalibrationTarget,
    type LoginRunSnapshot,
    type SpecialOpsBootstrap,
    type SpecialOpsSettings,
    type SpecialOpsStateChanged,
    type StationKind,
    type StationPlan,
} from "@/components/app/special-ops-types";
import {formatRecordedHotkey} from "@/components/app/morse-utils";
import {
    applySpecialOpsBootstrapUpdate,
    applyExecutableSelection,
    eligibleLoginTrialAccounts,
    formatCalibrationTemplateTestResult,
    persistSpecialOpsSaveRequest,
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
        wegameExecutablePath: "",
        gameExecutablePath: "",
        accounts: [],
        activeCalibrationId: null,
        calibrationEnvironments: [],
    },
    schedule: {dueAccounts: [], nextWakeAtMs: null},
    settingsRevision: 0,
    nowMs: Date.now(),
    runSnapshot: null,
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
        password: "",
        enabled: true,
        initialized: false,
        order,
        status: "ready",
        stations: stationKinds.map(createStation),
        ammoTargets: [],
        lastFailure: null,
        loginTrialSignature: null,
    };
}

function createAmmoTarget(order: number): AmmoTarget {
    return {
        id: crypto.randomUUID(),
        name: "",
        enabled: true,
        seasonal: false,
        scrollSteps: 0,
        order,
        lastSuccessDay: null,
        retryCount: 0,
    };
}

function DraftInput({value, onCommit, ...props}: Omit<ComponentProps<typeof Input>, "value" | "onChange"> & {
    value: string;
    onCommit: (value: string) => void;
}) {
    const [draft, setDraft] = useState(value);
    useEffect(() => setDraft(value), [value]);
    return <Input {...props} value={draft} onChange={(event) => setDraft(event.target.value)} onBlur={() => draft !== value && onCommit(draft)}/>;
}

export function SpecialOpsPage() {
    const isNativeShell = useNativeShell();
    const [bootstrap, setBootstrap] = useState(emptyBootstrap);
    const [error, setError] = useState<string | null>(null);
    const [testingTargetKey, setTestingTargetKey] = useState<string | null>(null);
    const [calibrationTestResult, setCalibrationTestResult] = useState<string | null>(null);
    const [selectedAccountId, setSelectedAccountId] = useState<string | null>(null);
    const [hotkeyStatus, setHotkeyStatus] = useState<string | null>(null);
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
        setError(null);
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
    const setPaused = (paused: boolean) => {
        void (async () => {
            try {
                const saved = await flushSettings();
                await requestBootstrap(() => invoke<SpecialOpsBootstrap>("special_ops_set_paused", {
                    paused,
                    settingsRevision: saved.settingsRevision,
                }));
            } catch (cause) {
                if (!disposedRef.current) setError(String(cause));
            }
        })();
    };
    const updateAccount = (account: AccountPlan, patch: Partial<AccountPlan>) => save({
        ...settingsDraftRef.current,
        accounts: settingsDraftRef.current.accounts.map((item) => item.id === account.id ? {...item, ...patch} : item),
    });
    const updateStation = (account: AccountPlan, station: StationPlan, patch: Partial<StationPlan>) => updateAccount(account, {
        stations: account.stations.map((item) => item.kind === station.kind ? {...item, ...patch} : item),
    });
    const updateAmmoTarget = (account: AccountPlan, target: AmmoTarget, patch: Partial<AmmoTarget>) => updateAccount(account, {
        ammoTargets: account.ammoTargets.map((item) => item.id === target.id ? {...item, ...patch} : item),
    });
    const addAmmoTarget = (account: AccountPlan) => updateAccount(account, {
        ammoTargets: [...account.ammoTargets, createAmmoTarget(account.ammoTargets.length)],
    });
    const removeAmmoTarget = (account: AccountPlan, target: AmmoTarget) => updateAccount(account, {
        ammoTargets: account.ammoTargets
            .filter((item) => item.id !== target.id)
            .map((item, order) => ({...item, order})),
    });
    const moveAmmoTarget = (account: AccountPlan, index: number, offset: -1 | 1) => {
        const nextIndex = index + offset;
        if (nextIndex < 0 || nextIndex >= account.ammoTargets.length) return;
        const ammoTargets = [...account.ammoTargets];
        const [moved] = ammoTargets.splice(index, 1);
        ammoTargets.splice(nextIndex, 0, moved);
        updateAccount(account, {ammoTargets: ammoTargets.map((item, order) => ({...item, order}))});
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
    const isLoginTrialRunning = runSnapshot !== null
        && !["succeeded", "failed", "stopped"].includes(runSnapshot.status);
    const selectedAccount = bootstrap.settings.accounts.find(({id}) => id === selectedAccountId) ?? null;
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
        if (!isNativeShell || isLoginTrialRunning) return;
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
        if (!isNativeShell || !selectedAccountId || isLoginTrialRunning) return;
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
    const cancelLoginTrial = async () => {
        if (!isNativeShell || !isLoginTrialRunning) return;
        try {
            const snapshot = await invoke<LoginRunSnapshot>("special_ops_cancel_login_trial");
            applyRunSnapshot(snapshot);
        } catch (cause) {
            setError(String(cause));
        }
    };
    const activeEnvironment = bootstrap.settings.calibrationEnvironments[0];
    const beginCalibration = async (environment: CalibrationEnvironment, targetKey: string) => {
        setCalibrationTestResult(null);
        try {
            const saved = await flushSettings();
            await invoke("special_ops_begin_calibration_selection", {
                environmentId: environment.id,
                targetKey,
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
    const pickReferenceImage = async (environment: CalibrationEnvironment, target: CalibrationTarget) => {
        if (!isNativeShell) return;
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
        if (!isNativeShell) return;
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
                <Switch checked={bootstrap.settings.enabled} onCheckedChange={(enabled) => save({...settingsDraftRef.current, enabled})}/>
                <Button variant="outline" size="sm" onClick={reload}><RiRefreshLine data-icon="inline-start"/>刷新</Button>
                <Button size="sm" onClick={() => setPaused(!bootstrap.settings.paused)}>
                    {bootstrap.settings.paused ? <RiPlayLine data-icon="inline-start"/> : <RiPauseLine data-icon="inline-start"/>}
                    {bootstrap.settings.paused ? "继续" : "暂停"}
                </Button>
            </div>
        </header>

        {error && <div role="alert" className="alert alert-error"><span>{error}</span></div>}

        <section className="card card-border bg-base-100">
            <div className="card-body gap-4">
                <h2 className="card-title">全局配置</h2>
                <div className="grid gap-3 md:grid-cols-2">
                    <fieldset className="fieldset">
                        <legend className="fieldset-legend">WeGame 可执行文件</legend>
                        <div className="flex gap-2">
                            <Input readOnly value={bootstrap.settings.wegameExecutablePath} placeholder="请选择 WeGame.exe"/>
                            <Button disabled={isLoginTrialRunning} size="sm" variant="outline" onClick={() => void pickExecutable("wegameExecutablePath")}><RiFolderOpenLine data-icon="inline-start"/>选择</Button>
                        </div>
                    </fieldset>
                    <fieldset className="fieldset">
                        <legend className="fieldset-legend">游戏可执行文件</legend>
                        <div className="flex gap-2">
                            <Input readOnly value={bootstrap.settings.gameExecutablePath} placeholder="请选择游戏 .exe"/>
                            <Button disabled={isLoginTrialRunning} size="sm" variant="outline" onClick={() => void pickExecutable("gameExecutablePath")}><RiFolderOpenLine data-icon="inline-start"/>选择</Button>
                        </div>
                    </fieldset>
                    <fieldset className="fieldset">
                        <legend className="fieldset-legend">每日兑换时间（Asia/Shanghai）</legend>
                        <DraftInput value={bootstrap.settings.dailyExchangeTime} placeholder="08:00" onCommit={(dailyExchangeTime) => save({...settingsDraftRef.current, dailyExchangeTime})}/>
                    </fieldset>
                    <fieldset className="fieldset">
                        <legend className="fieldset-legend">紧急停止热键</legend>
                        <Button
                            disabled={isLoginTrialRunning}
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

        <section className="card card-border bg-base-100">
            <div className="card-body gap-4">
                <div className="flex flex-wrap items-center justify-between gap-2">
                    <div><h2 className="card-title">单账号登录试运行</h2><p className="text-sm text-base-content/60">仅运行所选账号一次，不执行收取、生产、购买或子弹兑换</p></div>
                    <div className="stat w-auto p-0"><div className="stat-title">待处理账号</div><div className="stat-value text-2xl">{bootstrap.schedule.dueAccounts.length}</div></div>
                </div>
                <div role="alert" className="alert alert-warning alert-soft">
                    <span>启动前先把游戏置顶；运行时不搜索或滚动窗口。校准位置必须与当前显示环境一致。</span>
                </div>
                <div className="grid gap-3 md:grid-cols-[minmax(0,1fr)_auto_auto] md:items-end">
                    <fieldset className="fieldset">
                        <legend className="fieldset-legend">试运行账号</legend>
                        <select
                            className="select select-sm w-full"
                            disabled={eligibleAccounts.length === 0 || isLoginTrialRunning}
                            value={selectedAccountId ?? ""}
                            onChange={(event) => setSelectedAccountId(event.target.value || null)}
                        >
                            {eligibleAccounts.length === 0 && <option value="">无启用且凭据完整的账号</option>}
                            {eligibleAccounts.map((account) => <option key={account.id} value={account.id}>{account.qqAccount}</option>)}
                        </select>
                    </fieldset>
                    <Button disabled={!isNativeShell || !selectedAccountId || isLoginTrialRunning} onClick={() => void startLoginTrial()}>
                        <RiPlayLine data-icon="inline-start"/>运行所选账号一次
                    </Button>
                    <Button disabled={!isLoginTrialRunning} variant="outline" onClick={() => void cancelLoginTrial()}>
                        <RiStopCircleLine data-icon="inline-start"/>取消本次试运行
                    </Button>
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

        <section className="space-y-3">
            <div className="flex items-center justify-between"><h2 className="text-lg font-semibold">账号</h2><Button size="sm" onClick={addAccount}><RiAddLine data-icon="inline-start"/>添加账号</Button></div>
            {bootstrap.settings.accounts.length === 0 && <div className="rounded-box border border-dashed border-base-300 p-8 text-center text-sm text-base-content/60">暂无账号，点击“添加账号”开始配置</div>}
            {bootstrap.settings.accounts.map((account, index) => {
                const due = bootstrap.schedule.dueAccounts.find((item) => item.accountId === account.id);
                return <article key={account.id} className="rounded-box border border-base-300 bg-base-100 p-4">
                    <div className="flex flex-wrap items-center justify-between gap-2">
                        <div><h3 className="font-semibold">账号 {index + 1}</h3><p className="text-xs text-base-content/60">状态：{account.status}</p></div>
                        <div className="flex items-center gap-2 text-sm"><span>启用</span><Switch checked={account.enabled} onCheckedChange={(enabled) => updateAccount(account, {enabled})}/><Button variant="ghost" size="icon-sm" title="删除账号" onClick={() => removeAccount(account)}><RiDeleteBinLine/></Button></div>
                    </div>
                    <div className="mt-3 grid gap-3 md:grid-cols-2">
                        <label className="form-control gap-1"><span className="label-text">QQ 账号</span><DraftInput value={account.qqAccount} onCommit={(qqAccount) => updateAccount(account, {qqAccount})}/></label>
                        <label className="form-control gap-1"><span className="label-text">QQ 密码（明文保存）</span><DraftInput type="password" value={account.password} onCommit={(password) => updateAccount(account, {password})}/></label>
                    </div>
                    <div className="mt-3 grid gap-2 sm:grid-cols-2 lg:grid-cols-4">
                        {account.stations.map((station) => <div key={station.kind} className="rounded-box bg-base-200 p-3">
                            <div className="flex items-center justify-between"><span className="text-sm font-medium">{STATION_LABELS[station.kind]}</span><Switch checked={station.enabled} onCheckedChange={(enabled) => updateStation(account, station, {enabled})}/></div>
                            <DraftInput className="mt-2" value={station.itemName} placeholder="制作物品" disabled={!station.enabled} onCommit={(itemName) => updateStation(account, station, {itemName})}/>
                            <div className="mt-2 grid grid-cols-2 gap-2">
                                <label className="text-xs">小时<DraftInput type="number" min={0} max={168} value={String(Math.floor(station.durationMinutes / 60))} disabled={!station.enabled} onCommit={(hours) => updateStation(account, station, {durationMinutes: Number(hours) * 60 + station.durationMinutes % 60})}/></label>
                                <label className="text-xs">分钟<DraftInput type="number" min={0} max={59} value={String(station.durationMinutes % 60)} disabled={!station.enabled} onCommit={(minutes) => updateStation(account, station, {durationMinutes: Math.floor(station.durationMinutes / 60) * 60 + Number(minutes)})}/></label>
                            </div>
                            <div className="mt-2 text-xs text-base-content/60">{station.status}{due?.stationKinds.includes(station.kind) ? " · 到期" : ""}</div>
                        </div>)}
                    </div>
                    <div className="mt-3 border-t border-base-300 pt-3">
                        <div className="flex flex-wrap items-center justify-between gap-2">
                            <div className="flex items-center gap-2"><RiShieldCheckLine/><h4 className="text-sm font-medium">子弹兑换顺序</h4><span className="text-xs text-base-content/60">{due?.ammoTargetIds.length ?? 0} 个待处理</span></div>
                            <Button size="sm" variant="outline" onClick={() => addAmmoTarget(account)}><RiAddLine data-icon="inline-start"/>添加子弹</Button>
                        </div>
                        {account.ammoTargets.length === 0 ? <p className="mt-2 text-xs text-base-content/60">未配置兑换目标</p> : (
                            <ul className="list mt-2">
                                {account.ammoTargets.map((target, targetIndex) => <li key={target.id} className="list-row items-center gap-2 border-t border-base-300 px-0">
                                    <span className="font-mono text-xs text-base-content/50">{String(targetIndex + 1).padStart(2, "0")}</span>
                                    <div className="list-col-grow grid min-w-0 gap-2 sm:grid-cols-[minmax(10rem,1fr)_7rem_auto_auto] sm:items-end">
                                        <label className="form-control gap-1"><span className="label-text text-xs">子弹类型</span><DraftInput value={target.name} placeholder="例如：5.45×39mm BT" onCommit={(name) => updateAmmoTarget(account, target, {name})}/></label>
                                        <label className="form-control gap-1"><span className="label-text text-xs">相对滚轮步数</span><DraftInput type="number" min={0} step={1} value={String(target.scrollSteps)} onCommit={(value) => updateAmmoTarget(account, target, {scrollSteps: Math.max(0, Math.trunc(Number(value) || 0))})}/></label>
                                        <label className="flex h-9 items-center gap-2 text-xs"><Switch checked={target.seasonal} onCheckedChange={(seasonal) => updateAmmoTarget(account, target, {seasonal})}/>赛季限定</label>
                                        <label className="flex h-9 items-center gap-2 text-xs"><Switch checked={target.enabled} onCheckedChange={(enabled) => updateAmmoTarget(account, target, {enabled})}/>启用</label>
                                    </div>
                                    <div className="join">
                                        <Button className="join-item" disabled={targetIndex === 0} size="icon-sm" title="上移" variant="ghost" onClick={() => moveAmmoTarget(account, targetIndex, -1)}><RiArrowUpLine data-icon="inline-start"/></Button>
                                        <Button className="join-item" disabled={targetIndex === account.ammoTargets.length - 1} size="icon-sm" title="下移" variant="ghost" onClick={() => moveAmmoTarget(account, targetIndex, 1)}><RiArrowDownLine data-icon="inline-start"/></Button>
                                        <Button className="join-item" size="icon-sm" title="删除子弹" variant="ghost" onClick={() => removeAmmoTarget(account, target)}><RiDeleteBinLine data-icon="inline-start"/></Button>
                                    </div>
                                </li>)}
                            </ul>
                        )}
                    </div>
                </article>;
            })}
        </section>

        <section className="space-y-3 rounded-box border border-base-300 bg-base-100 p-4">
            <div className="flex flex-wrap items-center justify-between gap-2">
                <div><h2 className="text-lg font-semibold">点击区域校准</h2><p className="text-xs text-base-content/60">坐标按当前显示环境全局保存，不按账号复制。显示环境变化后重新校准。</p></div>
            </div>
            {calibrationTestResult && <div role="alert" className="alert alert-info"><span>{calibrationTestResult}</span></div>}
            {activeEnvironment && <>
                <div className="overflow-x-auto rounded-box border border-base-300">
                    <table className="table table-sm">
                        <thead><tr><th>步骤</th><th>类型</th><th>坐标</th><th>参考图</th><th className="text-right">操作</th></tr></thead>
                        <tbody>{activeEnvironment.targets.map((target) => <tr key={target.key}>
                            <td><div className="font-medium">{target.label}</div><div className="font-mono text-[11px] text-base-content/50">{target.key}</div>{target.guardAnyOf.length > 0 && <div className="mt-1 text-[11px] text-base-content/60">前置：{target.guardAnyOf.join(" / ")}</div>}</td>
                            <td>{target.kind === "clickPoint" ? "点击点" : target.kind === "inputRegion" ? "输入区域" : target.recognitionMethod === "ocr" ? "OCR 区域" : "模板识别区域"}</td>
                            <td className="font-mono text-xs">{target.rect ? `${target.rect.x}, ${target.rect.y}, ${target.rect.width}×${target.rect.height}` : "未配置"}</td>
                            <td className="max-w-40 truncate text-xs" title={target.referenceImagePath ?? undefined}>
                                {target.recognitionMethod === "template" ? target.referenceImagePath?.split(/[\\/]/).pop() ?? "未上传" : target.recognitionMethod === "ocr" ? "按业务配置比对文本" : "-"}
                            </td>
                            <td className="text-right">
                                <div className="join">
                                    {target.recognitionMethod === "template" && <Button className="join-item" size="sm" variant="outline" onClick={() => void pickReferenceImage(activeEnvironment, target)}><RiFolderOpenLine data-icon="inline-start"/>{target.referenceImagePath ? "替换" : "上传"}</Button>}
                                    {target.recognitionMethod === "template" && target.referenceImagePath && <Button aria-label="清除参考图" className="join-item" size="icon-sm" title="清除参考图" variant="outline" onClick={() => updateCalibrationTarget(activeEnvironment, target, {referenceImagePath: null})}><RiDeleteBinLine data-icon="inline-start"/></Button>}
                                    {target.recognitionMethod && <Button className="join-item" disabled={testingTargetKey === target.key} size="sm" variant="outline" onClick={() => void testCalibrationTarget(activeEnvironment, target)}><RiPlayLine data-icon="inline-start"/>{testingTargetKey === target.key ? "测试中" : "测试"}</Button>}
                                    <Button className="join-item" size="sm" variant={target.rect ? "outline" : "default"} onClick={() => void beginCalibration(activeEnvironment, target.key)}><RiCrosshair2Line data-icon="inline-start"/>{target.rect ? "重新框选" : "框选"}</Button>
                                </div>
                            </td>
                        </tr>)}</tbody>
                    </table>
                </div>
            </>}
        </section>
    </main>;
}
