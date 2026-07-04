import {useCallback, useEffect, useMemo, useState} from "react";
import {invoke} from "@tauri-apps/api/core";
import {open} from "@tauri-apps/plugin-dialog";
import {listen} from "@tauri-apps/api/event";
import {AUDIO_EVENTS} from "@/lib/tauri-events";
import {RiArrowDownLine, RiArrowUpLine, RiCheckLine, RiCloseLine, RiDeleteBinLine, RiFolderOpenLine, RiPlayLine, RiVolumeUpLine,} from "@remixicon/react";
import {toast} from "sonner";

import {Badge} from "@/components/ui/badge";
import {Button} from "@/components/ui/button";
import {Field, FieldContent, FieldGroup, FieldLabel} from "@/components/ui/field";
import {Input} from "@/components/ui/input";
import {Select, SelectContent, SelectItem, SelectTrigger, SelectValue} from "@/components/ui/select";
import {Switch} from "@/components/ui/switch";
import {
    AppPage,
    AddCardButton,
    CardBody,
    ControlTile,
    MacroHeader,
    PagePreviewBanner,
    SaveStateBadge,
    SectionHeader,
    SignalTile,
    TacticalCard,
} from "@/components/app/app-ui";
import type {AudioBootstrap, AudioCard, AudioSettings, AudioSettingsForm, ColorProbeForm} from "@/components/app/audio-types";
import {AUDIO_AUTOSAVE_DELAY_MS} from "@/components/app/audio-types";
import {
    createEmptyAudioCard,
    mergeAudioWatchRegionsIntoForm,
    parseSettingsForm,
    rgbToHex,
    settingsToForm
} from "@/components/app/audio-utils";
import {getErrorMessage, getSelectionRect} from "@/components/app/morse-utils";
import type {Point} from "@/components/app/morse-types";
import {MIN_SELECTION_HEIGHT, MIN_SELECTION_WIDTH} from "@/components/app/morse-types";
import {useNativeShell} from "@/hooks/use-native-shell";
import {useBootstrapForm} from "@/hooks/use-bootstrap-form";
import {useAutosave} from "@/hooks/use-autosave";

const AUDIO_BOOTSTRAP_SPEC = {
    getBootstrapCommand: "audio_get_bootstrap",
    saveSettingsCommand: "audio_save_settings",
    settingsToForm,
    parseSettingsForm,
};

export function AudioPage() {
    const isNativeShell = useNativeShell();
    return <AudioWorkbench isNativeShell={isNativeShell}/>;
}

function AudioWorkbench({isNativeShell}: { isNativeShell: boolean }) {
    const {
        form,
        setForm,
        setBootstrap,
        isDirty,
        updateForm,
        saveSettings,
        loading,
        saving,
        pageError,
        setPageError,
        statusMessage,
        setStatusMessage,
        autosaveVersionRef,
    } = useBootstrapForm<AudioBootstrap, AudioSettings, AudioSettingsForm>({
        spec: AUDIO_BOOTSTRAP_SPEC,
        isNativeShell,
        loadStatusMessage: "正在加载音频设置...",
        readyStatusMessage: "音频模块就绪。",
    });

    useAutosave<AudioSettingsForm>({
        form,
        isDirty,
        disabled: !isNativeShell || loading || !form,
        onSave: (formSnapshot, nextVersion) => saveSettings(parseSettingsForm(formSnapshot), nextVersion),
        onError: (message) => {
            toast.error(`保存失败：${message}`);
        },
        delay: AUDIO_AUTOSAVE_DELAY_MS,
        autosaveVersionRef,
    });

    useEffect(() => {
        if (!isNativeShell) return;
        let disposed = false;
        let unlistenStateChanged: (() => void) | undefined;
        let unlistenHotkeyError: (() => void) | undefined;
        let unlistenHotkeyTriggered: (() => void) | undefined;
        let unlistenRegionMatched: (() => void) | undefined;

        void listen<AudioBootstrap>(AUDIO_EVENTS.stateChanged, (event) => {
            if (disposed) return;
            const next = event.payload;
            setBootstrap(next);
            setForm((current) => mergeAudioWatchRegionsIntoForm(current, next.settings));
            setPageError(null);
        }).then((dispose) => {
            unlistenStateChanged = dispose;
        });

        void listen<string>(AUDIO_EVENTS.hotkeyError, (event) => {
            if (disposed) return;
            setPageError(event.payload);
            setStatusMessage(event.payload);
            toast.error(event.payload);
        }).then((dispose) => {
            unlistenHotkeyError = dispose;
        });

        void listen<string>(AUDIO_EVENTS.hotkeyTriggered, (event) => {
            if (disposed) return;
            toast.info(`快捷键触发：卡片 ${event.payload}`);
            setStatusMessage(`快捷键触发：卡片 ${event.payload}`);
        }).then((dispose) => {
            unlistenHotkeyTriggered = dispose;
        });

        void listen<string>(AUDIO_EVENTS.regionMatched, (event) => {
            if (disposed) return;
            toast.info(`区域匹配触发：卡片 ${event.payload}`);
            setStatusMessage(`区域匹配触发：卡片 ${event.payload}`);
        }).then((dispose) => {
            unlistenRegionMatched = dispose;
        });

        return () => {
            disposed = true;
            unlistenStateChanged?.();
            unlistenHotkeyError?.();
            unlistenHotkeyTriggered?.();
            unlistenRegionMatched?.();
        };
    }, [isNativeShell, setBootstrap, setForm, setPageError, setStatusMessage]);

    const handleAddCard = useCallback(() => {
        setForm((current) => {
            if (!current) return current;
            const newCard = cardToForm(createEmptyAudioCard());
            return {...current, cards: [...current.cards, newCard]};
        });
    }, [setForm]);

    const handleRemoveCard = useCallback((index: number) => {
        setForm((current) => {
            if (!current) return current;
            return {...current, cards: current.cards.filter((_, i) => i !== index)};
        });
    }, [setForm]);

    const handleUpdateCard = useCallback(
        (index: number, patch: Partial<AudioSettingsForm["cards"][number]>) => {
            setForm((current) => {
                if (!current) return current;
                const nextCards = current.cards.map((card, i) => (i === index ? {...card, ...patch} : card));
                return {...current, cards: nextCards};
            });
        },
        [setForm],
    );

    // 操作前强制把当前 form 落盘到后端内存：避免「新建卡片/改字段后未等 400ms autosave 落地就调后端命令」
    // 导致后端按 cardId 查不到卡或读到旧字段（「卡片不存在」/「只有识色模式才可测试」）。
    // 直接 invoke save 命令、不经 useBootstrapForm.saveSettings，不重置前端 form 草稿；
    // 失败（如必填项缺失）时 toast 提示并抛出带标记的 Error，让调用方中断后续命令且不重复弹窗。
    const flushSettings = useCallback(async (): Promise<void> => {
        if (!isNativeShell || !form) return;
        try {
            await invoke(AUDIO_BOOTSTRAP_SPEC.saveSettingsCommand, {
                settingsValue: parseSettingsForm(form),
            });
        } catch (error) {
            const message = getErrorMessage(error);
            toast.error(`保存设置失败：${message}`);
            // 用带标记的 Error 抛出，调用方通过 name 判定避免重复弹窗
            const wrapped = new Error(message);
            wrapped.name = "FlushSettingsError";
            throw wrapped;
        }
    }, [isNativeShell, form]);

    const handleTestPlay = useCallback(
        async (cardId: string) => {
            if (!isNativeShell) return;
            try {
                await flushSettings();
                await invoke("audio_test_play", {cardId});
                toast.success("播放测试已触发");
            } catch (error) {
                if (error instanceof Error && error.name === "FlushSettingsError") return;
                toast.error(getErrorMessage(error));
            }
        },
        [isNativeShell, flushSettings],
    );

    const handleTestMatch = useCallback(
        async (cardId: string) => {
            if (!isNativeShell) return;
            try {
                await flushSettings();
                type TestMatchResult = { similarity: number; triggered: boolean; matchPosition: { x: number; y: number } | null };
                const result = await invoke<TestMatchResult>("audio_test_match", {cardId});
                const pos = result.matchPosition ? ` (位置: ${result.matchPosition.x}, ${result.matchPosition.y})` : "";
                toast.success(
                    `匹配度: ${(result.similarity * 100).toFixed(1)}% ${result.triggered ? "(已触发)" : "(未触发)"}${pos}`
                );
            } catch (error) {
                if (error instanceof Error && error.name === "FlushSettingsError") return;
                toast.error(getErrorMessage(error));
            }
        },
        [isNativeShell, flushSettings],
    );

    const handleBeginRegionSelection = useCallback(
        async (cardId: string, probeIndex?: number) => {
            if (!isNativeShell) return;
            try {
                await flushSettings();
                await invoke("audio_begin_region_selection", {cardId, probeIndex});
            } catch (error) {
                if (error instanceof Error && error.name === "FlushSettingsError") return;
                toast.error(getErrorMessage(error));
            }
        },
        [isNativeShell, flushSettings],
    );

    const handleTestColorMatch = useCallback(
        async (cardId: string) => {
            if (!isNativeShell) return;
            type ColorTargetResult = {
                matched: boolean;
                targetColor: [number, number, number];
                tolerance: number;
                sampledColor: [number, number, number];
                distance: number;
                matchingPixelCount?: number;
            };
            type ColorProbeResult = {
                matched: boolean;
                sampledColor: [number, number, number];
                distance: number;
                targetColor: [number, number, number];
                tolerance: number;
                matchingPixelCount?: number;
                targets: ColorTargetResult[];
            };
            type ColorTestResult = {
                triggered: boolean;
                hitCount: number;
                totalCount: number;
                probes: ColorProbeResult[];
            };
            try {
                await flushSettings();
                const result = await invoke<ColorTestResult>("audio_test_color_match", {cardId});
                const detail = result.probes
                    .map((p, i) => {
                        const sample = p.sampledColor.map((v) => v.toString(16).padStart(2, "0")).join("");
                        const count = p.matchingPixelCount && p.matchingPixelCount > 0 ? ` 命中${p.matchingPixelCount}px` : "";
                        return `#${i + 1}: ${p.matched ? "命中" : "未中"} (采样 #${sample} 距离 ${p.distance.toFixed(1)}${count})`;
                    })
                    .join("\n");
                const summary = `识色: ${result.hitCount}/${result.totalCount} 命中 ${result.triggered ? "(已触发)" : "(未触发)"}`;
                toast.success(`${summary}\n${detail}`, {duration: 6000});
            } catch (error) {
                if (error instanceof Error && error.name === "FlushSettingsError") return;
                toast.error(getErrorMessage(error));
            }
        },
        [isNativeShell, flushSettings],
    );

    const handleAddColorProbe = useCallback(
        (index: number) => {
            setForm((current) => {
                if (!current) return current;
                const card = current.cards[index];
                if (!card) return current;
                const newProbe: ColorProbeForm = {region: null, targets: [{color: "#ff0000", tolerance: "30"}], probeMatchMode: "any"};
                const nextCards = current.cards.map((c, i) =>
                    i === index ? {...c, colorProbes: [...c.colorProbes, newProbe]} : c,
                );
                return {...current, cards: nextCards};
            });
        },
        [setForm],
    );

    const handleRemoveColorProbe = useCallback(
        (cardIndex: number, probeIndex: number) => {
            setForm((current) => {
                if (!current) return current;
                const card = current.cards[cardIndex];
                if (!card) return current;
                const nextProbes = card.colorProbes.filter((_, i) => i !== probeIndex);
                const nextCards = current.cards.map((c, i) =>
                    i === cardIndex ? {...c, colorProbes: nextProbes} : c,
                );
                return {...current, cards: nextCards};
            });
        },
        [setForm],
    );

    const handleUpdateColorProbe = useCallback(
        (cardIndex: number, probeIndex: number, patch: Partial<ColorProbeForm>) => {
            setForm((current) => {
                if (!current) return current;
                const card = current.cards[cardIndex];
                if (!card) return current;
                const nextProbes = card.colorProbes.map((p, i) =>
                    i === probeIndex ? {...p, ...patch} : p,
                );
                const nextCards = current.cards.map((c, i) =>
                    i === cardIndex ? {...c, colorProbes: nextProbes} : c,
                );
                return {...current, cards: nextCards};
            });
        },
        [setForm],
    );

    const handleLoadReferencePreview = useCallback(
        async (cardId: string): Promise<string | null> => {
            if (!isNativeShell) return null;
            try {
                const dataUrl = await invoke<string>("audio_read_reference_image", {cardId});
                return dataUrl;
            } catch {
                return null;
            }
        },
        [isNativeShell],
    );

    const handlePickReferenceImage = useCallback(
        async (index: number) => {
            if (!isNativeShell) return;
            try {
                const path = await open({
                    multiple: false,
                    directory: false,
                    filters: [{name: "图像文件", extensions: ["png", "jpg", "jpeg", "webp", "bmp", "gif", "tiff"]}],
                });
                if (path && typeof path === "string") {
                    handleUpdateCard(index, {watchReferenceImagePath: path});
                }
            } catch (error) {
                toast.error(getErrorMessage(error));
            }
        },
        [isNativeShell, handleUpdateCard],
    );

    const handlePickAudioFile = useCallback(
        async (index: number) => {
            if (!isNativeShell) return;
            try {
                const picked = await open({
                    multiple: true,
                    directory: false,
                    filters: [{name: "音频文件", extensions: ["mp3", "wav", "ogg", "flac", "aac", "m4a", "wma"]}],
                });
                if (!picked) return;
                const newPaths = Array.isArray(picked) ? picked.filter((p): p is string => typeof p === "string") : [picked];
                if (newPaths.length === 0) return;
                setForm((current) => {
                    if (!current) return current;
                    const card = current.cards[index];
                    if (!card) return current;
                    const existing = card.audioFiles ?? [];
                    const merged = [...existing, ...newPaths];
                    const nextCards = current.cards.map((c, i) =>
                        i === index ? {...c, audioFiles: merged} : c,
                    );
                    return {...current, cards: nextCards};
                });
            } catch (error) {
                toast.error(getErrorMessage(error));
            }
        },
        [isNativeShell, setForm],
    );

    const handleRemoveAudioFile = useCallback(
        (cardIndex: number, fileIndex: number) => {
            setForm((current) => {
                if (!current) return current;
                const card = current.cards[cardIndex];
                if (!card) return current;
                const nextFiles = (card.audioFiles ?? []).filter((_, i) => i !== fileIndex);
                const nextCards = current.cards.map((c, i) =>
                    i === cardIndex ? {...c, audioFiles: nextFiles} : c,
                );
                return {...current, cards: nextCards};
            });
        },
        [setForm],
    );

    const handleMoveAudioFile = useCallback(
        (cardIndex: number, fileIndex: number, direction: -1 | 1) => {
            setForm((current) => {
                if (!current) return current;
                const card = current.cards[cardIndex];
                if (!card) return current;
                const files = [...(card.audioFiles ?? [])];
                const target = fileIndex + direction;
                if (target < 0 || target >= files.length) return current;
                [files[fileIndex], files[target]] = [files[target], files[fileIndex]];
                const nextCards = current.cards.map((c, i) =>
                    i === cardIndex ? {...c, audioFiles: files} : c,
                );
                return {...current, cards: nextCards};
            });
        },
        [setForm],
    );

    // Issue #62: 单独设置某段音频的连杀窗口（"" 表示该段用卡片级默认）。
    const handleUpdateComboWindow = useCallback(
        (cardIndex: number, fileIndex: number, value: string) => {
            setForm((current) => {
                if (!current) return current;
                const card = current.cards[cardIndex];
                if (!card) return current;
                const files = card.audioFiles ?? [];
                const base = card.comboWindows ?? [];
                const next = files.map((_, i) => (i === fileIndex ? value : String(base[i] ?? "")));
                const nextCards = current.cards.map((c, i) =>
                    i === cardIndex ? {...c, comboWindows: next} : c,
                );
                return {...current, cards: nextCards};
            });
        },
        [setForm],
    );

    const enabled = form?.audioEnabled ?? false;
    const cardCount = form?.cards.length ?? 0;
    const activeCards = form?.cards.filter((c) => c.enabled).length ?? 0;

    return (
        <AppPage className="auto-rows-max">
            <MacroHeader
                className="col-span-12"
                code="A-04"
                title="AUDIO / 音频"
                verticalLabel="音频"
                subtitle="快捷键触发或区域监听+图像匹配触发音频播放。"
                badges={
                    <>
                        <Badge variant={enabled ? "default" : "outline"}>{enabled ? "已启用" : "已禁用"}</Badge>
                        <Badge variant="secondary">{activeCards} 卡片激活</Badge>
                        <SaveStateBadge dirty={isDirty} saving={saving}/>
                    </>
                }
                actions={
                    <>
                        <SignalTile label="总开关" value={enabled ? "ON" : "OFF"} detail={statusMessage}/>
                        <SignalTile label="卡片数" value={cardCount} detail="已配置"/>
                    </>
                }
            />

            {pageError && (
                <div
                    className="col-span-12 mb-3 border border-error bg-error/10 px-3 py-2 font-mono text-xs font-semibold text-error">
                    [ 错误 ] {pageError}
                </div>
            )}

            {!isNativeShell && <div className="col-span-12"><PagePreviewBanner/></div>}

            <TacticalCard className="col-span-12 mt-3">
                <SectionHeader eyebrow="全局设置" title="全局设置"/>
                <CardBody>
                    <ControlTile>
                        <div className="flex items-center gap-3">
                            <Switch
                                checked={enabled}
                                onCheckedChange={(v) => updateForm("audioEnabled", v)}
                                aria-label="音频总开关"
                            />
                            <span
                                className="font-mono text-xs font-semibold text-base-content">
                {enabled ? "已启用" : "已禁用"}
              </span>
                        </div>
                    </ControlTile>
                </CardBody>
            </TacticalCard>

            <TacticalCard className="col-span-12 mt-3">
                <SectionHeader eyebrow="音频卡片" title="音频卡片" />
                <section className="@container grid min-h-0 gap-3 p-3 @xl:grid-cols-2">
                    {form?.cards.map((card, index) => (
                        <AudioCardEditor
                            key={card.id}
                            card={card}
                            index={index}
                            isNativeShell={isNativeShell}
                            onUpdate={(patch) => handleUpdateCard(index, patch)}
                            onRemove={() => handleRemoveCard(index)}
                            onTestPlay={() => handleTestPlay(card.id)}
                            onTestMatch={() => handleTestMatch(card.id)}
                            onBeginRegionSelection={(probeIndex) => handleBeginRegionSelection(card.id, probeIndex)}
                            onPickReferenceImage={() => handlePickReferenceImage(index)}
                            onPickAudioFile={() => handlePickAudioFile(index)}
                            onRemoveAudioFile={(fileIndex) => handleRemoveAudioFile(index, fileIndex)}
                            onMoveAudioFile={(fileIndex, direction) => handleMoveAudioFile(index, fileIndex, direction)}
                            onUpdateComboWindow={(fileIndex, value) => handleUpdateComboWindow(index, fileIndex, value)}
                            onLoadReferencePreview={() => handleLoadReferencePreview(card.id)}
                            onTestColorMatch={() => handleTestColorMatch(card.id)}
                            onAddColorProbe={() => handleAddColorProbe(index)}
                            onRemoveColorProbe={(probeIndex) => handleRemoveColorProbe(index, probeIndex)}
                            onUpdateColorProbe={(probeIndex, patch) => handleUpdateColorProbe(index, probeIndex, patch)}
                        />
                    ))}
                    <AddCardButton
                        className="min-h-36"
                        disabled={!isNativeShell || loading}
                        title="新增音频卡片"
                        description="添加新的快捷键触发或区域监听音频卡片。"
                        onClick={handleAddCard}
                    />
                </section>
            </TacticalCard>
        </AppPage>
    );
}

function AudioCardEditor({
                             card,
                             index,
                             isNativeShell,
                             onUpdate,
                             onRemove,
                             onTestPlay,
                             onTestMatch,
                             onBeginRegionSelection,
                             onPickReferenceImage,
                             onPickAudioFile,
                             onRemoveAudioFile,
                             onMoveAudioFile,
                             onUpdateComboWindow,
                             onLoadReferencePreview,
                             onTestColorMatch,
                             onAddColorProbe,
                             onRemoveColorProbe,
                             onUpdateColorProbe,
                         }: {
    card: AudioSettingsForm["cards"][number];
    index: number;
    isNativeShell: boolean;
    onUpdate: (patch: Partial<AudioSettingsForm["cards"][number]>) => void;
    onRemove: () => void;
    onTestPlay: () => void;
    onTestMatch: () => void;
    onBeginRegionSelection: (probeIndex?: number) => void;
    onPickReferenceImage: () => void;
    onPickAudioFile: () => void;
    onRemoveAudioFile: (fileIndex: number) => void;
    onMoveAudioFile: (fileIndex: number, direction: -1 | 1) => void;
    onUpdateComboWindow: (fileIndex: number, value: string) => void;
    onLoadReferencePreview: () => Promise<string | null>;
    onTestColorMatch: () => void;
    onAddColorProbe: () => void;
    onRemoveColorProbe: (probeIndex: number) => void;
    onUpdateColorProbe: (probeIndex: number, patch: Partial<ColorProbeForm>) => void;
}) {
    const isHotkey = card.triggerMode === "hotkey";
    const isRegion = card.triggerMode === "regionWatch";
    const isColor = card.triggerMode === "colorWatch";

    // 参考图像预览
    const [previewUrl, setPreviewUrl] = useState<string | null>(null);
    const [previewLoading, setPreviewLoading] = useState(false);

    useEffect(() => {
        if (!isRegion || !card.watchReferenceImagePath.trim() || !isNativeShell) {
            setPreviewUrl(null);
            return;
        }
        let disposed = false;
        setPreviewLoading(true);
        void onLoadReferencePreview().then((url) => {
            if (!disposed) {
                setPreviewUrl(url);
                setPreviewLoading(false);
            }
        });
        return () => { disposed = true; };
    }, [card.watchReferenceImagePath, isRegion, isNativeShell, onLoadReferencePreview]);

    return (
        <div className="border border-base-300 bg-base-200">
            <div
                className="flex items-center justify-between border-b-2 border-base-content bg-base-100 px-3 py-2">
                <div className="flex items-center gap-2">
                    <span
                        className="font-mono text-xs font-semibold text-primary">A-{String(index + 1).padStart(2, "0")}</span>
                    <Switch
                        checked={card.enabled}
                        onCheckedChange={(v) => onUpdate({enabled: v})}
                        aria-label={`卡片 ${index + 1} 启用开关`}
                    />
                    <span className="font-mono text-xs font-bold text-base-content">
            {card.enabled ? "启用" : "禁用"}
          </span>
                </div>
                <div className="flex items-center gap-1">
                    <Button variant="ghost" size="sm" onClick={onTestPlay} title="测试播放" data-icon="inline-start">
                        <RiPlayLine className="size-4" aria-hidden="true"/>
                        测试
                    </Button>
                    <Button variant="ghost" size="sm" onClick={onRemove} title="删除卡片" data-icon="inline-start">
                        <RiDeleteBinLine className="size-4 text-error" aria-hidden="true"/>
                    </Button>
                </div>
            </div>

            <div className="space-y-3 p-3">
                <FieldGroup>
                    <Field>
                        <FieldLabel>卡片名称</FieldLabel>
                        <FieldContent>
                            <Input
                                value={card.name}
                                onChange={(e) => onUpdate({name: e.target.value})}
                                placeholder="输入卡片名称..."
                            />
                        </FieldContent>
                    </Field>

                    <Field>
                        <FieldLabel>触发模式</FieldLabel>
                        <FieldContent>
                            <Select value={card.triggerMode}
                                    onValueChange={(v) => onUpdate({triggerMode: v as "hotkey" | "regionWatch" | "colorWatch"})}>
                                <SelectTrigger>
                                    <SelectValue/>
                                </SelectTrigger>
                                <SelectContent>
                                    <SelectItem value="hotkey">快捷键触发</SelectItem>
                                    <SelectItem value="regionWatch">区域监听+图像匹配</SelectItem>
                                    <SelectItem value="colorWatch">多区域识色</SelectItem>
                                </SelectContent>
                            </Select>
                        </FieldContent>
                    </Field>
                </FieldGroup>

                {isHotkey && (
                    <FieldGroup>
                        <Field>
                            <FieldLabel>快捷键</FieldLabel>
                            <FieldContent>
                                <Input
                                    value={card.hotkey}
                                    onChange={(e) => onUpdate({hotkey: e.target.value})}
                                    placeholder="输入快捷键，如 Ctrl+F1..."
                                />
                            </FieldContent>
                        </Field>
                    </FieldGroup>
                )}

                {isRegion && (
                    <FieldGroup>
                        <Field>
                            <FieldLabel>监听区域</FieldLabel>
                            <FieldContent>
                                <div className="flex items-center gap-2">
                                    <Button variant="secondary" size="sm" onClick={() => onBeginRegionSelection()}
                                            data-icon="inline-start">
                                        <RiVolumeUpLine className="size-4" aria-hidden="true"/>
                                        {card.watchRegion ? "重新框选" : "框选区域"}
                                    </Button>
                                    {card.watchRegion && (
                                        <Button variant="ghost" size="sm" onClick={onTestMatch}
                                                data-icon="inline-start">
                                            <RiPlayLine className="size-4" aria-hidden="true"/>
                                            实时匹配测试
                                        </Button>
                                    )}
                                    {card.watchRegion && (
                                        <Badge variant="outline" className="font-mono text-xs">
                                            {card.watchRegion.x},{card.watchRegion.y} / {card.watchRegion.width}x{card.watchRegion.height}
                                        </Badge>
                                    )}
                                </div>
                            </FieldContent>
                        </Field>
                        <Field>
                            <FieldLabel>参考图像路径</FieldLabel>
                            <FieldContent>
                                <div className="flex items-center gap-2">
                                    <Input
                                        className="flex-1"
                                        value={card.watchReferenceImagePath}
                                        onChange={(e) => onUpdate({watchReferenceImagePath: e.target.value})}
                                        placeholder="参考图像文件路径..."
                                    />
                                    <Button
                                        variant="secondary"
                                        size="sm"
                                        onClick={onPickReferenceImage}
                                        disabled={!isNativeShell}
                                        title={isNativeShell ? "浏览图像文件" : "仅在桌面端可用"}
                                        data-icon="inline-start"
                                    >
                                        <RiFolderOpenLine className="size-4" aria-hidden="true"/>
                                        浏览...
                                    </Button>
                                </div>
                                {previewUrl && (
                                    <div className="mt-2 border border-base-300 bg-base-200 p-1">
                                        <img
                                            src={previewUrl}
                                            alt="参考图像预览"
                                            className="max-h-32 max-w-48 object-contain"
                                        />
                                    </div>
                                )}
                                {previewLoading && (
                                    <p className="mt-1 text-xs text-base-content/60">加载预览中...</p>
                                )}
                            </FieldContent>
                        </Field>
                        <Field>
                            <FieldLabel>匹配阈值</FieldLabel>
                            <FieldContent>
                                <Input
                                    type="number"
                                    min={0}
                                    max={1}
                                    step={0.01}
                                    value={card.watchMatchThreshold}
                                    onChange={(e) => onUpdate({watchMatchThreshold: e.target.value})}
                                />
                            </FieldContent>
                        </Field>
                        <Field>
                            <FieldLabel>检查间隔 (ms)</FieldLabel>
                            <FieldContent>
                                <Input
                                    type="number"
                                    min={100}
                                    max={10000}
                                    step={100}
                                    value={card.watchPollIntervalMs}
                                    onChange={(e) => onUpdate({watchPollIntervalMs: e.target.value})}
                                    title="每隔多久截图比对一次"
                                />
                            </FieldContent>
                        </Field>
                    </FieldGroup>
                )}

                {isColor && (
                    <FieldGroup>
                        <Field>
                            <FieldLabel>匹配方式</FieldLabel>
                            <FieldContent>
                                <Select
                                    value={card.colorMatchMethod}
                                    onValueChange={(v) => onUpdate({colorMatchMethod: v as "average" | "anyPixel"})}
                                >
                                    <SelectTrigger>
                                        <SelectValue/>
                                    </SelectTrigger>
                                    <SelectContent>
                                        <SelectItem value="average">识色1 · 区域平均色</SelectItem>
                                        <SelectItem value="anyPixel">识色2 · 单像素命中</SelectItem>
                                    </SelectContent>
                                </Select>
                            </FieldContent>
                        </Field>
                        <Field>
                            <FieldLabel>聚合模式</FieldLabel>
                            <FieldContent>
                                <Select
                                    value={card.colorMatchMode}
                                    onValueChange={(v) => onUpdate({colorMatchMode: v as "all" | "any"})}
                                >
                                    <SelectTrigger>
                                        <SelectValue/>
                                    </SelectTrigger>
                                    <SelectContent>
                                        <SelectItem value="all">全部命中才触发</SelectItem>
                                        <SelectItem value="any">任一命中即触发</SelectItem>
                                    </SelectContent>
                                </Select>
                            </FieldContent>
                        </Field>

                        {card.colorProbes.map((probe, probeIndex) => (
                            <div key={probeIndex} className="border border-base-300 p-2 space-y-2">
                                <div className="flex items-center justify-between">
                                    <span className="font-mono text-xs font-bold text-primary">
                                        探针 #{probeIndex + 1}
                                    </span>
                                    <Button
                                        variant="ghost"
                                        size="sm"
                                        onClick={() => onRemoveColorProbe(probeIndex)}
                                        title="删除探针"
                                        data-icon="inline-start"
                                    >
                                        <RiDeleteBinLine className="size-4 text-error" aria-hidden="true"/>
                                        删除
                                    </Button>
                                </div>
                                <Field>
                                    <FieldLabel>监听区域</FieldLabel>
                                    <FieldContent>
                                        <div className="flex items-center gap-2">
                                            <Button
                                                variant="secondary"
                                                size="sm"
                                                onClick={() => onBeginRegionSelection(probeIndex)}
                                                data-icon="inline-start"
                                            >
                                                <RiVolumeUpLine className="size-4" aria-hidden="true"/>
                                                {probe.region ? "重新框选" : "框选区域"}
                                            </Button>
                                            {probe.region && (
                                                <Badge variant="outline" className="font-mono text-xs">
                                                    {probe.region.x},{probe.region.y} / {probe.region.width}x{probe.region.height}
                                                </Badge>
                                            )}
                                        </div>
                                    </FieldContent>
                                </Field>
                                {/* Issue #65：探针内多目标颜色子列表 */}
                                <Field>
                                    <FieldLabel>目标颜色（探针内聚合：{probe.probeMatchMode === "all" ? "全部命中" : "任一命中"}）</FieldLabel>
                                    <FieldContent>
                                        <div className="space-y-2">
                                            {probe.targets.map((target, targetIndex) => (
                                                <div key={targetIndex} className="flex items-center gap-2">
                                                    <input
                                                        type="color"
                                                        value={target.color}
                                                        onChange={(e) => {
                                                            const nextTargets = probe.targets.map((t, i) =>
                                                                i === targetIndex ? {...t, color: e.target.value} : t
                                                            );
                                                            onUpdateColorProbe(probeIndex, {targets: nextTargets});
                                                        }}
                                                        className="h-9 w-12 cursor-pointer border border-base-300 bg-transparent p-0"
                                                        aria-label="目标颜色"
                                                    />
                                                    <Input
                                                        className="flex-1 font-mono"
                                                        value={target.color}
                                                        onChange={(e) => {
                                                            const nextTargets = probe.targets.map((t, i) =>
                                                                i === targetIndex ? {...t, color: e.target.value} : t
                                                            );
                                                            onUpdateColorProbe(probeIndex, {targets: nextTargets});
                                                        }}
                                                        placeholder="#RRGGBB"
                                                    />
                                                    <Input
                                                        type="number"
                                                        min={0}
                                                        max={255}
                                                        step={1}
                                                        className="w-20 font-mono"
                                                        value={target.tolerance}
                                                        onChange={(e) => {
                                                            const nextTargets = probe.targets.map((t, i) =>
                                                                i === targetIndex ? {...t, tolerance: e.target.value} : t
                                                            );
                                                            onUpdateColorProbe(probeIndex, {targets: nextTargets});
                                                        }}
                                                        title="RGB 欧氏距离阈值，越小越严格"
                                                    />
                                                    <Button
                                                        variant="ghost"
                                                        size="sm"
                                                        onClick={() => {
                                                            const nextTargets = probe.targets.filter((_, i) => i !== targetIndex);
                                                            onUpdateColorProbe(probeIndex, {targets: nextTargets});
                                                        }}
                                                        title="删除此目标颜色"
                                                        disabled={probe.targets.length <= 1}
                                                    >
                                                        <RiDeleteBinLine className="size-4 text-error" aria-hidden="true"/>
                                                    </Button>
                                                </div>
                                            ))}
                                            <Button
                                                variant="ghost"
                                                size="sm"
                                                onClick={() => {
                                                    const nextTargets = [...probe.targets, {color: "#00ff00", tolerance: "30"}];
                                                    onUpdateColorProbe(probeIndex, {targets: nextTargets});
                                                }}
                                                data-icon="inline-start"
                                            >
                                                <RiCheckLine className="size-4" aria-hidden="true"/>
                                                添加颜色
                                            </Button>
                                        </div>
                                    </FieldContent>
                                </Field>
                                <Field>
                                    <FieldLabel>探针内聚合模式</FieldLabel>
                                    <FieldContent>
                                        <Select
                                            value={probe.probeMatchMode}
                                            onValueChange={(v) => onUpdateColorProbe(probeIndex, {probeMatchMode: v as "all" | "any"})}
                                        >
                                            <SelectTrigger className="w-full">
                                                <SelectValue/>
                                            </SelectTrigger>
                                            <SelectContent>
                                                <SelectItem value="any">任一命中即触发</SelectItem>
                                                <SelectItem value="all">全部命中才触发</SelectItem>
                                            </SelectContent>
                                        </Select>
                                    </FieldContent>
                                </Field>
                            </div>
                        ))}

                        <Button
                            variant="secondary"
                            size="sm"
                            onClick={onAddColorProbe}
                            data-icon="inline-start"
                        >
                            <RiCheckLine className="size-4" aria-hidden="true"/>
                            新增探针
                        </Button>

                        {card.colorProbes.length > 0 && (
                            <Button variant="ghost" size="sm" onClick={onTestColorMatch} data-icon="inline-start">
                                <RiPlayLine className="size-4" aria-hidden="true"/>
                                实时识色测试
                            </Button>
                        )}

                        <Field>
                            <FieldLabel>检查间隔 (ms)</FieldLabel>
                            <FieldContent>
                                <Input
                                    type="number"
                                    min={100}
                                    max={10000}
                                    step={100}
                                    value={card.watchPollIntervalMs}
                                    onChange={(e) => onUpdate({watchPollIntervalMs: e.target.value})}
                                    title="每隔多久截图比对一次"
                                />
                            </FieldContent>
                        </Field>
                    </FieldGroup>
                )}

                <FieldGroup>
                    <Field>
                        <FieldLabel>播放方式</FieldLabel>
                        <FieldContent>
                            <Select
                                value={card.playMode}
                                onValueChange={(v) => onUpdate({playMode: v as AudioCard["playMode"]})}
                            >
                                <SelectTrigger className="w-full">
                                    <SelectValue/>
                                </SelectTrigger>
                                <SelectContent>
                                    <SelectItem value="single">单文件</SelectItem>
                                    <SelectItem value="combo">连杀（窗口内顺序递增）</SelectItem>
                                    <SelectItem value="random">随机（不重复上一次）</SelectItem>
                                </SelectContent>
                            </Select>
                        </FieldContent>
                    </Field>
                    {card.playMode === "combo" && (
                        <Field>
                            <FieldLabel>默认连杀窗口 (ms)</FieldLabel>
                            <FieldContent>
                                <Input
                                    type="number"
                                    min={100}
                                    max={600000}
                                    step={1000}
                                    value={card.comboWindowMs}
                                    onChange={(e) => onUpdate({comboWindowMs: e.target.value})}
                                    title="每段音频可在下方单独设置窗口；留空的段回退到此默认值"
                                />
                            </FieldContent>
                        </Field>
                    )}
                    <Field>
                        <FieldLabel>
                            音频文件
                            {card.playMode === "combo" && "（顺序即连杀顺序）"}
                            {card.playMode === "random" && "（至少 2 个）"}
                        </FieldLabel>
                        <FieldContent>
                            <div className="flex flex-col gap-2">
                                {(card.audioFiles ?? []).length === 0 ? (
                                    <p className="text-xs text-base-content/60">尚未添加音频文件。</p>
                                ) : (
                                    <ul className="flex flex-col gap-1">
                                        {(card.audioFiles ?? []).map((file, fileIndex) => (
                                            <li
                                                key={`${file}-${fileIndex}`}
                                                className="flex items-center gap-2 border border-base-300 bg-base-100 px-2 py-1"
                                            >
                                                <span className="flex-1 truncate font-mono text-xs text-base-content" title={file}>
                                                    {file}
                                                </span>
                                                {card.playMode === "combo" && (
                                                    <div className="flex items-center gap-1" title="播完此段后用此窗口判断是否播放下一段（空=用卡片默认窗口）">
                                                        <span className="font-mono text-xs text-base-content/60">窗口</span>
                                                        <Input
                                                            type="number"
                                                            min={100}
                                                            max={600000}
                                                            step={1000}
                                                            className="h-7 w-24 font-mono text-xs"
                                                            value={(card.comboWindows ?? [])[fileIndex] ?? ""}
                                                            onChange={(e) => onUpdateComboWindow(fileIndex, e.target.value)}
                                                            placeholder={card.comboWindowMs}
                                                        />
                                                    </div>
                                                )}
                                                <Button
                                                    variant="ghost"
                                                    size="sm"
                                                    className="h-7 w-7 p-0"
                                                    onClick={() => onMoveAudioFile(fileIndex, -1)}
                                                    disabled={fileIndex === 0}
                                                    title="上移"
                                                    data-icon="inline-start"
                                                >
                                                    <RiArrowUpLine className="size-4" aria-hidden="true"/>
                                                </Button>
                                                <Button
                                                    variant="ghost"
                                                    size="sm"
                                                    className="h-7 w-7 p-0"
                                                    onClick={() => onMoveAudioFile(fileIndex, 1)}
                                                    disabled={fileIndex === (card.audioFiles ?? []).length - 1}
                                                    title="下移"
                                                    data-icon="inline-start"
                                                >
                                                    <RiArrowDownLine className="size-4" aria-hidden="true"/>
                                                </Button>
                                                <Button
                                                    variant="ghost"
                                                    size="sm"
                                                    className="h-7 w-7 p-0"
                                                    onClick={() => onRemoveAudioFile(fileIndex)}
                                                    title="移除"
                                                    data-icon="inline-start"
                                                >
                                                    <RiDeleteBinLine className="size-4" aria-hidden="true"/>
                                                </Button>
                                            </li>
                                        ))}
                                    </ul>
                                )}
                                <Button
                                    variant="secondary"
                                    size="sm"
                                    onClick={onPickAudioFile}
                                    disabled={!isNativeShell}
                                    title={isNativeShell ? "浏览并添加音频文件（可多选）" : "仅在桌面端可用"}
                                    data-icon="inline-start"
                                >
                                    <RiFolderOpenLine className="size-4" aria-hidden="true"/>
                                    添加音频文件...
                                </Button>
                            </div>
                        </FieldContent>
                    </Field>
                    <Field>
                        <FieldLabel>音量</FieldLabel>
                        <FieldContent>
                            <Input
                                type="number"
                                min={0}
                                max={1}
                                step={0.1}
                                value={card.volume}
                                onChange={(e) => onUpdate({volume: e.target.value})}
                            />
                        </FieldContent>
                    </Field>
                    <Field>
                        <FieldLabel>触发冷却 (ms)</FieldLabel>
                        <FieldContent>
                            <Input
                                type="number"
                                min={0}
                                max={60000}
                                step={100}
                                value={card.cooldownMs}
                                onChange={(e) => onUpdate({cooldownMs: e.target.value})}
                                title="匹配成功后多久内不重复触发"
                            />
                        </FieldContent>
                    </Field>
                    <Field>
                        <FieldLabel>允许同时播放</FieldLabel>
                        <FieldContent>
                            <Switch
                                checked={card.allowSimultaneous}
                                onCheckedChange={(checked) => onUpdate({allowSimultaneous: checked})}
                                title="开启后此卡片音频可与其他卡片同时播放（默认互斥）"
                            />
                        </FieldContent>
                    </Field>
                </FieldGroup>
            </div>
        </div>
    );
}

function cardToForm(card: AudioCard): AudioSettingsForm["cards"][number] {
    return {
        id: card.id,
        name: card.name,
        enabled: card.enabled,
        triggerMode: card.triggerMode,
        hotkey: card.hotkey ?? "",
        watchRegion: card.watchRegion,
        watchReferenceImagePath: card.watchReferenceImagePath ?? "",
        watchMatchThreshold: String(card.watchMatchThreshold),
        watchPollIntervalMs: String(card.watchPollIntervalMs),
        audioFiles: card.audioFiles ?? [],
        playMode: card.playMode ?? "single",
        comboWindowMs: String(card.comboWindowMs ?? 60000),
        volume: String(card.volume),
        cooldownMs: String(card.cooldownMs),
        allowSimultaneous: card.allowSimultaneous ?? false,
        colorProbes: (card.colorProbes ?? []).map((p) => ({
            region: p.region,
            targets: (p.targets ?? []).map((t) => ({
                color: rgbToHex(t.color),
                tolerance: String(t.tolerance),
            })),
            probeMatchMode: p.probeMatchMode ?? "any",
        })),
        colorMatchMode: card.colorMatchMode ?? "all",
        colorMatchMethod: card.colorMatchMethod ?? "average",
    };
}

export function AudioRegionOverlay() {
    const params = useMemo(() => new URLSearchParams(window.location.search), []);
    const cardId = params.get("audio_card") ?? "";
    // 识色模式探针框选时透传的探针索引；区域监听模式为 null
    const probeIndex = useMemo(() => {
        const raw = params.get("probe_index");
        if (raw === null) return undefined;
        const parsed = Number.parseInt(raw, 10);
        return Number.isNaN(parsed) ? undefined : parsed;
    }, [params]);

    const [dragStart, setDragStart] = useState<Point | null>(null);
    const [dragCurrent, setDragCurrent] = useState<Point | null>(null);
    // 松开后固定的已选区域（等待确认/重选）
    const [committedRect, setCommittedRect] = useState<{
        x: number;
        y: number;
        width: number;
        height: number
    } | null>(null);
    const [statusMessage, setStatusMessage] = useState("拖拽鼠标框选监听区域，松开后按 Enter 确认，Esc 取消。");
    const [submitting, setSubmitting] = useState(false);

    // 拖拽中的实时矩形
    const currentRect = useMemo(() => {
        if (!dragStart || !dragCurrent) return null;
        return getSelectionRect(dragStart, dragCurrent);
    }, [dragStart, dragCurrent]);

    const cancelSelection = useCallback(async () => {
        if (submitting) return;
        setSubmitting(true);
        setStatusMessage("正在取消...");
        try {
            await invoke("audio_overlay_cancel_selection", {cardId});
        } catch (error) {
            setStatusMessage(getErrorMessage(error));
            setSubmitting(false);
        }
    }, [cardId, submitting]);

    const submitSelection = useCallback(async () => {
        const rect = committedRect;
        if (!rect || submitting) return;
        if (rect.width <= MIN_SELECTION_WIDTH || rect.height <= MIN_SELECTION_HEIGHT) {
            setStatusMessage(`区域太小（${rect.width}x${rect.height}），请重新框选。`);
            return;
        }
        setSubmitting(true);
        setStatusMessage("正在提交...");
        try {
            await invoke("audio_overlay_submit_selection", {cardId, probeIndex, region: rect});
        } catch (error) {
            setStatusMessage(getErrorMessage(error));
            setSubmitting(false);
        }
    }, [cardId, probeIndex, committedRect, submitting]);

    useEffect(() => {
        const handleKeyDown = (event: KeyboardEvent) => {
            if (event.key === "Escape") {
                event.preventDefault();
                void cancelSelection();
            } else if (event.key === "Enter" && committedRect && !submitting) {
                event.preventDefault();
                void submitSelection();
            }
        };
        window.addEventListener("keydown", handleKeyDown);
        return () => window.removeEventListener("keydown", handleKeyDown);
    }, [cancelSelection, committedRect, submitSelection, submitting]);

    const handleMouseDown = (event: React.MouseEvent<HTMLDivElement>) => {
        if (submitting || event.button !== 0) return;
        // 重新框选：清除已提交矩形
        setCommittedRect(null);
        const point = {x: event.clientX, y: event.clientY};
        setDragStart(point);
        setDragCurrent(point);
        setStatusMessage("正在框选...");
    };

    const handleMouseMove = (event: React.MouseEvent<HTMLDivElement>) => {
        if (!dragStart || submitting) return;
        setDragCurrent({x: event.clientX, y: event.clientY});
    };

    const handleMouseUp = () => {
        if (!dragStart || submitting) return;
        const rect = currentRect;
        // 清除拖拽状态，固定矩形
        setDragStart(null);
        setDragCurrent(null);

        if (rect && (rect.width <= MIN_SELECTION_WIDTH || rect.height <= MIN_SELECTION_HEIGHT)) {
            setCommittedRect(null);
            setStatusMessage(`区域太小（${rect.width}x${rect.height}），请重新框选。`);
            return;
        }
        if (rect) {
            setCommittedRect(rect);
            setStatusMessage("区域已框选，按 Enter 确认或重新拖拽框选，Esc 取消。");
        }
    };

    // 最终显示的矩形：拖拽中用实时矩形，松开后用已提交矩形
    const displayRect = currentRect ?? committedRect;

    return (
        <div
            className="fixed inset-0 cursor-crosshair select-none text-white"
            onContextMenu={(event) => {
                event.preventDefault();
                void cancelSelection();
            }}
            onMouseDown={handleMouseDown}
            onMouseMove={handleMouseMove}
            onMouseUp={handleMouseUp}
        >
            {displayRect && (
                <div
                    className="pointer-events-none absolute border border-primary bg-primary/16"
                    style={{
                        left: displayRect.x,
                        top: displayRect.y,
                        width: displayRect.width,
                        height: displayRect.height,
                    }}
                />
            )}

            <div
                className="pointer-events-none absolute left-6 top-6 max-w-md border border-white/40 bg-base-100/88 px-4 py-4 text-base-content backdrop-blur-md">
                <h1 className="text-lg font-semibold text-base-content">音频区域选择</h1>
                <p className="mt-2 text-sm text-base-content/60">{statusMessage}</p>
                {displayRect && (
                    <p className="mt-3 border border-base-300 bg-base-200/80 px-3 py-2 font-mono text-xs text-base-content/60">
                        {`X ${displayRect.x} · Y ${displayRect.y} · W ${displayRect.width} · H ${displayRect.height}`}
                    </p>
                )}
            </div>

            <div
                className="absolute right-6 top-6 flex items-center gap-2 border border-white/30 bg-base-100/80 px-3 py-3 backdrop-blur-md">
                <Button
                    disabled={!committedRect || submitting}
                    onClick={() => void submitSelection()}
                    type="button"
                    variant="secondary"
                    data-icon="inline-start"
                >
                    <RiCheckLine className="size-4" aria-hidden="true"/>
                    确认
                </Button>
                <Button
                    disabled={submitting}
                    onClick={() => void cancelSelection()}
                    type="button"
                    variant="secondary"
                    data-icon="inline-start"
                >
                    <RiCloseLine className="size-4" aria-hidden="true"/>
                    取消
                </Button>
            </div>
        </div>
    );
}
