import {useCallback, useEffect, useMemo, useState} from "react";
import {invoke} from "@tauri-apps/api/core";
import {open} from "@tauri-apps/plugin-dialog";
import {AUDIO_EVENTS, listenEvent} from "@/lib/tauri-events";
import {RiCheckLine, RiCloseLine, RiDeleteBinLine, RiFolderOpenLine, RiPlayLine, RiVolumeUpLine,} from "@remixicon/react";
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
import type {AudioBootstrap, AudioCard, AudioSettings, AudioSettingsForm} from "@/components/app/audio-types";
import {AUDIO_AUTOSAVE_DELAY_MS} from "@/components/app/audio-types";
import {
    createEmptyAudioCard,
    mergeAudioWatchRegionsIntoForm,
    parseSettingsForm,
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

        void listenEvent(AUDIO_EVENTS.stateChanged, (event) => {
            if (disposed) return;
            const next = event.payload;
            setBootstrap(next);
            setForm((current) => mergeAudioWatchRegionsIntoForm(current, next.settings));
            setPageError(null);
        }).then((dispose) => {
            unlistenStateChanged = dispose;
        });

        void listenEvent(AUDIO_EVENTS.hotkeyError, (event) => {
            if (disposed) return;
            setPageError(event.payload);
            setStatusMessage(event.payload);
            toast.error(event.payload);
        }).then((dispose) => {
            unlistenHotkeyError = dispose;
        });

        return () => {
            disposed = true;
            unlistenStateChanged?.();
            unlistenHotkeyError?.();
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

    const handleTestPlay = useCallback(
        async (cardId: string) => {
            if (!isNativeShell) return;
            try {
                await invoke("audio_test_play", {cardId});
                toast.success("播放测试已触发");
            } catch (error) {
                toast.error(getErrorMessage(error));
            }
        },
        [isNativeShell],
    );

    const handleTestMatch = useCallback(
        async (cardId: string) => {
            if (!isNativeShell) return;
            try {
                type TestMatchResult = { similarity: number; triggered: boolean; matchPosition: { x: number; y: number } | null };
                const result = await invoke<TestMatchResult>("audio_test_match", {cardId});
                const pos = result.matchPosition ? ` (位置: ${result.matchPosition.x}, ${result.matchPosition.y})` : "";
                toast.success(
                    `匹配度: ${(result.similarity * 100).toFixed(1)}% ${result.triggered ? "(已触发)" : "(未触发)"}${pos}`
                );
            } catch (error) {
                toast.error(getErrorMessage(error));
            }
        },
        [isNativeShell],
    );

    const handleBeginRegionSelection = useCallback(
        async (cardId: string) => {
            if (!isNativeShell) return;
            try {
                await invoke("audio_begin_region_selection", {cardId});
            } catch (error) {
                toast.error(getErrorMessage(error));
            }
        },
        [isNativeShell],
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
                const path = await open({
                    multiple: false,
                    directory: false,
                    filters: [{name: "音频文件", extensions: ["mp3", "wav", "ogg", "flac", "aac", "m4a", "wma"]}],
                });
                if (path && typeof path === "string") {
                    handleUpdateCard(index, {audioFilePath: path});
                }
            } catch (error) {
                toast.error(getErrorMessage(error));
            }
        },
        [isNativeShell, handleUpdateCard],
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
                    className="col-span-12 mb-3 border-2 border-[var(--alert-red)] bg-[var(--alert-red)]/10 px-3 py-2 font-mono text-xs font-black tracking-[0.12em] text-[var(--alert-red)] uppercase">
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
                                className="font-mono text-xs font-black tracking-[0.14em] uppercase text-[var(--chalk)]">
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
                            onBeginRegionSelection={() => handleBeginRegionSelection(card.id)}
                            onPickReferenceImage={() => handlePickReferenceImage(index)}
                            onPickAudioFile={() => handlePickAudioFile(index)}
                            onLoadReferencePreview={() => handleLoadReferencePreview(card.id)}
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
                             onLoadReferencePreview,
                         }: {
    card: AudioSettingsForm["cards"][number];
    index: number;
    isNativeShell: boolean;
    onUpdate: (patch: Partial<AudioSettingsForm["cards"][number]>) => void;
    onRemove: () => void;
    onTestPlay: () => void;
    onTestMatch: () => void;
    onBeginRegionSelection: () => void;
    onPickReferenceImage: () => void;
    onPickAudioFile: () => void;
    onLoadReferencePreview: () => Promise<string | null>;
}) {
    const isHotkey = card.triggerMode === "hotkey";
    const isRegion = card.triggerMode === "regionWatch";

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
        <div className="border-2 border-[var(--chalk)] bg-[var(--slate)]">
            <div
                className="flex items-center justify-between border-b-2 border-[var(--chalk)] bg-[var(--carbon)] px-3 py-2">
                <div className="flex items-center gap-2">
                    <span
                        className="font-mono text-xs font-black text-[var(--amber)]">A-{String(index + 1).padStart(2, "0")}</span>
                    <Switch
                        checked={card.enabled}
                        onCheckedChange={(v) => onUpdate({enabled: v})}
                        aria-label={`卡片 ${index + 1} 启用开关`}
                    />
                    <span className="font-mono text-xs font-bold tracking-[0.12em] uppercase text-[var(--chalk)]">
            {card.enabled ? "启用" : "禁用"}
          </span>
                </div>
                <div className="flex items-center gap-1">
                    <Button variant="ghost" size="sm" onClick={onTestPlay} title="测试播放" data-icon="inline-start">
                        <RiPlayLine className="size-4" aria-hidden="true"/>
                        测试
                    </Button>
                    <Button variant="ghost" size="sm" onClick={onRemove} title="删除卡片" data-icon="inline-start">
                        <RiDeleteBinLine className="size-4 text-[var(--alert-red)]" aria-hidden="true"/>
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
                                    onValueChange={(v) => onUpdate({triggerMode: v as "hotkey" | "regionWatch"})}>
                                <SelectTrigger>
                                    <SelectValue/>
                                </SelectTrigger>
                                <SelectContent>
                                    <SelectItem value="hotkey">快捷键触发</SelectItem>
                                    <SelectItem value="regionWatch">区域监听+图像匹配</SelectItem>
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
                                    <Button variant="secondary" size="sm" onClick={onBeginRegionSelection}
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
                                    <div className="mt-2 border border-[var(--seam)] bg-[var(--slate)] p-1">
                                        <img
                                            src={previewUrl}
                                            alt="参考图像预览"
                                            className="max-h-32 max-w-48 object-contain"
                                        />
                                    </div>
                                )}
                                {previewLoading && (
                                    <p className="mt-1 text-xs text-[var(--zinc)]">加载预览中...</p>
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

                <FieldGroup>
                    <Field>
                        <FieldLabel>音频文件路径</FieldLabel>
                        <FieldContent>
                            <div className="flex items-center gap-2">
                                <Input
                                    className="flex-1"
                                    value={card.audioFilePath}
                                    onChange={(e) => onUpdate({audioFilePath: e.target.value})}
                                    placeholder="音频文件绝对路径..."
                                />
                                <Button
                                    variant="secondary"
                                    size="sm"
                                    onClick={onPickAudioFile}
                                    disabled={!isNativeShell}
                                    title={isNativeShell ? "浏览音频文件" : "仅在桌面端可用"}
                                    data-icon="inline-start"
                                >
                                    <RiFolderOpenLine className="size-4" aria-hidden="true"/>
                                    浏览...
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
        audioFilePath: card.audioFilePath,
        volume: String(card.volume),
        cooldownMs: String(card.cooldownMs),
        allowSimultaneous: card.allowSimultaneous ?? false,
    };
}

export function AudioRegionOverlay() {
    const params = useMemo(() => new URLSearchParams(window.location.search), []);
    const cardId = params.get("audio_card") ?? "";

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
            await invoke("audio_overlay_submit_selection", {cardId, region: rect});
        } catch (error) {
            setStatusMessage(getErrorMessage(error));
            setSubmitting(false);
        }
    }, [cardId, committedRect, submitting]);

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
                    className="pointer-events-none absolute border-2 border-[var(--amber)] bg-[var(--amber)]/16"
                    style={{
                        left: displayRect.x,
                        top: displayRect.y,
                        width: displayRect.width,
                        height: displayRect.height,
                    }}
                />
            )}

            <div
                className="pointer-events-none absolute left-6 top-6 max-w-md border-2 border-white/40 bg-[var(--carbon)]/88 px-4 py-4 text-[var(--chalk)] backdrop-blur-md">
                <h1 className="text-lg font-semibold text-[var(--chalk)]">音频区域选择</h1>
                <p className="mt-2 text-sm text-[var(--zinc)]">{statusMessage}</p>
                {displayRect && (
                    <p className="mt-3 border border-[var(--seam)] bg-[var(--slate)]/80 px-3 py-2 font-mono text-xs text-[var(--zinc)]">
                        {`X ${displayRect.x} · Y ${displayRect.y} · W ${displayRect.width} · H ${displayRect.height}`}
                    </p>
                )}
            </div>

            <div
                className="absolute right-6 top-6 flex items-center gap-2 border-2 border-white/30 bg-[var(--carbon)]/80 px-3 py-3 backdrop-blur-md">
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
