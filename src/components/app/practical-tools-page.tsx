import {useCallback, useEffect, useState, type MouseEvent} from "react";
import {RiFlashlightLine, RiSaveLine} from "@remixicon/react";

import {Button} from "@/components/ui/button";
import {Input} from "@/components/ui/input";
import {Switch} from "@/components/ui/switch";
import {Tabs, TabsContent, TabsList, TabsTrigger} from "@/components/ui/tabs";
import {HotkeyField, ToolPageFrame} from "@/components/app/app-ui";
import type {PracticalToolsBootstrap, PracticalToolsSettings} from "@/components/app/practical-tools-types";
import type {Point} from "@/components/app/morse-types";
import {MIN_SELECTION_HEIGHT, MIN_SELECTION_WIDTH} from "@/components/app/morse-types";
import {getErrorMessage, getSelectionRect} from "@/components/app/morse-utils";
import {invokeLogged as invoke} from "@/lib/logging";
import {useNativeShell} from "@/hooks/use-native-shell";
import {useProfile} from "@/hooks/use-profile";
import {getSettingsRevision} from "@/components/app/profile-utils";
import {RiMapPinLine} from "@remixicon/react";
import {useBootstrapForm} from "@/hooks/use-bootstrap-form";
import {useAutosave} from "@/hooks/use-autosave";
import {subscribeTauriEvent} from "@/lib/tauri-listener";
import {PRACTICAL_TOOLS_EVENTS} from "@/lib/tauri-events";
import {mergePracticalToolsRegions} from "@/components/app/practical-tools-types";
import {useHotkeyRecorder} from "@/hooks/use-hotkey-recorder";
import {formatTriggerHotkey} from "@/components/app/rapidfire-types";
import type {RegionRect} from "@/components/app/morse-types";

const PRACTICAL_TOOLS_SPEC = {
    getBootstrapCommand: "practical_tools_get_bootstrap",
    saveSettingsCommand: "practical_tools_save_settings",
    settingsToForm: (settings: PracticalToolsSettings) => settings,
    parseSettingsForm: (settings: PracticalToolsSettings) => settings,
};

const defaults: PracticalToolsSettings = {
    enabled: false, autoFullAutoEnabled: false, autoFullAutoToggleKey: "F6", fireKey: "MouseLeft", weaponKeys: ["1", "2", "3", "4", "5"],
    startDelayMs: 80, weaponDetectionDelayMs: 100, pressMs: 30, intervalMs: 30, autoRegion: null, matchThreshold: 0.75, autoIndicatorOpacity: 0.9, autoIndicatorRight: 28, autoIndicatorBottom: 28,
    bombProgressEnabled: false, installRegion: null, bombIconRegion: null, bombTargetColor: "#F5C400", bombColorTolerance: 55, bombDurationSeconds: 30,
    warningSeconds: 5, overlayX: 0, overlayY: 0, overlayWidth: 420, overlayHeight: 28, overlayOpacity: 0.85,
};

function RegionReadout({label, region}: {label: string; region: RegionRect | null}) {
    return <p className="text-sm text-base-content/60" aria-live="polite">
        {label}：{region
            ? `已框选 · X ${region.x} · Y ${region.y} · 宽 ${region.width} · 高 ${region.height}`
            : "未框选"}
    </p>;
}
export function PracticalToolsPage() {
    const isNativeShell = useNativeShell();
    const {
        form, setForm, setBootstrap, isDirty, saveSettings, loading, saving,
        statusMessage: status, setStatusMessage: setStatus, autosaveVersionRef,
    } = useBootstrapForm<PracticalToolsBootstrap, PracticalToolsSettings, PracticalToolsSettings>({
        spec: PRACTICAL_TOOLS_SPEC,
        isNativeShell,
        readyStatusMessage: "已加载，修改自动保存。",
        saveSuccessMessage: "已保存，配置已生效。",
    });
    const settings = form ?? defaults;
    const [activeTab, setActiveTab] = useState("auto");

    useEffect(() => {
        if (!isNativeShell) setForm(defaults);
    }, [isNativeShell, setForm]);

    useAutosave({
        form, isDirty, disabled: !isNativeShell || loading,
        onSave: saveSettings, autosaveVersionRef,
    });

    useEffect(() => {
        if (!isNativeShell) return;
        return subscribeTauriEvent<PracticalToolsBootstrap>(PRACTICAL_TOOLS_EVENTS.regionSelected, (event) => {
            setBootstrap(event.payload);
            setForm((current) => mergePracticalToolsRegions(current, event.payload.settings));
            setStatus("框选成功，识别区域已保存。");
        }, (error) => setStatus(getErrorMessage(error)));
    }, [isNativeShell, setBootstrap, setForm, setStatus]);

    const update = <K extends keyof PracticalToolsSettings>(key: K, value: PracticalToolsSettings[K]) => {
        if (!form) return;
        const next = {...form, [key]: value};
        setForm(next);
        if (isNativeShell && typeof value === "boolean") {
            void saveSettings(next).catch(() => undefined);
        }
    };
    const toggleKeyRecorder = useHotkeyRecorder({
        formatKey: (event) => formatTriggerHotkey(event) || null,
        onCommit: (key) => update("autoFullAutoToggleKey", key),
        onCancel: (draft) => update("autoFullAutoToggleKey", draft),
        onStatusMessage: setStatus,
        recordingActiveMessage: "正在录制自动全自动切换键。失焦取消。",
        keyRecordedMessage: (key) => `切换键已设置为 ${key}`,
    });
    const updateWeaponKey = (index: number, value: string) => setForm((current) => {
        if (!current) return current;
        const weaponKeys = [...current.weaponKeys] as PracticalToolsSettings["weaponKeys"];
        weaponKeys[index] = value;
        return {...current, weaponKeys};
    });
    async function selectRegion(target: "auto" | "install" | "bomb") {
        if (!isNativeShell || !form || loading) return;
        try {
            await saveSettings(form);
            await invoke("practical_tools_begin_region_selection", {target});
        } catch (error) { setStatus(getErrorMessage(error)); }
    }

    const save = async () => {
        if (!isNativeShell || !form) return;
        try {
            await saveSettings(form);
        } catch (error) { setStatus(String(error)); }
    };

    return <ToolPageFrame title="实用小工具" copy="识别触发与输入辅助。默认关闭。" className="w-full min-w-0">
        <div className="col-span-12 w-full min-w-0 space-y-4 justify-self-stretch">
            <div className="flex flex-wrap items-center justify-between gap-3 border-b border-base-300 pb-4">
                <p className="text-sm text-base-content/60">关闭后两个小工具均停止响应。</p>
                <Switch disabled={loading} checked={settings.enabled} onCheckedChange={(value) => update("enabled", value)} aria-label="实用小工具总开关" />
            </div>
            <Tabs value={activeTab} onValueChange={setActiveTab} className="w-full min-w-0">
                <TabsList variant="line" className="w-full justify-start">
                    <TabsTrigger value="auto"><RiFlashlightLine/>自动全自动</TabsTrigger>
                    <TabsTrigger value="bomb">爆破下包进度</TabsTrigger>
                </TabsList>
                <TabsContent value="auto" className="mt-4">
                    <section className="w-full min-w-0 space-y-4">
                        <div className="flex flex-wrap items-center justify-between gap-3 border-b border-base-300 pb-3">
                            <div><h2 className="font-semibold">自动全自动</h2><p className="text-sm text-base-content/60">未识别到全自动图标时，按住开火键才启动模拟连发。</p></div>
                            <label className="flex items-center gap-2 text-sm">启用<Switch checked={settings.autoFullAutoEnabled} onCheckedChange={(value) => update("autoFullAutoEnabled", value)} /></label>
                        </div>
                        <div className="grid min-w-0 grid-cols-[minmax(0,1fr)] gap-4 @container">
                            <div className="flex flex-wrap items-center justify-between gap-2 rounded-field bg-base-200 p-3"><span>全自动图标区域</span><Button size="sm" variant="outline" onClick={() => void selectRegion("auto")} data-icon="inline-start"><RiMapPinLine className="size-4"/>{settings.autoRegion ? "重新框选" : "框选"}</Button></div>
                            <RegionReadout label="全自动图标区域" region={settings.autoRegion}/>
                            <label className="form-control gap-2">
                                <span className="flex items-center justify-between gap-3">
                                    <span className="label-text">图标匹配阈值</span>
                                    <output className="font-mono text-sm tabular-nums">{Math.round(settings.matchThreshold * 100)}%</output>
                                </span>
                                <input
                                    type="range"
                                    className="range range-sm"
                                    min="0"
                                    max="100"
                                    step="1"
                                    value={Math.round(settings.matchThreshold * 100)}
                                    onChange={(event) => update("matchThreshold", Number(event.target.value) / 100)}
                                    aria-label="图标匹配阈值"
                                />
                                <span className="text-xs text-base-content/60">相似度达到阈值才判定为全自动；全自动武器漏识别时可适当降低。</span>
                            </label>
                            <label className="form-control gap-2">
                                <span className="flex items-center justify-between gap-3"><span className="label-text">提示透明程度</span><output className="font-mono text-sm tabular-nums">{Math.round((1 - settings.autoIndicatorOpacity) * 100)}%</output></span>
                                <input type="range" className="range range-sm" min="0" max="95" step="1" value={Math.round((1 - settings.autoIndicatorOpacity) * 100)} onChange={(event) => update("autoIndicatorOpacity", 1 - Number(event.target.value) / 100)} aria-label="自动全自动提示透明程度" />
                            </label>
                            <div className="grid gap-3 @md:grid-cols-2">
                                <label className="form-control"><span className="label-text">距屏幕右侧（px）</span><Input type="number" min="0" max="10000" value={settings.autoIndicatorRight} onChange={(event) => update("autoIndicatorRight", Math.max(0, Math.min(10000, Number(event.target.value))))}/></label>
                                <label className="form-control"><span className="label-text">距屏幕底部（px）</span><Input type="number" min="0" max="10000" value={settings.autoIndicatorBottom} onChange={(event) => update("autoIndicatorBottom", Math.max(0, Math.min(10000, Number(event.target.value))))}/></label>
                            </div>
                            <div className="grid gap-3 @md:grid-cols-2">
                                <label className="form-control"><span className="label-text">开火键</span><Input value={settings.fireKey} onChange={(event) => update("fireKey", event.target.value)} placeholder="MouseLeft / A / - / =" /></label>
                                <div className="form-control"><span className="label-text">强制自动全自动切换键</span><HotkeyField controlsDisabled={loading} hotkey={settings.autoFullAutoToggleKey} id="auto-full-auto-toggle-key" isRecording={toggleKeyRecorder.isRecording} labeled={false} onBeginHotkeyRecording={() => toggleKeyRecorder.beginRecording(settings.autoFullAutoToggleKey)} onHotkeyKeyDown={toggleKeyRecorder.handleKeyDown} onHotkeyRecorderBlur={toggleKeyRecorder.handleBlur} helperText="按一次强制模拟连发，再按一次恢复识图判定。" /></div>
                                <fieldset className="form-control @md:col-span-2"><legend className="label-text mb-2">切枪快捷键</legend><div className="grid grid-cols-2 gap-2 sm:grid-cols-3 lg:grid-cols-5">{settings.weaponKeys.map((key, index) => <label className="form-control" key={index}><span className="label-text text-xs">武器 {index + 1}</span><Input value={key} onChange={(event) => updateWeaponKey(index, event.target.value)} /></label>)}</div></fieldset>
                                <label className="form-control"><span className="label-text">启动延迟（ms）</span><Input type="number" value={settings.startDelayMs} onChange={(event) => update("startDelayMs", Number(event.target.value))} /></label>
                                <label className="form-control"><span className="label-text">切枪后识别延迟（ms）</span><Input type="number" min="0" max="10000" step="10" value={settings.weaponDetectionDelayMs} onChange={(event) => update("weaponDetectionDelayMs", Number(event.target.value))} /></label>
                                <label className="form-control"><span className="label-text">按下时长（ms）</span><Input type="number" value={settings.pressMs} onChange={(event) => update("pressMs", Number(event.target.value))} /></label>
                                <label className="form-control"><span className="label-text">循环间隔（ms）</span><Input type="number" value={settings.intervalMs} onChange={(event) => update("intervalMs", Number(event.target.value))} /></label>
                            </div>
                        </div>
                    </section>
                </TabsContent>
                <TabsContent value="bomb" className="mt-4">
                    <section className="w-full min-w-0 space-y-4">
                        <div className="flex flex-wrap items-center justify-between gap-3 border-b border-base-300 pb-3">
                            <div><h2 className="font-semibold">爆破下包进度可视化</h2><p className="text-sm text-base-content/60">任一区域识别命中即启动倒计时，运行期间暂停识图。</p></div>
                            <label className="flex items-center gap-2 text-sm">启用<Switch checked={settings.bombProgressEnabled} onCheckedChange={(value) => update("bombProgressEnabled", value)} aria-label="爆破进度可视化开关" /></label>
                        </div>
                        <div className="grid min-w-0 grid-cols-[minmax(0,1fr)] gap-4 @container">
                            <div className="grid gap-2 @md:grid-cols-2">
                                <div className="flex flex-wrap items-center justify-between gap-2 rounded-field bg-base-200 p-3"><span>安装成功提示区域</span><Button size="sm" variant="outline" onClick={() => void selectRegion("install")} data-icon="inline-start"><RiMapPinLine className="size-4"/>{settings.installRegion ? "重新框选" : "框选"}</Button></div>
                                <div className="flex flex-wrap items-center justify-between gap-2 rounded-field bg-base-200 p-3"><span>炸弹计时区域（识色）</span><Button size="sm" variant="outline" onClick={() => void selectRegion("bomb")} data-icon="inline-start"><RiMapPinLine className="size-4"/>{settings.bombIconRegion ? "重新框选" : "框选"}</Button></div>
                            </div>
                            <RegionReadout label="安装成功提示区域" region={settings.installRegion}/>
                            <RegionReadout label="炸弹计时区域" region={settings.bombIconRegion}/>
                            <div className="grid gap-3 @md:grid-cols-2">
                                <label className="form-control"><span className="label-text">炸弹图标目标色</span><div className="flex items-center gap-3"><input type="color" className="h-10 w-14 cursor-pointer rounded border border-base-300 bg-base-100 p-1" value={settings.bombTargetColor} onChange={(event) => update("bombTargetColor", event.target.value.toUpperCase())} aria-label="炸弹图标目标色"/><output className="font-mono text-sm">{settings.bombTargetColor}</output></div></label>
                                <label className="form-control"><span className="label-text">颜色容差（0-255）</span><Input type="number" min="0" max="255" value={settings.bombColorTolerance} onChange={(event) => update("bombColorTolerance", Math.max(0, Math.min(255, Number(event.target.value))))}/></label>
                            </div>
                            <div className="grid gap-3 @md:grid-cols-2 @xl:grid-cols-3">
                                <label className="form-control"><span className="label-text">时长（秒）</span><Input type="number" value={settings.bombDurationSeconds} onChange={(event) => update("bombDurationSeconds", Number(event.target.value))} /></label>
                                <label className="form-control"><span className="label-text">提示提前量（秒）</span><Input type="number" value={settings.warningSeconds} onChange={(event) => update("warningSeconds", Number(event.target.value))} /></label>
                                <label className="form-control"><span className="label-text">宽度</span><Input type="number" value={settings.overlayWidth} onChange={(event) => update("overlayWidth", Number(event.target.value))} /></label>
                                <label className="form-control"><span className="label-text">高度</span><Input type="number" value={settings.overlayHeight} onChange={(event) => update("overlayHeight", Number(event.target.value))} /></label>
                                <label className="form-control"><span className="label-text">透明度</span><Input type="number" min="0.05" max="1" step="0.05" value={settings.overlayOpacity} onChange={(event) => update("overlayOpacity", Number(event.target.value))} /></label>
                                <label className="form-control"><span className="label-text">位置 X</span><Input type="number" value={settings.overlayX} onChange={(event) => update("overlayX", Number(event.target.value))} /></label>
                                <label className="form-control"><span className="label-text">位置 Y</span><Input type="number" value={settings.overlayY} onChange={(event) => update("overlayY", Number(event.target.value))} /></label>
                            </div>
                        </div>
                        <Button variant="outline" onClick={() => void invoke<boolean>("practical_tools_preview_bomb_progress").then((started) => setStatus(started ? "进度条预览已启动" : "进度条正在运行")).catch((error: unknown) => setStatus(String(error)))}>预览进度条</Button>
                    </section>
                </TabsContent>
            </Tabs>
            <div className="flex items-center justify-between border-t border-base-300 pt-3"><span className="text-sm text-base-content/60">{status}</span><Button disabled={loading || saving} onClick={() => void save()} data-icon="inline-start"><RiSaveLine className="size-4"/>保存设置</Button></div>
        </div>
    </ToolPageFrame>;
}

type ProgressState = {remainingMs: number; durationMs: number; warningMs: number; opacity: number} | null;
type PracticalToolsDisplayState = {progress: ProgressState; autoFullAutoForced: boolean; autoFullAutoActive: boolean; autoFullAutoSimulating: boolean; autoIndicatorOpacity: number};

export function PracticalToolsOverlay() {
    const [display, setDisplay] = useState<PracticalToolsDisplayState>({progress: null, autoFullAutoForced: false, autoFullAutoActive: false, autoFullAutoSimulating: false, autoIndicatorOpacity: 0.9});
    useEffect(() => {
        let disposed = false;
        const refresh = () => void invoke<PracticalToolsDisplayState>("practical_tools_get_progress")
            .then((value) => { if (!disposed) setDisplay(value); })
            .catch(() => undefined);
        refresh();
        const timer = window.setInterval(refresh, 100);
        return () => { disposed = true; window.clearInterval(timer); };
    }, []);
    const progress = display.progress;
    if (!progress) return null;
    const percent = Math.max(0, Math.min(100, progress.remainingMs / progress.durationMs * 100));
    const warning = progress.remainingMs <= progress.warningMs;
    const warningPosition = Math.max(0, Math.min(100, 100 - (progress.warningMs / progress.durationMs) * 100));
    return <div className="relative flex h-screen w-screen items-center bg-transparent p-1">
        <div className="relative h-full w-full overflow-hidden rounded-md border border-white/25 bg-black/35" style={{opacity: progress.opacity}}>
            <div className={`absolute inset-y-0 right-0 transition-[width,background-color] duration-150 ${warning ? "bg-error" : "bg-success"}`} style={{width: `${percent}%`}} />
            <div className="absolute inset-y-0 z-10 w-[3px] -translate-x-1/2 bg-white shadow-[0_0_8px_1px_rgb(255_255_255_/_75%)]" style={{left: `${warningPosition}%`}} />
            <div className="pointer-events-none absolute inset-x-0 top-0 h-px bg-white/45" />
        </div>
    </div>;
}

export function PracticalToolsAutoIndicator() {
    const [display, setDisplay] = useState<PracticalToolsDisplayState>({
        progress: null,
        autoFullAutoForced: false,
        autoFullAutoActive: false,
        autoFullAutoSimulating: false,
        autoIndicatorOpacity: 0.9,
    });
    useEffect(() => {
        let disposed = false;
        const refresh = () => void invoke<PracticalToolsDisplayState>("practical_tools_get_progress")
            .then((value) => { if (!disposed) setDisplay(value); })
            .catch(() => undefined);
        refresh();
        const timer = window.setInterval(refresh, 250);
        return () => { disposed = true; window.clearInterval(timer); };
    }, []);
    if (!display.autoFullAutoActive) return null;
    const opacity = display.autoIndicatorOpacity;
    return <div className="flex h-screen w-screen items-end justify-end bg-transparent p-1">
        <div className="relative flex h-14 w-full items-center gap-3 overflow-hidden rounded-md border border-white/25 bg-warning px-4 text-black shadow-lg" style={{opacity}}>
            <div className="flex items-center gap-3">
                <RiFlashlightLine className="size-5 shrink-0" aria-hidden="true" />
                <div className="min-w-0 leading-tight">
                    <div className="text-sm font-bold">{display.autoFullAutoForced ? "强制自动全自动" : "自动全自动已启用"}</div>
                    <div className="text-xs opacity-85">{display.autoFullAutoForced || display.autoFullAutoSimulating ? "正在模拟连续开火" : "识图判定为原生全自动"}</div>
                </div>
            </div>
        </div>
    </div>;
}

export function PracticalToolsRegionOverlay() {
    const {bootstrap: profileBootstrap} = useProfile();
    const target = new URLSearchParams(window.location.search).get("target") ?? "";
    const [dragStart, setDragStart] = useState<Point | null>(null);
    const [dragCurrent, setDragCurrent] = useState<Point | null>(null);
    const [committedRect, setCommittedRect] = useState<{x: number; y: number; width: number; height: number} | null>(null);
    const [statusMessage, setStatusMessage] = useState("拖拽鼠标框选识别区域，松开后按 Enter 确认，Esc 取消。");
    const [submitting, setSubmitting] = useState(false);
    const currentRect = dragStart && dragCurrent ? getSelectionRect(dragStart, dragCurrent) : null;
    const displayRect = currentRect ?? committedRect;
    const cancel = useCallback(async () => {
        if (submitting) return;
        setSubmitting(true);
        setStatusMessage("正在取消...");
        try { await invoke("practical_tools_cancel_region_selection"); }
        catch (reason) { setStatusMessage(getErrorMessage(reason)); setSubmitting(false); }
    }, [submitting]);
    const submit = useCallback(async () => {
        if (!committedRect || submitting) return;
        if (committedRect.width <= MIN_SELECTION_WIDTH || committedRect.height <= MIN_SELECTION_HEIGHT) {
            setStatusMessage(`区域太小（${committedRect.width}x${committedRect.height}），请重新框选。`);
            return;
        }
        setSubmitting(true);
        setStatusMessage("正在提交...");
        try {
            await invoke("practical_tools_submit_region_selection", {target, region: committedRect, settingsRevision: getSettingsRevision(profileBootstrap)});
        } catch (reason) { setStatusMessage(getErrorMessage(reason)); setSubmitting(false); }
    }, [committedRect, profileBootstrap, submitting, target]);
    useEffect(() => {
        const key = (event: KeyboardEvent) => {
            if (event.key === "Escape") { event.preventDefault(); void cancel(); }
            if (event.key === "Enter" && committedRect && !submitting) { event.preventDefault(); void submit(); }
        };
        window.addEventListener("keydown", key);
        return () => window.removeEventListener("keydown", key);
    }, [cancel, committedRect, submit, submitting]);
    const handleMouseDown = (event: MouseEvent<HTMLDivElement>) => {
        if (submitting || event.button !== 0) return;
        if (event.target instanceof Element && event.target.closest("button")) return;
        setCommittedRect(null);
        const point = {x: event.clientX, y: event.clientY};
        setDragStart(point); setDragCurrent(point); setStatusMessage("正在框选...");
    };
    const handleMouseMove = (event: MouseEvent<HTMLDivElement>) => {
        if (!dragStart || submitting) return;
        setDragCurrent({x: event.clientX, y: event.clientY});
    };
    const handleMouseUp = () => {
        if (!dragStart || submitting) return;
        const rect = currentRect;
        setDragStart(null); setDragCurrent(null);
        if (!rect || rect.width <= MIN_SELECTION_WIDTH || rect.height <= MIN_SELECTION_HEIGHT) {
            setCommittedRect(null); setStatusMessage("区域太小，请重新框选。"); return;
        }
        setCommittedRect(rect); setStatusMessage("区域已框选，按 Enter 确认或重新拖拽，Esc 取消。");
    };
    return <div className="fixed inset-0 cursor-crosshair select-none text-white" onContextMenu={(event) => {event.preventDefault(); void cancel();}} onMouseDown={handleMouseDown} onMouseMove={handleMouseMove} onMouseUp={handleMouseUp}>
        {displayRect && <div className="pointer-events-none absolute border border-primary bg-primary/16" style={{left: displayRect.x, top: displayRect.y, width: displayRect.width, height: displayRect.height}}/>}
        <div className="pointer-events-none absolute left-6 top-6 max-w-md border border-white/40 bg-base-100/88 px-4 py-4 text-base-content backdrop-blur-md"><h1 className="text-lg font-semibold text-base-content">实用小工具识别区域</h1><p className="mt-2 text-sm text-base-content/60">{statusMessage}</p>{displayRect && <p className="mt-3 border border-base-300 bg-base-200/80 px-3 py-2 font-mono text-xs text-base-content/60">{`X ${displayRect.x} · Y ${displayRect.y} · W ${displayRect.width} · H ${displayRect.height}`}</p>}</div>
        <div className="absolute right-6 top-6 flex items-center gap-2 border border-white/30 bg-base-100/80 px-3 py-3 backdrop-blur-md"><Button disabled={!committedRect || submitting} onClick={() => void submit()} type="button" variant="secondary" data-icon="inline-start">确认</Button><Button disabled={submitting} onClick={() => void cancel()} type="button" variant="secondary" data-icon="inline-start">取消</Button></div>
    </div>;
}
