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
} from "@remixicon/react";

import {Button} from "@/components/ui/button";
import {Input} from "@/components/ui/input";
import {Switch} from "@/components/ui/switch";
import {useNativeShell} from "@/hooks/use-native-shell";
import {invokeLogged as invoke} from "@/lib/logging";
import {SPECIAL_OPS_EVENTS} from "@/lib/tauri-events";
import {subscribeTauriEvent} from "@/lib/tauri-listener";
import {
    STATION_LABELS,
    formatCalibrationTemplateTestResult,
    reloadSpecialOpsAfterStateChanged,
    runLatestSpecialOpsBootstrapRequest,
    testSpecialOpsCalibrationTarget,
    type AccountPlan,
    type AmmoTarget,
    type CalibrationEnvironment,
    type CalibrationTarget,
    type SpecialOpsBootstrap,
    type SpecialOpsStateChanged,
    type StationKind,
    type StationPlan,
} from "@/components/app/special-ops-types";

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
    const bootstrapRequestToken = useRef(0);

    const applyResult = (next: SpecialOpsBootstrap) => {
        setBootstrap(next);
        setError(null);
    };
    const runBootstrapRequest = (request: () => Promise<SpecialOpsBootstrap>) => void runLatestSpecialOpsBootstrapRequest(
        bootstrapRequestToken,
        request,
        applyResult,
        (cause) => setError(String(cause)),
    );
    const reload = () => {
        if (!isNativeShell) return;
        runBootstrapRequest(() => invoke<SpecialOpsBootstrap>("special_ops_get_bootstrap"));
    };
    useEffect(() => {
        reload();
        if (!isNativeShell) return;
        const unsubscribe = subscribeTauriEvent<SpecialOpsStateChanged>(SPECIAL_OPS_EVENTS.stateChanged, (event) => {
            reloadSpecialOpsAfterStateChanged(event.payload, reload);
        });
        return () => {
            bootstrapRequestToken.current += 1;
            unsubscribe();
        };
    }, [isNativeShell]);

    const save = (settings: SpecialOpsBootstrap["settings"]) => runBootstrapRequest(() => invoke<SpecialOpsBootstrap>(
        "special_ops_save_settings",
        {settingsValue: settings, settingsRevision: bootstrap.settingsRevision},
    ));
    const setPaused = (paused: boolean) => runBootstrapRequest(() => invoke<SpecialOpsBootstrap>(
        "special_ops_set_paused",
        {paused, settingsRevision: bootstrap.settingsRevision},
    ));
    const updateAccount = (account: AccountPlan, patch: Partial<AccountPlan>) => save({
        ...bootstrap.settings,
        accounts: bootstrap.settings.accounts.map((item) => item.id === account.id ? {...item, ...patch} : item),
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
        ...bootstrap.settings,
        accounts: [...bootstrap.settings.accounts, createAccount(bootstrap.settings.accounts.length)],
    });
    const removeAccount = (account: AccountPlan) => {
        if (!window.confirm(`删除账号 ${account.qqAccount || "未命名账号"}？`)) return;
        save({...bootstrap.settings, accounts: bootstrap.settings.accounts.filter((item) => item.id !== account.id)});
    };
    const activeEnvironment = bootstrap.settings.calibrationEnvironments[0];
    const beginCalibration = (environment: CalibrationEnvironment, targetKey: string) => {
        setCalibrationTestResult(null);
        void invoke(
            "special_ops_begin_calibration_selection",
            {environmentId: environment.id, targetKey, settingsRevision: bootstrap.settingsRevision},
        ).catch((cause) => setError(String(cause)));
    };
    const updateCalibrationTarget = (
        environment: CalibrationEnvironment,
        target: CalibrationTarget,
        patch: Partial<CalibrationTarget>,
    ) => {
        setCalibrationTestResult(null);
        save({
            ...bootstrap.settings,
            calibrationEnvironments: bootstrap.settings.calibrationEnvironments.map((item) => item.id === environment.id
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
            const result = await testSpecialOpsCalibrationTarget({
                environmentId: environment.id,
                targetKey: target.key,
                settingsRevision: bootstrap.settingsRevision,
            });
            setCalibrationTestResult(formatCalibrationTemplateTestResult(target.label, result));
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
                <Switch checked={bootstrap.settings.enabled} onCheckedChange={(enabled) => save({...bootstrap.settings, enabled})}/>
                <Button variant="outline" size="sm" onClick={reload}><RiRefreshLine data-icon="inline-start"/>刷新</Button>
                <Button size="sm" onClick={() => setPaused(!bootstrap.settings.paused)}>
                    {bootstrap.settings.paused ? <RiPlayLine data-icon="inline-start"/> : <RiPauseLine data-icon="inline-start"/>}
                    {bootstrap.settings.paused ? "继续" : "暂停"}
                </Button>
            </div>
        </header>

        {error && <div className="alert alert-error"><span>{error}</span></div>}

        <section className="grid gap-3 md:grid-cols-3">
            <label className="form-control gap-1"><span className="label-text">每日兑换时间（Asia/Shanghai）</span><DraftInput value={bootstrap.settings.dailyExchangeTime} placeholder="08:00" onCommit={(dailyExchangeTime) => save({...bootstrap.settings, dailyExchangeTime})}/></label>
            <label className="form-control gap-1"><span className="label-text">紧急停止快捷键</span><DraftInput value={bootstrap.settings.emergencyHotkey} placeholder="Ctrl+Shift+F12" onCommit={(emergencyHotkey) => save({...bootstrap.settings, emergencyHotkey})}/></label>
            <div className="stat rounded-box border border-base-300 bg-base-100"><div className="stat-title">待处理账号</div><div className="stat-value text-2xl">{bootstrap.schedule.dueAccounts.length}</div></div>
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
                                    <Button className="join-item" size="sm" variant={target.rect ? "outline" : "default"} onClick={() => beginCalibration(activeEnvironment, target.key)}><RiCrosshair2Line data-icon="inline-start"/>{target.rect ? "重新框选" : "框选"}</Button>
                                </div>
                            </td>
                        </tr>)}</tbody>
                    </table>
                </div>
            </>}
        </section>
    </main>;
}
