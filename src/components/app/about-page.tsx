import {useCallback, useEffect, useState} from "react";
import {invoke} from "@tauri-apps/api/core";
import {relaunch} from "@tauri-apps/plugin-process";
import {openUrl} from "@tauri-apps/plugin-opener";
import {RiArrowRightLine, RiDownloadLine, RiGithubLine, RiRestartLine, RiSearchLine,} from "@remixicon/react";
import {toast} from "sonner";

import {Badge} from "@/components/ui/badge";
import {Button} from "@/components/ui/button";
import {Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle,} from "@/components/ui/dialog";
import {DataWell, FieldUnit, StatusMatrix} from "@/components/app/app-ui";
import {DEPENDENCIES} from "@/components/app/about-deps";
import type {AboutBootstrap, UpdateInfo, UpdateProgress} from "@/components/app/about-types";
import {ABOUT_EVENTS, listenEvent} from "@/lib/tauri-events";
import {useNativeShell} from "@/hooks/use-native-shell";
import {cn} from "@/lib/utils";

type AboutDialogProps = {
    open: boolean;
    onOpenChange: (open: boolean) => void;
};

export function AboutDialog({open, onOpenChange}: AboutDialogProps) {
    const isNativeShell = useNativeShell();
    const [bootstrap, setBootstrap] = useState<AboutBootstrap | null>(null);
    const [updateInfo, setUpdateInfo] = useState<UpdateInfo | null>(null);
    const [progress, setProgress] = useState<UpdateProgress | null>(null);
    const [checking, setChecking] = useState(false);

    useEffect(() => {
        if (!open || !isNativeShell) return;
        let disposed = false;
        void invoke<AboutBootstrap>("about_get_bootstrap").then((data) => {
            if (disposed) return;
            setBootstrap(data);
        });
        return () => {
            disposed = true;
        };
    }, [open, isNativeShell]);

    useEffect(() => {
        if (!open || !isNativeShell) return;
        let disposed = false;
        let unlisten: (() => void) | undefined;
        void listenEvent(ABOUT_EVENTS.updateProgress, (event) => {
            if (disposed) return;
            setProgress(event.payload as UpdateProgress);
        }).then((dispose) => {
            unlisten = dispose;
        });
        return () => {
            disposed = true;
            unlisten?.();
        };
    }, [open, isNativeShell]);

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
        <Dialog open={open} onOpenChange={onOpenChange}>
            <DialogContent className="sm:max-w-3xl w-[min(96vw,900px)] max-h-[80vh] overflow-y-auto">
                <DialogHeader>
                    <DialogTitle>
                        {bootstrap ? bootstrap.name.toUpperCase() : "DELTA AUTO TOOLS"} / 关于
                    </DialogTitle>
                    <DialogDescription>
                        软件版本、开源协议与更新信息
                    </DialogDescription>
                </DialogHeader>

                {/* 版本信息 */}
                <div className="grid gap-2 border-2 border-[var(--chalk)] bg-[var(--carbon)] p-3">
                    <div className="flex flex-wrap items-center gap-3">
            <span
                className="font-heading text-[clamp(1.5rem,4vw,3rem)] font-black leading-[0.85] tracking-[-0.06em] text-[var(--chalk)] uppercase">
              {bootstrap?.version ?? "—"}
            </span>
                        <div className="flex flex-wrap gap-2">
                            <Badge variant="secondary">{bootstrap?.target ?? "windows"}</Badge>
                            <Badge variant="outline">Tauri {bootstrap?.tauriVersion ?? "?"}</Badge>
                            <Badge variant="outline">GPLv2+</Badge>
                        </div>
                    </div>
                    <p className="font-mono text-[0.6rem] font-bold tracking-[0.18em] text-[var(--zinc)] uppercase">
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
                <FieldUnit header="[ UPDATE STATUS ]" className="border-2 border-[var(--chalk)]">
                    {!isNativeShell && (
                        <p className="font-mono text-xs font-bold tracking-[0.08em] text-[var(--zinc)] uppercase">
                            更新功能仅在桌面端可用
                        </p>
                    )}
                    {isNativeShell && (
                        <>
                            {statusItems.length > 0 ? (
                                <StatusMatrix items={statusItems} className="mb-3"/>
                            ) : (
                                <p className="mb-3 font-mono text-[0.68rem] font-bold tracking-[0.08em] text-[var(--zinc)] uppercase">
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

                {/* 开源协议 */}
                <FieldUnit header="[ LICENSE / GPLv2+ ]" className="border-2 border-[var(--chalk)]">
                    <DataWell maxHeight="max-h-40" className="text-[var(--zinc)]">
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
                <FieldUnit header="[ ATTRIBUTIONS ]" className="border-2 border-[var(--chalk)]">
                    <div className="grid gap-px border-2 border-[var(--chalk)] bg-[var(--chalk)]">
                        <div
                            className="grid grid-cols-[auto_1fr_auto_auto] items-center gap-x-3 bg-[var(--carbon)] px-3 py-2">
                            <span
                                className="font-mono text-[0.56rem] font-black tracking-[0.18em] text-[var(--amber)] uppercase">KIND</span>
                            <span
                                className="font-mono text-[0.56rem] font-black tracking-[0.18em] text-[var(--amber)] uppercase">NAME</span>
                            <span
                                className="font-mono text-[0.56rem] font-black tracking-[0.18em] text-[var(--amber)] uppercase">LICENSE</span>
                            <span
                                className="font-mono text-[0.56rem] font-black tracking-[0.18em] text-[var(--amber)] uppercase">URL</span>
                        </div>
                        {(bootstrap?.dependencies ?? DEPENDENCIES).map((dep) => (
                            <div key={dep.name}
                                 className="grid grid-cols-[auto_1fr_auto_auto] items-center gap-x-3 bg-[var(--carbon)] px-3 py-1.5 border-b border-[var(--seam)]">
                <span className={cn(
                    "shrink-0 px-1.5 py-0.5 font-mono text-[0.56rem] font-bold tracking-[0.12em] uppercase",
                    dep.kind === "frontend"
                        ? "border border-[var(--amber)] text-[var(--amber)]"
                        : "border border-[var(--zinc)] text-[var(--zinc)]",
                )}>
                  {dep.kind === "frontend" ? "FE" : "RS"}
                </span>
                                <span className="truncate text-sm font-bold tracking-[-0.01em]">{dep.name}</span>
                                <span
                                    className="shrink-0 font-mono text-[0.56rem] font-bold tracking-[0.08em] text-[var(--zinc)] uppercase">{dep.license}</span>
                                <a
                                    href={dep.url}
                                    target="_blank"
                                    rel="noopener noreferrer"
                                    className="shrink-0 text-[var(--amber)] hover:text-[var(--chalk)]"
                                    onClick={(e) => {
                                        if (isNativeShell) {
                                            e.preventDefault();
                                            void openUrl(dep.url);
                                        }
                                    }}
                                >
                                    <RiArrowRightLine className="size-3.5"/>
                                </a>
                            </div>
                        ))}
                    </div>
                </FieldUnit>
            </DialogContent>
        </Dialog>
    );
}
