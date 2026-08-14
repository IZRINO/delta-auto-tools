import {useEffect, useMemo, useState} from "react";
import {
    RiAddLine,
    RiArrowDownSLine,
    RiCheckLine,
    RiDeleteBinLine,
    RiDownload2Line,
    RiEditLine,
    RiFileCopyLine,
    RiUpload2Line,
} from "@remixicon/react";
import {open as openDialog, save as saveDialog} from "@tauri-apps/plugin-dialog";

import {Button} from "@/components/ui/button";
import {Input} from "@/components/ui/input";
import {
    Popover,
    PopoverContent,
    PopoverTrigger,
} from "@/components/ui/popover";
import {useProfile} from "@/hooks/use-profile";
import type {Profile} from "@/components/app/profile-types";
import {
    sortProfilesForSwitcher,
    validateProfileName,
} from "@/components/app/profile-utils";
import {publishUiCoverState} from "@/components/app/settings-dialog-events";
import {cn} from "@/lib/utils";

const PROFILE_FILE_FILTERS = [{name: "Delta Profile", extensions: ["json"]}];
const PROFILE_COVER_SOURCE = "profile-switcher";

function profileExportFileName(profile: Profile): string {
    const safeName = profile.name.trim().replace(/[<>:"/\\|?*\x00-\x1F]/g, "_") || "profile";
    return `profile-${safeName}.json`;
}

export function ProfileSwitcher() {
    const {
        bootstrap,
        activeProfile,
        activeProfileName,
        loading,
        error,
        switchingProfileId,
        createDefaultProfile,
        switchProfile,
        renameProfile,
        saveCurrentProfile,
        deleteProfile,
        exportProfileToPath,
        importProfileFromPath,
    } = useProfile();
    const [open, setOpen] = useState(false);
    const [renamingId, setRenamingId] = useState<string | null>(null);
    const [renameValue, setRenameValue] = useState("");
    const [message, setMessage] = useState<string | null>(null);
    const [busy, setBusy] = useState(false);
    const [saveAsOpen, setSaveAsOpen] = useState(false);
    const [saveAsName, setSaveAsName] = useState("");

    const profiles = useMemo(
        () => sortProfilesForSwitcher(bootstrap?.profiles ?? [], bootstrap?.activeProfileId ?? ""),
        [bootstrap?.activeProfileId, bootstrap?.profiles],
    );
    const profileActionBusy = busy || Boolean(switchingProfileId);

    const setPopoverOpen = (nextOpen: boolean) => {
        setOpen(nextOpen);
        publishUiCoverState(PROFILE_COVER_SOURCE, nextOpen);
    };

    useEffect(() => {
        return () => publishUiCoverState(PROFILE_COVER_SOURCE, false);
    }, []);

    const beginRename = (profile: Profile) => {
        setRenamingId(profile.id);
        setRenameValue(profile.name);
        setMessage(null);
    };

    const commitRename = async () => {
        if (!renamingId) return;
        const validation = validateProfileName(renameValue);
        if (validation) {
            setMessage(validation);
            return;
        }
        setBusy(true);
        try {
            await renameProfile(renamingId, renameValue.trim());
            setRenamingId(null);
            setRenameValue("");
            setMessage(null);
        } catch (err) {
            setMessage(String(err));
        } finally {
            setBusy(false);
        }
    };

    const handleSwitch = async (profile: Profile) => {
        if (switchingProfileId) return;
        if (profile.id === activeProfile?.id) return;
        setBusy(true);
        try {
            await switchProfile(profile.id);
            setPopoverOpen(false);
            setMessage(null);
        } catch (err) {
            setMessage(String(err));
        } finally {
            setBusy(false);
        }
    };

    const handleCreate = async () => {
        setBusy(true);
        try {
            await createDefaultProfile();
            setPopoverOpen(false);
            setMessage(null);
        } catch (err) {
            setMessage(String(err));
        } finally {
            setBusy(false);
        }
    };

    const handleSaveAs = async () => {
        const validation = validateProfileName(saveAsName);
        if (validation) {
            setMessage(validation);
            return;
        }
        setBusy(true);
        try {
            await saveCurrentProfile(saveAsName.trim());
            setSaveAsOpen(false);
            setSaveAsName("");
            setMessage("已复制为新配置。");
        } catch (err) {
            setMessage(String(err));
        } finally {
            setBusy(false);
        }
    };

    const handleDelete = async (profile: Profile) => {
        if (profile.id === activeProfile?.id) {
            setMessage("不能删除当前激活的配置。");
            return;
        }
        if (!window.confirm(`删除配置「${profile.name}」？此操作不可撤销。`)) {
            return;
        }
        setBusy(true);
        try {
            await deleteProfile(profile.id);
            setMessage("配置已删除。");
        } catch (err) {
            setMessage(String(err));
        } finally {
            setBusy(false);
        }
    };

    const handleExport = async (profile: Profile) => {
        setBusy(true);
        try {
            const path = await saveDialog({
                defaultPath: profileExportFileName(profile),
                filters: PROFILE_FILE_FILTERS,
                title: "导出配置",
            });
            if (!path) {
                return;
            }
            await exportProfileToPath(profile.id, path);
            setMessage(`已导出配置：${profile.name}`);
        } catch (err) {
            setMessage(String(err));
        } finally {
            setBusy(false);
        }
    };

    const handleImport = async () => {
        setBusy(true);
        try {
            const selected = await openDialog({
                multiple: false,
                filters: PROFILE_FILE_FILTERS,
                title: "导入配置",
            });
            if (typeof selected !== "string") {
                return;
            }
            await importProfileFromPath(selected);
            setMessage("已导入配置。");
        } catch (err) {
            setMessage(String(err));
        } finally {
            setBusy(false);
        }
    };

    return (
        <Popover open={open} onOpenChange={setPopoverOpen}>
            <PopoverTrigger asChild>
                <Button
                    type="button"
                    variant="outline"
                    size="lg"
                    className="h-8 min-w-32 justify-between border px-2 text-xs"
                    aria-label="切换配置"
                    disabled={loading}
                >
                    <span className="min-w-0 truncate">
                        {loading ? "配置加载中" : error ? "配置异常" : activeProfileName}
                    </span>
                    <RiArrowDownSLine className="size-3.5" data-icon="inline-end" aria-hidden="true"/>
                </Button>
            </PopoverTrigger>
            <PopoverContent align="end" className="w-72 gap-2 p-2">
                <div className="border-b border-base-content pb-2">
                    <p className="font-mono text-caption font-semibold text-base-content/60">
                        PROFILE / CONFIG SLOT
                    </p>
                    <p className="mt-1 truncate text-sm font-semibold">{activeProfileName}</p>
                </div>

                {message ? (
                    <div className="border border-warning bg-warning/10 px-2 py-1 font-mono text-caption text-warning">
                        {message}
                    </div>
                ) : null}

                <div className="flex max-h-64 flex-col overflow-y-auto border border-base-300">
                    {profiles.map((profile) => {
                        const active = profile.id === activeProfile?.id;
                        const renaming = renamingId === profile.id;
                        const switching = switchingProfileId === profile.id;
                        const switchDisabled = busy || Boolean(switchingProfileId) || active;
                        return (
                            <div
                                key={profile.id}
                                className={cn(
                                    "grid grid-cols-[minmax(0,1fr)_auto_auto_auto] items-center border-b border-base-300 last:border-b-0",
                                    active ? "bg-primary/10" : "bg-base-100",
                                )}
                            >
                                {renaming ? (
                                    <div className="col-span-4 flex items-center gap-1 p-1.5">
                                        <Input
                                            value={renameValue}
                                            onChange={(event) => setRenameValue(event.target.value)}
                                            onKeyDown={(event) => {
                                                if (event.key === "Enter") void commitRename();
                                                if (event.key === "Escape") setRenamingId(null);
                                            }}
                                            className="h-7 flex-1"
                                            autoFocus
                                            spellCheck={false}
                                        />
                                        <Button
                                            type="button"
                                            size="icon-sm"
                                            variant="outline"
                                            onClick={() => void commitRename()}
                                            disabled={profileActionBusy}
                                            aria-label="确认重命名"
                                        >
                                            <RiCheckLine className="size-3.5" data-icon="inline-start" aria-hidden="true"/>
                                        </Button>
                                    </div>
                                ) : (
                                    <>
                                        <button
                                            type="button"
                                            className="min-w-0 px-2 py-2 text-left hover:bg-base-200 focus:outline-none focus-visible:outline-2 focus-visible:outline-primary"
                                            onClick={() => void handleSwitch(profile)}
                                            disabled={switchDisabled}
                                        >
                                            <span className="block truncate text-xs font-semibold">
                                                {switching ? "切换中" : profile.name}
                                            </span>
                                            <span className="mt-0.5 block font-mono text-caption text-base-content/60">
                                                {switching ? "SWITCHING" : active ? "ACTIVE" : "READY"}
                                            </span>
                                        </button>
                                        <Button
                                            type="button"
                                            size="icon-sm"
                                            variant="ghost"
                                            onClick={() => void handleExport(profile)}
                                            disabled={profileActionBusy}
                                            aria-label={`导出 ${profile.name}`}
                                        >
                                            <RiDownload2Line className="size-3.5" data-icon="inline-start" aria-hidden="true"/>
                                        </Button>
                                        <Button
                                            type="button"
                                            size="icon-sm"
                                            variant="ghost"
                                            onClick={() => beginRename(profile)}
                                            disabled={profileActionBusy}
                                            aria-label={`重命名 ${profile.name}`}
                                        >
                                            <RiEditLine className="size-3.5" data-icon="inline-start" aria-hidden="true"/>
                                        </Button>
                                        <Button
                                            type="button"
                                            size="icon-sm"
                                            variant="ghost"
                                            className="mr-1"
                                            onClick={() => void handleDelete(profile)}
                                            disabled={profileActionBusy || active}
                                            aria-label={active ? `不能删除当前激活配置 ${profile.name}` : `删除 ${profile.name}`}
                                            title={active ? "当前激活配置不可删除" : "删除配置"}
                                        >
                                            <RiDeleteBinLine className="size-3.5" data-icon="inline-start" aria-hidden="true"/>
                                        </Button>
                                    </>
                                )}
                            </div>
                        );
                    })}
                </div>

                <Button
                    type="button"
                    variant="default"
                    className="w-full justify-start"
                    onClick={() => void handleCreate()}
                    disabled={profileActionBusy || loading}
                >
                    <RiAddLine className="size-4" data-icon="inline-start" aria-hidden="true"/>
                    新增配置
                </Button>

                <Button
                    type="button"
                    variant="outline"
                    className="w-full justify-start"
                    onClick={() => void handleImport()}
                    disabled={profileActionBusy || loading}
                >
                    <RiUpload2Line className="size-4" data-icon="inline-start" aria-hidden="true"/>
                    导入配置
                </Button>

                {saveAsOpen ? (
                    <div className="flex items-center gap-1">
                        <Input
                            value={saveAsName}
                            onChange={(event) => setSaveAsName(event.target.value)}
                            onKeyDown={(event) => {
                                if (event.key === "Enter") void handleSaveAs();
                                if (event.key === "Escape") {
                                    setSaveAsOpen(false);
                                    setSaveAsName("");
                                }
                            }}
                            placeholder="复制后的配置名称"
                            className="h-7 flex-1"
                            autoFocus
                            spellCheck={false}
                        />
                        <Button
                            type="button"
                            size="icon-sm"
                            variant="outline"
                            onClick={() => void handleSaveAs()}
                            disabled={profileActionBusy}
                            aria-label="确认复制"
                        >
                            <RiCheckLine className="size-3.5" data-icon="inline-start" aria-hidden="true"/>
                        </Button>
                    </div>
                ) : (
                    <Button
                        type="button"
                        variant="outline"
                        className="w-full justify-start"
                        onClick={() => setSaveAsOpen(true)}
                        disabled={profileActionBusy || loading}
                    >
                        <RiFileCopyLine className="size-4" data-icon="inline-start" aria-hidden="true"/>
                        复制
                    </Button>
                )}
            </PopoverContent>
        </Popover>
    );
}
