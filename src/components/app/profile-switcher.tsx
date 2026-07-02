import {useMemo, useState} from "react";
import {
    RiAddLine,
    RiArrowDownSLine,
    RiCheckLine,
    RiEditLine,
    RiSave2Line,
} from "@remixicon/react";

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
import {cn} from "@/lib/utils";

export function ProfileSwitcher() {
    const {
        bootstrap,
        activeProfile,
        activeProfileName,
        loading,
        error,
        createDefaultProfile,
        switchProfile,
        renameProfile,
        saveCurrentProfile,
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
        if (profile.id === activeProfile?.id) return;
        setBusy(true);
        try {
            await switchProfile(profile.id);
            setOpen(false);
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
            setOpen(false);
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
            setMessage(null);
        } catch (err) {
            setMessage(String(err));
        } finally {
            setBusy(false);
        }
    };

    return (
        <Popover open={open} onOpenChange={setOpen}>
            <PopoverTrigger asChild>
                <Button
                    type="button"
                    variant="outline"
                    size="lg"
                    className="h-[34px] min-w-[132px] justify-between border-2 px-2 text-[0.58rem]"
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
                <div className="border-b border-[var(--chalk)] pb-2">
                    <p className="font-mono text-[0.58rem] font-black tracking-[0.18em] text-[var(--zinc)] uppercase">
                        PROFILE / CONFIG SLOT
                    </p>
                    <p className="mt-1 truncate text-sm font-black">{activeProfileName}</p>
                </div>

                {message ? (
                    <div className="border border-[var(--rust)] bg-[var(--rust)]/10 px-2 py-1 font-mono text-[0.58rem] text-[var(--rust)]">
                        {message}
                    </div>
                ) : null}

                <div className="flex max-h-64 flex-col overflow-y-auto border border-[var(--seam)]">
                    {profiles.map((profile) => {
                        const active = profile.id === activeProfile?.id;
                        const renaming = renamingId === profile.id;
                        return (
                            <div
                                key={profile.id}
                                className={cn(
                                    "grid grid-cols-[minmax(0,1fr)_auto] items-center border-b border-[var(--seam)] last:border-b-0",
                                    active ? "bg-[var(--amber)]/10" : "bg-[var(--carbon)]",
                                )}
                            >
                                {renaming ? (
                                    <div className="col-span-2 flex items-center gap-1 p-1.5">
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
                                            disabled={busy}
                                            aria-label="确认重命名"
                                        >
                                            <RiCheckLine className="size-3.5" aria-hidden="true"/>
                                        </Button>
                                    </div>
                                ) : (
                                    <>
                                        <button
                                            type="button"
                                            className="min-w-0 px-2 py-2 text-left hover:bg-[var(--slate)] focus:outline-none focus-visible:outline-2 focus-visible:outline-[var(--amber)]"
                                            onClick={() => void handleSwitch(profile)}
                                            disabled={busy}
                                        >
                                            <span className="block truncate text-xs font-black">
                                                {profile.name}
                                            </span>
                                            <span className="mt-0.5 block font-mono text-[0.56rem] text-[var(--zinc)]">
                                                {active ? "ACTIVE" : "READY"}
                                            </span>
                                        </button>
                                        <Button
                                            type="button"
                                            size="icon-sm"
                                            variant="ghost"
                                            className="mr-1"
                                            onClick={() => beginRename(profile)}
                                            disabled={busy}
                                            aria-label={`重命名 ${profile.name}`}
                                        >
                                            <RiEditLine className="size-3.5" aria-hidden="true"/>
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
                    disabled={busy || loading}
                >
                    <RiAddLine className="size-4" data-icon="inline-start" aria-hidden="true"/>
                    新增配置
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
                            placeholder="配置名称"
                            className="h-7 flex-1"
                            autoFocus
                            spellCheck={false}
                        />
                        <Button
                            type="button"
                            size="icon-sm"
                            variant="outline"
                            onClick={() => void handleSaveAs()}
                            disabled={busy}
                            aria-label="确认另存为"
                        >
                            <RiCheckLine className="size-3.5" aria-hidden="true"/>
                        </Button>
                    </div>
                ) : (
                    <Button
                        type="button"
                        variant="outline"
                        className="w-full justify-start"
                        onClick={() => setSaveAsOpen(true)}
                        disabled={busy || loading}
                    >
                        <RiSave2Line className="size-4" data-icon="inline-start" aria-hidden="true"/>
                        另存为
                    </Button>
                )}
            </PopoverContent>
        </Popover>
    );
}
