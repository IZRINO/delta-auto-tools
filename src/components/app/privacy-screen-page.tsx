import {useCallback, useEffect, useState} from "react";
import {open as openDialog} from "@tauri-apps/plugin-dialog";
import {RiEyeOffLine, RiFolderOpenLine, RiImageLine} from "@remixicon/react";

import {AppPage, HotkeyField, MacroHeader} from "@/components/app/app-ui";
import {formatRecordedHotkey} from "@/components/app/morse-utils";
import {Button} from "@/components/ui/button";
import {useHotkeyRecorder} from "@/hooks/use-hotkey-recorder";
import {useNativeShell} from "@/hooks/use-native-shell";
import {invokeLogged as invoke} from "@/lib/logging";
import {PRIVACY_SCREEN_EVENTS} from "@/lib/tauri-events";
import {subscribeTauriEvent} from "@/lib/tauri-listener";

export type PrivacyScreenSettings = {
    closeHotkey: string;
    imagePath: string | null;
};

export type PrivacyScreenBootstrap = {
    settings: PrivacyScreenSettings;
    visible: boolean;
    hotkeyError: string | null;
    imageDataUrl: string | null;
};

type PrivacyScreenChanged = {
    visible: boolean;
};

const emptyBootstrap: PrivacyScreenBootstrap = {
    settings: {closeHotkey: "", imagePath: null},
    visible: false,
    hotkeyError: null,
    imageDataUrl: null,
};

export function PrivacyScreenPage() {
    const isNative = useNativeShell();
    const [bootstrap, setBootstrap] = useState<PrivacyScreenBootstrap>(emptyBootstrap);
    const [message, setMessage] = useState<string | null>(null);
    const [busy, setBusy] = useState(false);

    const applyBootstrap = useCallback((next: PrivacyScreenBootstrap) => {
        setBootstrap(next);
        if (next.hotkeyError) setMessage(next.hotkeyError);
    }, []);

    useEffect(() => {
        if (!isNative) return;
        void invoke<PrivacyScreenBootstrap>("privacy_screen_get_bootstrap")
            .then(applyBootstrap)
            .catch((cause) => setMessage(String(cause)));
    }, [applyBootstrap, isNative]);

    useEffect(() => {
        if (!isNative) return;
        return subscribeTauriEvent<PrivacyScreenChanged>(PRIVACY_SCREEN_EVENTS.stateChanged, (event) => {
            setBootstrap((current) => ({...current, visible: event.payload.visible}));
        });
    }, [isNative]);

    const saveSettings = useCallback(async (next: PrivacyScreenSettings) => {
        const saved = await invoke<PrivacyScreenBootstrap>("privacy_screen_save_settings", {
            settingsValue: next,
        });
        applyBootstrap(saved);
        return saved;
    }, [applyBootstrap]);

    const recorder = useHotkeyRecorder({
        formatKey: formatRecordedHotkey,
        onCommit: (key) => {
            void saveSettings({...bootstrap.settings, closeHotkey: key}).catch((cause) => {
                setMessage(String(cause));
            });
        },
        onCancel: () => undefined,
        onStatusMessage: setMessage,
    });

    const pickImage = async () => {
        const selected = await openDialog({
            multiple: false,
            filters: [{name: "图片", extensions: ["png", "jpg", "jpeg", "webp", "bmp"]}],
        });
        if (typeof selected !== "string" || selected.length === 0) return;
        try {
            await saveSettings({...bootstrap.settings, imagePath: selected});
            setMessage("已使用自定义图片");
        } catch (cause) {
            setMessage(String(cause));
        }
    };

    const clearImage = async () => {
        try {
            await saveSettings({...bootstrap.settings, imagePath: null});
            setMessage("已恢复默认全黑");
        } catch (cause) {
            setMessage(String(cause));
        }
    };

    const showCover = async () => {
        setBusy(true);
        setMessage(null);
        try {
            applyBootstrap(await invoke<PrivacyScreenBootstrap>("privacy_screen_show"));
        } catch (cause) {
            setMessage(String(cause));
        } finally {
            setBusy(false);
        }
    };

    const hideCover = async () => {
        setBusy(true);
        setMessage(null);
        try {
            applyBootstrap(await invoke<PrivacyScreenBootstrap>("privacy_screen_hide"));
        } catch (cause) {
            setMessage(String(cause));
        } finally {
            setBusy(false);
        }
    };

    const closeHotkey = bootstrap.settings.closeHotkey;
    const canOpen = isNative && closeHotkey.trim().length > 0 && !busy;
    const headerActions = bootstrap.visible ? (
        <Button disabled={busy} onClick={() => void hideCover()} type="button" variant="outline">
            <RiEyeOffLine data-icon="inline-start"/>
            {busy ? "正在关闭" : "关闭息屏"}
        </Button>
    ) : (
        <Button disabled={!canOpen} onClick={() => void showCover()} type="button">
            <RiEyeOffLine data-icon="inline-start"/>
            {busy ? "正在打开" : "打开息屏"}
        </Button>
    );

    return (
        <AppPage>
            <MacroHeader
                actions={headerActions}
                badges={bootstrap.visible ? <span className="badge badge-warning badge-sm">已打开</span> : null}
                code="06"
                subtitle="按钮打开，快捷键只在打开后关闭。只挡画面；识别截图、键鼠和 Alt+Tab 照常。关闭键画在主屏右下角。"
                title="息屏"
                verticalLabel="通用"
            />
            <section className="card card-border col-span-12 bg-base-100">
                <div className="card-body gap-4">
                    {!isNative && <div role="alert" className="alert alert-warning alert-soft">
                        <span>息屏只在桌面端可用</span>
                    </div>}
                    <HotkeyField
                        controlsDisabled={!isNative}
                        helperText="仅在息屏打开后按此键关闭。本程序聚焦时同样有效。"
                        hotkey={closeHotkey}
                        id="privacy-screen-close-hotkey"
                        isRecording={recorder.isRecording}
                        onBeginHotkeyRecording={() => recorder.beginRecording(closeHotkey)}
                        onHotkeyKeyDown={recorder.handleKeyDown}
                        onHotkeyRecorderBlur={recorder.handleBlur}
                    />
                    <div className="flex flex-wrap items-center gap-2">
                        <Button disabled={!isNative} onClick={() => void pickImage()} type="button" variant="outline">
                            <RiFolderOpenLine data-icon="inline-start"/>
                            选择图片
                        </Button>
                        <Button disabled={!isNative || !bootstrap.settings.imagePath} onClick={() => void clearImage()} type="button" variant="ghost">
                            恢复全黑
                        </Button>
                        <span className="text-sm text-base-content/60">
                            {bootstrap.settings.imagePath ?? "默认全黑"}
                        </span>
                    </div>
                    <div className="overflow-hidden rounded-box border border-base-300 bg-black">
                        {bootstrap.imageDataUrl
                            ? <img alt="息屏预览" className="h-40 w-full object-cover" src={bootstrap.imageDataUrl}/>
                            : <div className="flex h-40 items-center justify-center text-sm text-base-content/40">
                                <RiImageLine className="mr-2 size-4"/>默认全黑
                            </div>}
                    </div>
                    {message && <p className="text-sm text-base-content/70">{message}</p>}
                </div>
            </section>
        </AppPage>
    );
}
