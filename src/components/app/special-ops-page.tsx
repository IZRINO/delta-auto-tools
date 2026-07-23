import {type ComponentProps, useEffect, useState} from "react";
import {
    RiAddLine,
    RiDeleteBinLine,
    RiPauseLine,
    RiPlayLine,
    RiRefreshLine,
    RiShieldCheckLine,
    RiCrosshair2Line,
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
    type AccountPlan,
    type CalibrationEnvironment,
    type SpecialOpsBootstrap,
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
        accounts: [],
        activeCalibrationId: null,
        calibrationEnvironments: [],
    },
    schedule: {dueAccounts: [], nextWakeAtMs: null},
    settingsRevision: 0,
    nowMs: Date.now(),
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
        wegameId: "",
        enabled: true,
        initialized: false,
        order,
        status: "ready",
        stations: stationKinds.map(createStation),
        ammoTargets: [],
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

    const applyResult = (next: SpecialOpsBootstrap) => {
        setBootstrap(next);
        setError(null);
    };
    const reload = () => {
        if (!isNativeShell) return;
        void invoke<SpecialOpsBootstrap>("special_ops_get_bootstrap").then(applyResult).catch((cause) => setError(String(cause)));
    };
    useEffect(() => {
        reload();
        if (!isNativeShell) return;
        return subscribeTauriEvent<SpecialOpsBootstrap>(SPECIAL_OPS_EVENTS.stateChanged, (event) => applyResult(event.payload));
    }, [isNativeShell]);

    const save = (settings: SpecialOpsBootstrap["settings"]) => void invoke<SpecialOpsBootstrap>(
        "special_ops_save_settings",
        {settingsValue: settings, settingsRevision: bootstrap.settingsRevision},
    ).then(applyResult).catch((cause) => setError(String(cause)));
    const setPaused = (paused: boolean) => void invoke<SpecialOpsBootstrap>(
        "special_ops_set_paused",
        {paused, settingsRevision: bootstrap.settingsRevision},
    ).then(applyResult).catch((cause) => setError(String(cause)));
    const updateAccount = (account: AccountPlan, patch: Partial<AccountPlan>) => save({
        ...bootstrap.settings,
        accounts: bootstrap.settings.accounts.map((item) => item.id === account.id ? {...item, ...patch} : item),
    });
    const updateStation = (account: AccountPlan, station: StationPlan, patch: Partial<StationPlan>) => updateAccount(account, {
        stations: account.stations.map((item) => item.kind === station.kind ? {...item, ...patch} : item),
    });
    const addAccount = () => save({
        ...bootstrap.settings,
        accounts: [...bootstrap.settings.accounts, createAccount(bootstrap.settings.accounts.length)],
    });
    const removeAccount = (account: AccountPlan) => {
        if (!window.confirm(`删除账号 ${account.wegameId || account.qqAccount || "未命名账号"}？`)) return;
        save({...bootstrap.settings, accounts: bootstrap.settings.accounts.filter((item) => item.id !== account.id)});
    };
    const activeEnvironment = bootstrap.settings.calibrationEnvironments.find(
        (item) => item.id === bootstrap.settings.activeCalibrationId,
    ) ?? bootstrap.settings.calibrationEnvironments[0];
    const updateEnvironment = (environment: CalibrationEnvironment, patch: Partial<CalibrationEnvironment>) => save({
        ...bootstrap.settings,
        calibrationEnvironments: bootstrap.settings.calibrationEnvironments.map((item) => item.id === environment.id ? {...item, ...patch} : item),
    });
    const removeEnvironment = (environment: CalibrationEnvironment) => {
        if (bootstrap.settings.calibrationEnvironments.length <= 1) return;
        if (!window.confirm(`删除显示环境“${environment.name}”？`)) return;
        const remaining = bootstrap.settings.calibrationEnvironments.filter((item) => item.id !== environment.id);
        save({...bootstrap.settings, calibrationEnvironments: remaining, activeCalibrationId: remaining[0]?.id ?? null});
    };
    const beginCalibration = (environment: CalibrationEnvironment, targetKey: string) => void invoke(
        "special_ops_begin_calibration_selection",
        {environmentId: environment.id, targetKey, settingsRevision: bootstrap.settingsRevision},
    ).catch((cause) => setError(String(cause)));

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
                    <div className="mt-3 grid gap-3 md:grid-cols-3">
                        <label className="form-control gap-1"><span className="label-text">QQ 账号</span><DraftInput value={account.qqAccount} onCommit={(qqAccount) => updateAccount(account, {qqAccount})}/></label>
                        <label className="form-control gap-1"><span className="label-text">QQ 密码（明文保存）</span><DraftInput type="password" value={account.password} onCommit={(password) => updateAccount(account, {password})}/></label>
                        <label className="form-control gap-1"><span className="label-text">WeGame ID</span><DraftInput value={account.wegameId} onCommit={(wegameId) => updateAccount(account, {wegameId})}/></label>
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
                    <div className="mt-3 flex items-center gap-2 text-xs text-base-content/60"><RiShieldCheckLine/>兑换目标：{due?.ammoTargetIds.length ?? 0} 个待处理</div>
                </article>;
            })}
        </section>

        <section className="space-y-3 rounded-box border border-base-300 bg-base-100 p-4">
            <div className="flex flex-wrap items-center justify-between gap-2">
                <div><h2 className="text-lg font-semibold">显示环境与点击区域校准</h2><p className="text-xs text-base-content/60">坐标不按账号复制。分辨率、DPI 或窗口模式变化后需更新环境并重新校准。</p></div>
            </div>
            {bootstrap.settings.calibrationEnvironments.length > 1 && <div className="flex flex-wrap gap-2">{bootstrap.settings.calibrationEnvironments.map((environment) => <div key={environment.id} className="join"><Button className="join-item" size="sm" variant={environment.id === activeEnvironment?.id ? "default" : "outline"} onClick={() => save({...bootstrap.settings, activeCalibrationId: environment.id})}>{environment.name}</Button><Button className="join-item" size="icon-sm" variant="outline" title={`删除 ${environment.name}`} onClick={() => removeEnvironment(environment)}><RiDeleteBinLine/></Button></div>)}</div>}
            {activeEnvironment && <>
                <div className="grid gap-3 md:grid-cols-5">
                    <label className="form-control gap-1"><span className="label-text">环境名称</span><DraftInput value={activeEnvironment.name} onCommit={(name) => updateEnvironment(activeEnvironment, {name})}/></label>
                    <label className="form-control gap-1"><span className="label-text">显示器</span><DraftInput value={activeEnvironment.monitor} onCommit={(monitor) => updateEnvironment(activeEnvironment, {monitor})}/></label>
                    <label className="form-control gap-1"><span className="label-text">分辨率宽</span><DraftInput type="number" value={String(activeEnvironment.resolutionWidth)} onCommit={(value) => updateEnvironment(activeEnvironment, {resolutionWidth: Number(value)})}/></label>
                    <label className="form-control gap-1"><span className="label-text">分辨率高</span><DraftInput type="number" value={String(activeEnvironment.resolutionHeight)} onCommit={(value) => updateEnvironment(activeEnvironment, {resolutionHeight: Number(value)})}/></label>
                    <label className="form-control gap-1"><span className="label-text">DPI 缩放</span><DraftInput type="number" step="0.25" value={String(activeEnvironment.dpiScale)} onCommit={(value) => updateEnvironment(activeEnvironment, {dpiScale: Number(value)})}/></label>
                    <label className="form-control gap-1 md:col-span-2"><span className="label-text">游戏窗口模式</span><DraftInput value={activeEnvironment.windowMode} onCommit={(windowMode) => updateEnvironment(activeEnvironment, {windowMode})}/></label>
                </div>
                <div className="overflow-x-auto rounded-box border border-base-300">
                    <table className="table table-sm">
                        <thead><tr><th>步骤</th><th>类型</th><th>坐标</th><th className="text-right">操作</th></tr></thead>
                        <tbody>{activeEnvironment.targets.map((target) => <tr key={target.key}><td><div className="font-medium">{target.label}</div><div className="font-mono text-[11px] text-base-content/50">{target.key}</div></td><td>{target.kind === "clickPoint" ? "点击点" : target.kind === "inputRegion" ? "输入区域" : "识别区域"}</td><td className="font-mono text-xs">{target.rect ? `${target.rect.x}, ${target.rect.y}, ${target.rect.width}×${target.rect.height}` : "未配置"}</td><td className="text-right"><Button size="sm" variant={target.rect ? "outline" : "default"} onClick={() => beginCalibration(activeEnvironment, target.key)}><RiCrosshair2Line data-icon="inline-start"/>{target.rect ? "重新框选" : "框选"}</Button></td></tr>)}</tbody>
                    </table>
                </div>
            </>}
        </section>
    </main>;
}
