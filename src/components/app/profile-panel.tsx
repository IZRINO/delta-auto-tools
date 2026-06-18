import {useEffect, useMemo, useState} from "react";
import {
    RiAddLine,
    RiCheckLine,
    RiDeleteBinLine,
    RiEditLine,
    RiRefreshLine,
} from "@remixicon/react";

import {Button} from "@/components/ui/button";
import {Input} from "@/components/ui/input";
import {FieldUnit} from "@/components/app/app-ui";
import {cn} from "@/lib/utils";

import {useProfile} from "@/hooks/use-profile";
import type {Profile} from "@/components/app/profile-types";
import {
    countIncludedTools,
    formatProfileTimestamp,
    isActiveProfile,
    validateProfileName,
} from "@/components/app/profile-utils";

/**
 * Profile 面板：配置列表 + 新建 + 切换 + 重命名 + 删除。
 *
 * 切换 Profile 时由 useProfile 触发 reloadNonce 自增，
 * AppShell 据此重挂载工具页（清挂起 autosave + 重拉新配置）。
 */
export function ProfilePanel() {
    const {
        bootstrap,
        loading,
        error,
        saveCurrentAs,
        switchProfile,
        deleteProfile,
        renameProfile,
    } = useProfile();

    const [newName, setNewName] = useState("");
    const [nameError, setNameError] = useState<string | null>(null);
    const [renamingId, setRenamingId] = useState<string | null>(null);
    const [renameValue, setRenameValue] = useState("");

    // 切换中状态：防止重复点击
    const [switchingId, setSwitchingId] = useState<string | null>(null);

    useEffect(() => {
        if (nameError) {
            const t = setTimeout(() => setNameError(null), 2500);
            return () => clearTimeout(t);
        }
    }, [nameError]);

    const profiles = useMemo(() => bootstrap?.profiles ?? [], [bootstrap]);

    const handleCreate = async () => {
        const err = validateProfileName(newName);
        if (err) {
            setNameError(err);
            return;
        }
        try {
            await saveCurrentAs(newName.trim());
            setNewName("");
        } catch (e) {
            setNameError(String(e));
        }
    };

    const handleSwitch = async (id: string) => {
        setSwitchingId(id);
        try {
            await switchProfile(id);
        } catch {
            // 错误已由 Context 捕获并写入 error
        } finally {
            setSwitchingId(null);
        }
    };

    const handleDelete = async (id: string) => {
        if (!confirm("确定删除该配置？此操作不可撤销。")) return;
        try {
            await deleteProfile(id);
        } catch {
            // 错误已由 Context 捕获
        }
    };

    const handleStartRename = (profile: Profile) => {
        setRenamingId(profile.id);
        setRenameValue(profile.name);
    };

    const handleCommitRename = async () => {
        if (!renamingId) return;
        const err = validateProfileName(renameValue);
        if (err) {
            setNameError(err);
            return;
        }
        try {
            await renameProfile(renamingId, renameValue.trim());
            setRenamingId(null);
        } catch (e) {
            setNameError(String(e));
        }
    };

    const handleCancelRename = () => {
        setRenamingId(null);
        setRenameValue("");
        setNameError(null);
    };

    if (loading) {
        return (
            <div className="flex min-h-[200px] items-center justify-center font-mono text-xs text-[var(--zinc)]">
                [ 正在加载配置列表... ]
            </div>
        );
    }

    if (error) {
        return (
            <div className="border-2 border-[var(--alert-red)] bg-[var(--alert-red)]/10 px-3 py-2 font-mono text-xs text-[var(--alert-red)]">
                配置加载失败：{error}
            </div>
        );
    }

    if (!bootstrap) {
        return (
            <div className="font-mono text-xs text-[var(--zinc)]">
                浏览器预览模式不支持配置管理，请在桌面应用内使用。
            </div>
        );
    }

    return (
        <div className="flex flex-col gap-4">
            {/* 新建配置 */}
            <FieldUnit header="[ NEW / 保存当前为配置 ]">
                <div className="flex flex-col gap-2 sm:flex-row sm:items-start">
                    <div className="flex-1">
                        <Input
                            value={newName}
                            onChange={(e) => setNewName(e.target.value)}
                            onKeyDown={(e) => {
                                if (e.key === "Enter") void handleCreate();
                            }}
                            placeholder="配置名称（如：PVE / 竞技 / 日常）"
                            className="h-9"
                            spellCheck={false}
                        />
                        {nameError ? (
                            <p className="mt-1 font-mono text-[0.58rem] text-[var(--alert-red)]">
                                {nameError}
                            </p>
                        ) : (
                            <p className="mt-1 font-mono text-[0.58rem] text-[var(--zinc)]">
                                将打包当前摩斯 / 计时器 / 计数器 / 连发器 / 音频的全部设置。
                            </p>
                        )}
                    </div>
                    <Button
                        onClick={handleCreate}
                        className="h-9 shrink-0 border-2 border-[var(--chalk)] bg-[var(--chalk)] text-[var(--carbon)] hover:bg-[var(--chalk)]/90"
                    >
                        <RiAddLine className="size-4" data-icon="inline-start" aria-hidden="true"/>
                        保存当前为配置
                    </Button>
                </div>
            </FieldUnit>

            {/* 配置列表 */}
            <FieldUnit header={`[ PROFILES / 已保存配置 · ${profiles.length} ]`}>
                {profiles.length === 0 ? (
                    <div className="flex flex-col items-center justify-center gap-2 py-8 text-center">
                        <p className="font-mono text-xs text-[var(--zinc)]">
                            [ 暂无已保存配置 ]
                        </p>
                        <p className="font-mono text-[0.58rem] text-[var(--dust)]">
                            调整好各工具设置后，点击上方「保存当前为配置」即可创建快照。
                        </p>
                    </div>
                ) : (
                    <div className="flex flex-col gap-2">
                        {profiles.map((profile) => {
                            const active = isActiveProfile(bootstrap, profile.id);
                            const isRenaming = renamingId === profile.id;
                            const isSwitching = switchingId === profile.id;
                            return (
                                <div
                                    key={profile.id}
                                    className={cn(
                                        "grid grid-cols-[minmax(0,1fr)_auto] items-center gap-3 border-2 p-3 transition-colors",
                                        active
                                            ? "border-[var(--amber)] bg-[var(--amber)]/10"
                                            : "border-[var(--chalk)] hover:border-[var(--zinc)] hover:bg-[var(--slate)]",
                                    )}
                                >
                                    <div className="min-w-0">
                                        {isRenaming ? (
                                            <div className="flex items-center gap-2">
                                                <Input
                                                    value={renameValue}
                                                    onChange={(e) => setRenameValue(e.target.value)}
                                                    onKeyDown={(e) => {
                                                        if (e.key === "Enter") void handleCommitRename();
                                                        if (e.key === "Escape") handleCancelRename();
                                                    }}
                                                    autoFocus
                                                    className="h-7 flex-1"
                                                    spellCheck={false}
                                                />
                                                <Button
                                                    size="icon"
                                                    variant="outline"
                                                    className="size-7 border-2 border-[var(--chalk)]"
                                                    onClick={() => void handleCommitRename()}
                                                    aria-label="确认重命名"
                                                >
                                                    <RiCheckLine className="size-3.5" aria-hidden="true"/>
                                                </Button>
                                            </div>
                                        ) : (
                                            <>
                                                <p className="truncate text-sm font-black tracking-tight">
                                                    {profile.name}
                                                </p>
                                                <p className="mt-0.5 truncate font-mono text-[0.58rem] text-[var(--zinc)]">
                                                    {formatProfileTimestamp(profile.createdAt)} ·
                                                    含 {countIncludedTools(profile)} 个工具
                                                </p>
                                            </>
                                        )}
                                    </div>
                                    <div className="flex shrink-0 items-center gap-1.5">
                                        {active ? (
                                            <span className="flex items-center gap-1 border-2 border-[var(--amber)] bg-[var(--amber)]/10 px-2 py-1 font-mono text-[0.58rem] font-black tracking-[0.12em] text-[var(--amber)] uppercase">
                                                <RiCheckLine className="size-3" aria-hidden="true"/>
                                                当前
                                            </span>
                                        ) : (
                                            <>
                                                <Button
                                                    size="sm"
                                                    variant="outline"
                                                    className="h-7 border-2 border-[var(--chalk)] px-2 text-xs"
                                                    onClick={() => void handleSwitch(profile.id)}
                                                    disabled={isSwitching}
                                                >
                                                    <RiRefreshLine
                                                        className="size-3.5"
                                                        data-icon="inline-start"
                                                        aria-hidden="true"
                                                    />
                                                    {isSwitching ? "切换中..." : "切换"}
                                                </Button>
                                                <Button
                                                    size="icon"
                                                    variant="outline"
                                                    className="size-7 border-2 border-[var(--chalk)]"
                                                    onClick={() => handleStartRename(profile)}
                                                    aria-label="重命名"
                                                >
                                                    <RiEditLine className="size-3.5" aria-hidden="true"/>
                                                </Button>
                                                <Button
                                                    size="icon"
                                                    variant="outline"
                                                    className="size-7 border-2 border-[var(--alert-red)] text-[var(--alert-red)] hover:bg-[var(--alert-red)]/10"
                                                    onClick={() => void handleDelete(profile.id)}
                                                    aria-label="删除"
                                                >
                                                    <RiDeleteBinLine className="size-3.5" aria-hidden="true"/>
                                                </Button>
                                            </>
                                        )}
                                        {active && isRenaming ? (
                                            <Button
                                                size="icon"
                                                variant="outline"
                                                className="size-7 border-2 border-[var(--chalk)]"
                                                onClick={handleCancelRename}
                                                aria-label="取消重命名"
                                            >
                                                ×
                                            </Button>
                                        ) : null}
                                        {active && !isRenaming ? (
                                            <Button
                                                size="icon"
                                                variant="outline"
                                                className="size-7 border-2 border-[var(--chalk)]"
                                                onClick={() => handleStartRename(profile)}
                                                aria-label="重命名"
                                            >
                                                <RiEditLine className="size-3.5" aria-hidden="true"/>
                                            </Button>
                                        ) : null}
                                    </div>
                                </div>
                            );
                        })}
                    </div>
                )}
            </FieldUnit>
        </div>
    );
}
