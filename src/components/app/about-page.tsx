import {useCallback, useEffect, useState} from "react";
import {invokeLogged as invoke} from "@/lib/logging";
import {relaunch} from "@tauri-apps/plugin-process";
import {openUrl} from "@tauri-apps/plugin-opener";
import {RiArrowRightLine, RiDownloadLine, RiGithubLine, RiRestartLine, RiSearchLine,} from "@remixicon/react";
import {toast} from "sonner";

import {Badge} from "@/components/ui/badge";
import {Button} from "@/components/ui/button";
import {Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle,} from "@/components/ui/dialog";
import {RadioGroup, RadioGroupItem} from "@/components/ui/radio-group";
import {DataWell, FieldUnit, StatusMatrix} from "@/components/app/app-ui";
import {DEPENDENCIES} from "@/components/app/about-deps";
import type {AboutBootstrap, UpdateInfo, UpdateProgress} from "@/components/app/about-types";
import {ABOUT_EVENTS} from "@/lib/tauri-events";
import {subscribeTauriEvent} from "@/lib/tauri-listener";
import {useNativeShell} from "@/hooks/use-native-shell";
import {cn} from "@/lib/utils";
import {getLogSettings, setLogSettings as saveLogSettings, type LogSettings, type FrontendLogLevel} from "@/lib/logging";

type AboutPanelProps = {
    /** 是否处于激活状态（设置 Dialog 打开且当前 Tab 为「关于」）。激活时才拉取数据。 */
    active: boolean;
};

/**
 * 关于面板内容：版本 / 更新状态 / 日志级别 / 协议 / 致谢。
 *
 * 原 AboutDialog 的逻辑拆出，可在 SettingsDialog 的「关于」Tab 内复用，
 * 也可被 AboutDialog 薄包装继续作为独立 Dialog 使用。
 */
export function AboutPanel({active}: AboutPanelProps) {
    const isNativeShell = useNativeShell();
    const [bootstrap, setBootstrap] = useState<AboutBootstrap | null>(null);
    const [updateInfo, setUpdateInfo] = useState<UpdateInfo | null>(null);
    const [progress, setProgress] = useState<UpdateProgress | null>(null);
    const [checking, setChecking] = useState(false);
    const [logSettings, setLogSettings] = useState<LogSettings | null>(null);

    useEffect(() => {
        if (!active || !isNativeShell) return;
        let disposed = false;
        void invoke<AboutBootstrap>("about_get_bootstrap").then((data) => {
            if (disposed) return;
            setBootstrap(data);
        });
        return () => {
            disposed = true;
        };
    }, [active, isNativeShell]);

    useEffect(() => {
        if (!active || !isNativeShell) return;
        let disposed = false;
        void getLogSettings().then((settings) => {
            if (disposed) return;
            setLogSettings(settings);
        });
        return () => {
            disposed = true;
        };
    }, [active, isNativeShell]);

    useEffect(() => {
        if (!active || !isNativeShell) return;
        let disposed = false;
        const unlisten = subscribeTauriEvent<UpdateProgress>(ABOUT_EVENTS.updateProgress, (event) => {
            if (disposed) return;
            setProgress(event.payload);
        });
        return () => {
            disposed = true;
            unlisten();
        };
    }, [active, isNativeShell]);

    const handleCheck = useCallback(async () => {
        if (!isNativeShell) return;
        setChecking(true);
        setProgress({phase: "checking"});
        try {
            const info = await invoke<UpdateInfo>("about_check_for_update");
            setUpdateInfo(info);
            setProgress(info.available
                ? {phase: "available", version: info.version ?? "?", notes: info.notes}
                : {phase: "notAvailable"});
        } catch (e) {
            const msg = String(e);
            setUpdateInfo({available: false});
            setProgress({phase: "error", message: msg});
            toast.error(`检查更新失败：${msg}`);
        } finally {
            setChecking(false);
        }
    }, [isNativeShell]);

    const handleDownloadAndInstall = useCallback(async () => {
        if (!isNativeShell) return;
        try {
            await invoke("about_download_and_install");
        } catch (e) {
            const msg = String(e);
            setProgress({phase: "error", message: msg});
            toast.error(`更新失败：${msg}`);
        }
    }, [isNativeShell]);

    const handleRelaunch = useCallback(async () => {
        await relaunch();
    }, []);

    const handleOpenReleases = useCallback(async () => {
        const url = `${bootstrap?.repositoryUrl ?? "https://github.com/IZRINO/delta-auto-tools"}/releases`;
        try {
            await openUrl(url);
        } catch {
            window.open(url, "_blank", "noopener,noreferrer");
        }
    }, [bootstrap]);

    const handleLogLevelChange = useCallback(async (level: string) => {
        if (!logSettings || !isNativeShell) return;
        const newSettings: LogSettings = {...logSettings, globalLevel: level as FrontendLogLevel};
        await saveLogSettings(newSettings);
        setLogSettings(newSettings);
    }, [logSettings, isNativeShell]);
    // 进度状态到 StatusMatrix items
    const statusItems = (() => {
        if (!progress) return [];
        const phase = progress.phase;
        if (phase === "checking") return [{id: "update", state: "warning" as const, label: "检查中..."}];
        if (phase === "notAvailable") return [{id: "update", state: "valid" as const, label: "已是最新"}];
        if (phase === "available") return [{
            id: "update",
            state: "active" as const,
            label: `新版本 ${progress.version}`
        }];
        if (phase === "downloading") {
            const pct = progress.total ? Math.round((progress.downloaded / progress.total) * 100) : null;
            return [{
                id: "update",
                state: "active" as const,
                label: pct != null ? `下载 ${pct}%` : `下载中 ${progress.downloaded}`
            }];
        }
        if (phase === "downloaded") return [{id: "update", state: "active" as const, label: "下载完成"}];
        if (phase === "installing") return [{id: "update", state: "active" as const, label: "安装中..."}];
        if (phase === "installed") return [{id: "update", state: "valid" as const, label: "已安装，待重启"}];
        if (phase === "error") return [{id: "update", state: "error" as const, label: progress.message}];
        return [];
    })();

    return (
        <div className="flex flex-col gap-3">
            {/* 版本信息 */}
            <div className="card card-border bg-base-200 p-4 shadow-none">
                <div className="flex flex-wrap items-center gap-3">
            <span className="text-3xl font-semibold leading-tight text-base-content">
              {bootstrap?.version ?? "—"}
            </span>
                        <div className="flex flex-wrap gap-2">
                            <Badge variant="secondary">{bootstrap?.target ?? "windows"}</Badge>
                            <Badge variant="outline">Tauri {bootstrap?.tauriVersion ?? "?"}</Badge>
                            <Badge variant="outline">GPLv2+</Badge>
                        </div>
                    </div>
                    <p className="text-xs text-base-content/60">
                        {bootstrap?.identifier ?? "org.izrino.delta-auto-tools"} / {bootstrap?.name ?? "delta-auto-tools"}
                    </p>
                    {bootstrap?.repositoryUrl && (
                        <Button variant="outline" size="sm" className="w-fit" onClick={handleOpenReleases}
                                disabled={!isNativeShell}>
                            <RiGithubLine data-icon="inline-start"/>
                            GitHub
                        </Button>
                    )}
                </div>

                {/* 更新状态 */}
                <FieldUnit header="更新状态">
                    {!isNativeShell && (
                        <p className="text-sm text-base-content/60">
                            更新功能仅在桌面端可用
                        </p>
                    )}
                    {isNativeShell && (
                        <>
                            {statusItems.length > 0 ? (
                                <StatusMatrix items={statusItems} className="mb-3"/>
                            ) : (
                                <p className="mb-3 text-sm text-base-content/60">
                                    点击「检查更新」或直接访问 GitHub Release 页面
                                </p>
                            )}
                            <div className="flex flex-wrap gap-2">
                                {progress?.phase !== "installed" && (
                                    <Button
                                        variant="outline"
                                        size="sm"
                                        onClick={handleCheck}
                                        disabled={checking || progress?.phase === "downloading" || progress?.phase === "installing"}
                                    >
                                        <RiSearchLine data-icon="inline-start"/>
                                        检查更新
                                    </Button>
                                )}
                                {(updateInfo?.available || progress?.phase === "available") && (
                                    <Button
                                        size="sm"
                                        onClick={handleDownloadAndInstall}
                                        disabled={progress?.phase === "downloading" || progress?.phase === "installing" || progress?.phase === "installed"}
                                    >
                                        <RiDownloadLine data-icon="inline-start"/>
                                        下载并安装
                                    </Button>
                                )}
                                {progress?.phase === "installed" && (
                                    <Button size="sm" onClick={handleRelaunch}>
                                        <RiRestartLine data-icon="inline-start"/>
                                        立即重启
                                    </Button>
                                )}
                                <Button variant="outline" size="sm" onClick={handleOpenReleases}>
                                    <RiGithubLine data-icon="inline-start"/>
                                    打开 GitHub Release
                                </Button>
                            </div>
                            {progress?.phase === "available" && progress.notes && (
                                <DataWell className="mt-3 max-h-32" maxHeight="max-h-32">
                                    {progress.notes}
                                </DataWell>
                            )}
                        </>
                    )}
                </FieldUnit>
                {/* 日志级别 */}
                <FieldUnit header="日志级别">
                    {!isNativeShell && (
                        <p className="text-sm text-base-content/60">
                            日志设置仅在桌面端可用
                        </p>
                    )}
                    {isNativeShell && logSettings && (
                        <RadioGroup
                            value={logSettings.globalLevel}
                            onValueChange={handleLogLevelChange}
                            className="grid grid-cols-5 gap-2"
                        >
                            {(["error", "warn", "info", "debug", "trace"] as const).map((level) => (
                                <label key={level} htmlFor={`log-${level}`}
                                       className="flex cursor-pointer items-center gap-2 rounded-field border border-base-300 bg-base-100 p-2 hover:border-primary">
                                    <RadioGroupItem value={level} id={`log-${level}`}/>
                                    <span className="font-mono text-xs font-medium">
                                        {level.toUpperCase()}
                                    </span>
                                </label>
                            ))}
                        </RadioGroup>
                    )}
                    <p className="mt-2 text-xs text-base-content/60">
                        当前: {logSettings?.globalLevel?.toUpperCase() ?? "INFO"} · 重启后保持
                    </p>
                </FieldUnit>

                {/* 开源协议 */}
                <FieldUnit header="开源协议">
                    <DataWell maxHeight="max-h-40" className="text-base-content/60">
                        {bootstrap?.license ?? "GPLv2+"}
                    </DataWell>
                    {bootstrap?.licenseUrl && (
                        <div className="mt-2">
                            <Button variant="outline" size="sm" onClick={async () => {
                                try {
                                    await openUrl(bootstrap.licenseUrl);
                                } catch {
                                    window.open(bootstrap.licenseUrl, "_blank");
                                }
                            }} disabled={!isNativeShell}>
                                <RiArrowRightLine data-icon="inline-start"/>
                                查看完整协议
                            </Button>
                        </div>
                    )}
                </FieldUnit>

                {/* 开源库致谢 */}
                <FieldUnit header="开源库致谢">
                    <div className="overflow-x-auto">
                        <table className="table table-sm">
                            <thead>
                            <tr>
                                <th>类型</th>
                                <th>名称</th>
                                <th>协议</th>
                                <th>链接</th>
                            </tr>
                            </thead>
                            <tbody>
                            {(bootstrap?.dependencies ?? DEPENDENCIES).map((dep) => (
                                <tr key={dep.name}>
                                    <td>
                                        <span className={cn(
                                            "badge badge-sm",
                                            dep.kind === "frontend" ? "badge-primary" : "badge-ghost",
                                        )}>
                                            {dep.kind === "frontend" ? "前端" : "运行时"}
                                        </span>
                                    </td>
                                    <td className="max-w-52 truncate font-medium">{dep.name}</td>
                                    <td className="font-mono text-xs text-base-content/60">{dep.license}</td>
                                    <td>
                                        <a
                                            href={dep.url}
                                            target="_blank"
                                            rel="noopener noreferrer"
                                            className="btn btn-ghost btn-square btn-xs text-primary"
                                            onClick={(e) => {
                                                if (isNativeShell) {
                                                    e.preventDefault();
                                                    void openUrl(dep.url);
                                                }
                                            }}
                                        >
                                            <RiArrowRightLine className="size-3.5"/>
                                        </a>
                                    </td>
                                </tr>
                            ))}
                            </tbody>
                        </table>
                    </div>
                </FieldUnit>
        </div>
    );
}

type AboutDialogProps = {
    open: boolean;
    onOpenChange: (open: boolean) => void;
};

/**
 * 关于 Dialog：保留为独立入口（向后兼容），内部直接渲染 AboutPanel。
 *
 * 新的统一设置入口请使用 SettingsDialog（含主题/配置/关于三 Tab）。
 */
export function AboutDialog({open, onOpenChange}: AboutDialogProps) {
    return (
        <Dialog open={open} onOpenChange={onOpenChange}>
            <DialogContent className="sm:max-w-3xl w-[min(96vw,900px)] max-h-[80vh] overflow-y-auto">
                <DialogHeader>
                    <DialogTitle>关于</DialogTitle>
                    <DialogDescription>
                        软件版本、开源协议与更新信息
                    </DialogDescription>
                </DialogHeader>
                <AboutPanel active={open}/>
            </DialogContent>
        </Dialog>
    );
}
