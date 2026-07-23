import {useEffect, useState} from "react";
import {RiPauseLine, RiPlayLine, RiRefreshLine, RiShieldCheckLine} from "@remixicon/react";
import {invokeLogged as invoke} from "@/lib/logging";
import {subscribeTauriEvent} from "@/lib/tauri-listener";
import {SPECIAL_OPS_EVENTS} from "@/lib/tauri-events";
import {useNativeShell} from "@/hooks/use-native-shell";
import {Button} from "@/components/ui/button";
import {Input} from "@/components/ui/input";
import {Switch} from "@/components/ui/switch";
import {STATION_LABELS, type AccountPlan, type SpecialOpsBootstrap} from "@/components/app/special-ops-types";

const emptyBootstrap: SpecialOpsBootstrap = {settings: {enabled: true, paused: true, dailyExchangeTime: "08:00", emergencyHotkey: "Ctrl+Shift+F12", accounts: []}, schedule: {dueAccounts: [], nextWakeAtMs: null}, settingsRevision: 0, nowMs: Date.now()};

export function SpecialOpsPage() {
    const isNativeShell = useNativeShell();
    const [bootstrap, setBootstrap] = useState(emptyBootstrap);
    const [error, setError] = useState<string | null>(null);
    const reload = () => { if (isNativeShell) void invoke<SpecialOpsBootstrap>("special_ops_get_bootstrap").then(setBootstrap).catch((cause) => setError(String(cause))); };
    useEffect(() => { reload(); if (!isNativeShell) return; return subscribeTauriEvent<SpecialOpsBootstrap>(SPECIAL_OPS_EVENTS.stateChanged, (event) => setBootstrap(event.payload)); }, [isNativeShell]);
    const save = (settings: SpecialOpsBootstrap["settings"]) => void invoke<SpecialOpsBootstrap>("special_ops_save_settings", {settingsValue: settings, settingsRevision: bootstrap.settingsRevision}).then(setBootstrap).catch((cause) => setError(String(cause)));
    const setPaused = (paused: boolean) => void invoke<SpecialOpsBootstrap>("special_ops_set_paused", {paused, settingsRevision: bootstrap.settingsRevision}).then(setBootstrap).catch((cause) => setError(String(cause)));
    const updateAccount = (account: AccountPlan, patch: Partial<AccountPlan>) => save({...bootstrap.settings, accounts: bootstrap.settings.accounts.map((item) => item.id === account.id ? {...item, ...patch} : item)});
    return <main className="space-y-4">
        <header className="flex flex-wrap items-center justify-between gap-3"><div><h1 className="text-2xl font-semibold">特勤处自动化</h1><p className="text-sm text-base-content/60">配置先行；真实游戏操作层尚未启用。</p></div><div className="flex items-center gap-2"><span className="text-sm">总开关</span><Switch checked={bootstrap.settings.enabled} onCheckedChange={(enabled) => save({...bootstrap.settings, enabled})}/><Button variant="outline" size="sm" onClick={reload}><RiRefreshLine data-icon="inline-start"/>刷新</Button><Button size="sm" onClick={() => setPaused(!bootstrap.settings.paused)}>{bootstrap.settings.paused ? <RiPlayLine data-icon="inline-start"/> : <RiPauseLine data-icon="inline-start"/>}{bootstrap.settings.paused ? "继续" : "暂停"}</Button></div></header>
        {error && <div className="alert alert-error"><span>{error}</span></div>}
        <section className="grid gap-3 md:grid-cols-3"><label className="form-control"><span className="label-text">每日兑换时间（Asia/Shanghai）</span><Input value={bootstrap.settings.dailyExchangeTime} inputMode="numeric" onChange={(event) => save({...bootstrap.settings, dailyExchangeTime: event.target.value})}/></label><div className="stat rounded-box border border-base-300 bg-base-100"><div className="stat-title">待处理账号</div><div className="stat-value text-2xl">{bootstrap.schedule.dueAccounts.length}</div></div><div className="stat rounded-box border border-base-300 bg-base-100"><div className="stat-title">紧急停止</div><div className="stat-value text-base">{bootstrap.settings.emergencyHotkey}</div></div></section>
        <section className="space-y-3">{bootstrap.settings.accounts.length === 0 && <div className="rounded-box border border-dashed border-base-300 p-8 text-center text-sm text-base-content/60">暂无账号配置</div>}{bootstrap.settings.accounts.map((account) => { const due = bootstrap.schedule.dueAccounts.find((item) => item.accountId === account.id); return <article key={account.id} className="rounded-box border border-base-300 bg-base-100 p-4"><div className="flex flex-wrap items-center justify-between gap-2"><div><h2 className="font-semibold">{account.id}</h2><p className="text-xs text-base-content/60">WeGame ID：{account.wegameId || "未配置"} · {account.status}</p></div><div className="flex items-center gap-2 text-sm"><span>启用</span><Switch checked={account.enabled} onCheckedChange={(enabled) => updateAccount(account, {enabled})}/></div></div><div className="mt-3 grid gap-2 sm:grid-cols-2 lg:grid-cols-4">{account.stations.map((station) => <div key={station.kind} className="rounded-box bg-base-200 p-3"><div className="text-sm font-medium">{STATION_LABELS[station.kind]}</div><div className="mt-1 truncate text-xs">{station.enabled ? station.itemName || "未配置物品" : "未启用"}</div><div className="mt-1 text-xs text-base-content/60">{station.status}{due?.stationKinds.includes(station.kind) ? " · 到期" : ""}</div></div>)}</div><div className="mt-3 flex items-center gap-2 text-xs text-base-content/60"><RiShieldCheckLine/>兑换目标：{due?.ammoTargetIds.length ?? 0} 个待处理</div></article>; })}</section>
    </main>;
}
