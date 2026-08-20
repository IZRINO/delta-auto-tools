import {memo, useEffect, useState} from "react";
import {RiAddLine, RiArrowDownLine, RiArrowUpLine, RiCheckLine, RiDeleteBinLine, RiFolderOpenLine, RiPlayLine, RiVolumeUpLine} from "@remixicon/react";

import {Badge} from "@/components/ui/badge";
import {Button} from "@/components/ui/button";
import {Field, FieldContent, FieldGroup, FieldLabel} from "@/components/ui/field";
import {Input} from "@/components/ui/input";
import {Select, SelectContent, SelectItem, SelectTrigger, SelectValue} from "@/components/ui/select";
import {Switch} from "@/components/ui/switch";
import {ToggleGroup, ToggleGroupItem} from "@/components/ui/toggle-group";
import {HotkeyField, SurfaceToggleGroup} from "@/components/app/app-ui";
import type {RecognitionCardAction} from "@/components/app/recognition-card-reducer";
import type {ColorProbeForm, RecognitionCard, RecognitionGroup, RecognitionSettingsForm} from "@/components/app/recognition-types";

export type RecognitionRecordingTarget = {
    cardId: string;
    field: "triggerHotkey" | "activationHotkey" | "effectHotkey";
    stepIndex?: number;
} | null;

export type RecognitionCardEditorAdapter = {
    moveToGroup: (cardId: string, groupId: string | null) => void;
    moveWithinGroup: (groupId: string, cardId: string, delta: -1 | 1) => void;
    testPlay: (cardId: string) => void;
    testMatch: (cardId: string) => void;
    beginRegionSelection: (cardId: string, probeIndex?: number, selectionTarget?: string) => void;
    pickReferenceImages: (cardId: string) => void;
    pickAudioFile: (cardId: string) => void;
    loadReferencePreview: (path: string) => Promise<string | null>;
    testColorMatch: (cardId: string) => void;
    beginHotkeyRecording: (
        card: RecognitionSettingsForm["cards"][number],
        field: NonNullable<RecognitionRecordingTarget>["field"],
        stepIndex?: number,
    ) => void;
    hotkeyKeyDown: (
        card: RecognitionSettingsForm["cards"][number],
        field: NonNullable<RecognitionRecordingTarget>["field"],
        stepIndex: number | undefined,
        event: React.KeyboardEvent<HTMLButtonElement>,
    ) => void;
    hotkeyRecorderBlur: () => void;
};

export const RECOGNITION_HOTKEY_HELPER_TEXT = "支持字母、数字、F1-F24、方向键及 , . ; / \\ [ ] - = + ` ' 等符号";

function normalizedGroupId(groupId: string | null | undefined): string {
    return groupId?.trim() || "default-recognition-group";
}

function recordingKey(target: RecognitionRecordingTarget, cardId: string): string {
    return target?.cardId === cardId ? `${target.field}:${target.stepIndex ?? ""}` : "";
}

function ReferenceImageRow({
                               path,
                               imageIndex,
                               isNativeShell,
                               onChange,
                               onRemove,
                               onLoadPreview,
                           }: {
    path: string;
    imageIndex: number;
    isNativeShell: boolean;
    onChange: (path: string) => void;
    onRemove: () => void;
    onLoadPreview: (path: string) => Promise<string | null>;
}) {
    const [previewUrl, setPreviewUrl] = useState<string | null>(null);
    const [previewLoading, setPreviewLoading] = useState(false);

    useEffect(() => {
        if (!path.trim() || !isNativeShell) {
            setPreviewUrl(null);
            setPreviewLoading(false);
            return;
        }
        let disposed = false;
        setPreviewLoading(true);
        void onLoadPreview(path).then((url) => {
            if (!disposed) {
                setPreviewUrl(url);
                setPreviewLoading(false);
            }
        });
        return () => {
            disposed = true;
        };
    }, [path, isNativeShell, onLoadPreview]);

    return (
        <div className="border border-base-300 bg-base-100 p-2">
            <div className="flex items-center gap-2">
                <span className="w-8 shrink-0 font-mono text-xs text-base-content/60">
                    {imageIndex + 1}
                </span>
                <Input
                    className="flex-1"
                    value={path}
                    onChange={(event) => onChange(event.target.value)}
                    placeholder="参考图像文件路径..."
                />
                <Button variant="ghost" size="sm" onClick={onRemove} title="删除参考图像"
                        aria-label="删除参考图像" data-icon="inline-start">
                    <RiDeleteBinLine className="size-4 text-error" aria-hidden="true"/>
                </Button>
            </div>
            {previewUrl && (
                <div className="mt-2 border border-base-300 bg-base-200 p-1">
                    <img
                        src={previewUrl}
                        alt={`参考图像 ${imageIndex + 1} 预览`}
                        className="max-h-32 max-w-48 object-contain"
                    />
                </div>
            )}
            {previewLoading && (
                <p className="mt-1 text-xs text-base-content/60">加载预览中...</p>
            )}
        </div>
    );
}

export const RecognitionCardEditor = memo(function RecognitionCardEditor({
                             card,
                             index,
                             position,
                             groupSize,
                             cardGroups,
                             collapsed,
                             isNativeShell,
                             dispatch,
                             adapter,
                             recordingTarget,
                          }: {
    card: RecognitionSettingsForm["cards"][number];
    index: number;
    position: number;
    groupSize: number;
    cardGroups: RecognitionGroup[];
    collapsed: boolean;
    isNativeShell: boolean;
    dispatch: (action: RecognitionCardAction) => void;
    adapter: RecognitionCardEditorAdapter;
    recordingTarget: RecognitionRecordingTarget;
}) {
    const updateCard = (update: (current: RecognitionSettingsForm["cards"][number]) => RecognitionSettingsForm["cards"][number]) => {
        dispatch({type: "update", cardId: card.id, update});
    };
    const onUpdate = (patch: Partial<RecognitionSettingsForm["cards"][number]>) => {
        dispatch({type: "patch", cardId: card.id, patch});
    };
    const onMoveToGroup = (groupId: string | null) => adapter.moveToGroup(card.id, groupId);
    const onRemove = () => dispatch({type: "remove", cardId: card.id});
    const onMoveUp = () => adapter.moveWithinGroup(normalizedGroupId(card.groupId), card.id, -1);
    const onMoveDown = () => adapter.moveWithinGroup(normalizedGroupId(card.groupId), card.id, 1);
    const onTestPlay = () => adapter.testPlay(card.id);
    const onTestMatch = () => adapter.testMatch(card.id);
    const onBeginRegionSelection = (probeIndex?: number) => adapter.beginRegionSelection(card.id, probeIndex);
    const onBeginCustomClickSelection = () => adapter.beginRegionSelection(card.id, undefined, "customClick");
    const onAddReferenceImage = () => updateCard((current) => {
        const existing = current.watchReferenceImagePaths
            ?? (current.watchReferenceImagePath ? [current.watchReferenceImagePath] : []);
        return {...current, watchReferenceImagePaths: [...existing, ""]};
    });
    const onPickReferenceImages = () => adapter.pickReferenceImages(card.id);
    const onUpdateReferenceImage = (imageIndex: number, path: string) => updateCard((current) => {
        const paths = [...(current.watchReferenceImagePaths ?? [])];
        paths[imageIndex] = path;
        return {...current, watchReferenceImagePaths: paths};
    });
    const onRemoveReferenceImage = (imageIndex: number) => updateCard((current) => ({
        ...current,
        watchReferenceImagePaths: (current.watchReferenceImagePaths ?? []).filter((_, index) => index !== imageIndex),
    }));
    const onPickAudioFile = () => adapter.pickAudioFile(card.id);
    const onRemoveAudioFile = (fileIndex: number) => updateCard((current) => ({
        ...current,
        audioFiles: (current.audioFiles ?? []).filter((_, index) => index !== fileIndex),
    }));
    const onMoveAudioFile = (fileIndex: number, direction: -1 | 1) => updateCard((current) => {
        const files = [...(current.audioFiles ?? [])];
        const target = fileIndex + direction;
        if (target < 0 || target >= files.length) return current;
        [files[fileIndex], files[target]] = [files[target], files[fileIndex]];
        return {...current, audioFiles: files};
    });
    const onUpdateComboWindow = (fileIndex: number, value: string) => updateCard((current) => ({
        ...current,
        comboWindows: (current.audioFiles ?? []).map((_, index) =>
            index === fileIndex ? value : String((current.comboWindows ?? [])[index] ?? ""),
        ),
    }));
    const onLoadReferencePreview = adapter.loadReferencePreview;
    const onTestColorMatch = () => adapter.testColorMatch(card.id);
    const onAddColorProbe = () => updateCard((current) => ({
        ...current,
        colorProbes: [...current.colorProbes, {
            region: null,
            targets: [{color: "#ff0000", tolerance: "30"}],
            probeMatchMode: "any",
        }],
    }));
    const onRemoveColorProbe = (probeIndex: number) => updateCard((current) => ({
        ...current,
        colorProbes: current.colorProbes.filter((_, index) => index !== probeIndex),
    }));
    const onUpdateColorProbe = (probeIndex: number, patch: Partial<ColorProbeForm>) => {
        dispatch({type: "patchProbe", cardId: card.id, probeIndex, patch});
    };
    const onBeginHotkeyRecording = (
        field: NonNullable<RecognitionRecordingTarget>["field"],
        stepIndex?: number,
    ) => adapter.beginHotkeyRecording(card, field, stepIndex);
    const onHotkeyKeyDown = (
        field: NonNullable<RecognitionRecordingTarget>["field"],
        stepIndex: number | undefined,
        event: React.KeyboardEvent<HTMLButtonElement>,
    ) => adapter.hotkeyKeyDown(card, field, stepIndex, event);
    const onHotkeyRecorderBlur = adapter.hotkeyRecorderBlur;

    const isHotkey = card.triggerMode === "hotkey";
    const isRegion = card.triggerMode === "regionWatch";
    const isColor = card.triggerMode === "colorWatch";
    const audioEffectEnabled = card.audioEffectEnabled ?? true;
    const hotkeyEffectEnabled = card.hotkeyEffectEnabled ?? false;
    const clickEffectEnabled = card.clickEffectEnabled ?? false;
    const hotkeySteps = card.hotkeyEffectSteps?.length
        ? card.hotkeyEffectSteps
        : [{hotkey: card.effectHotkey ?? "", delayMs: "0"}];
    const referenceImagePaths = card.watchReferenceImagePaths
        ?? (card.watchReferenceImagePath ? [card.watchReferenceImagePath] : []);

    return (
        <div className="border border-base-300 bg-base-200">
            <div
                className="flex items-center justify-between border-b-2 border-base-content bg-base-100 px-3 py-2">
                <div className="flex items-center gap-2">
                    <span
                        className="font-mono text-xs font-semibold text-base-content">A-{String(index + 1).padStart(2, "0")}</span>
                    <span className="max-w-48 truncate font-mono text-xs font-bold text-base-content">
                        {card.name || "未命名卡片"}
                    </span>
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
                    <Button
                        variant="ghost"
                        size="sm"
                        disabled={position === 0}
                        onClick={onMoveUp}
                        title="上移卡片"
                        aria-label="上移卡片"
                    >
                        <RiArrowUpLine className="size-4" aria-hidden="true"/>
                    </Button>
                    <Button
                        variant="ghost"
                        size="sm"
                        disabled={position === groupSize - 1}
                        onClick={onMoveDown}
                        title="下移卡片"
                        aria-label="下移卡片"
                    >
                        <RiArrowDownLine className="size-4" aria-hidden="true"/>
                    </Button>
                    <Button variant="ghost" size="sm" onClick={onTestPlay} title="测试播放" data-icon="inline-start">
                        <RiPlayLine className="size-4" aria-hidden="true"/>
                        测试
                    </Button>
                    <Button variant="ghost" size="sm" onClick={onRemove} title="删除卡片" data-icon="inline-start">
                        <RiDeleteBinLine className="size-4 text-error" aria-hidden="true"/>
                    </Button>
                </div>
            </div>

            {!collapsed && <div className="space-y-3 p-3">
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
                        <FieldLabel>分组</FieldLabel>
                        <FieldContent>
                            <Select
                                value={normalizedGroupId(card.groupId)}
                                onValueChange={(value) => onMoveToGroup(value)}
                            >
                                <SelectTrigger>
                                    <SelectValue/>
                                </SelectTrigger>
                                <SelectContent>
                                    {cardGroups.map((group) => (
                                        <SelectItem key={group.id} value={group.id}>
                                            {group.name}
                                        </SelectItem>
                                    ))}
                                </SelectContent>
                            </Select>
                        </FieldContent>
                    </Field>

                    <Field>
                        <FieldLabel>识别来源</FieldLabel>
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
                        <HotkeyField
                            controlsDisabled={!isNativeShell}
                            helperText={RECOGNITION_HOTKEY_HELPER_TEXT}
                            hotkey={card.hotkey}
                            id={`${card.id}-trigger-hotkey`}
                            isRecording={recordingTarget?.cardId === card.id && recordingTarget.field === "triggerHotkey"}
                            onBeginHotkeyRecording={() => onBeginHotkeyRecording("triggerHotkey")}
                            onHotkeyKeyDown={(event) => onHotkeyKeyDown("triggerHotkey", undefined, event)}
                            onHotkeyRecorderBlur={onHotkeyRecorderBlur}
                        />
                        <Field>
                            <FieldLabel>快捷键触发方式</FieldLabel>
                            <FieldContent>
                                <SurfaceToggleGroup>
                                    <ToggleGroup
                                        className="w-full"
                                        type="single"
                                        value={card.hotkeyRepeatMode ?? "once"}
                                        variant="outline"
                                        onValueChange={(value) => value
                                            ? onUpdate({hotkeyRepeatMode: value as "once" | "whileHeld"})
                                            : undefined}
                                    >
                                        <ToggleGroupItem
                                            className="min-w-24 flex-1 border-base-content font-mono text-sm font-semibold data-[state=on]:bg-base-content data-[state=on]:text-base-100"
                                            value="once"
                                        >
                                            按下触发一次
                                        </ToggleGroupItem>
                                        <ToggleGroupItem
                                            className="min-w-24 flex-1 border-base-content font-mono text-sm font-semibold data-[state=on]:bg-base-content data-[state=on]:text-base-100"
                                            value="whileHeld"
                                        >
                                            按住持续触发
                                        </ToggleGroupItem>
                                    </ToggleGroup>
                                </SurfaceToggleGroup>
                            </FieldContent>
                        </Field>
                    </FieldGroup>
                )}

                {!isHotkey && (
                    <FieldGroup>
                        <Field>
                            <FieldLabel>激活方式</FieldLabel>
                            <FieldContent>
                                <Select
                                    value={card.activationMode ?? "always"}
                                    onValueChange={(v) => onUpdate({activationMode: v as "always" | "onceHotkey" | "timedHotkey"})}
                                >
                                    <SelectTrigger>
                                        <SelectValue/>
                                    </SelectTrigger>
                                    <SelectContent>
                                        <SelectItem value="always">持续识别</SelectItem>
                                        <SelectItem value="onceHotkey">按快捷键识别一次</SelectItem>
                                        <SelectItem value="timedHotkey">按快捷键限时识别</SelectItem>
                                    </SelectContent>
                                </Select>
                            </FieldContent>
                        </Field>
                        {(card.activationMode ?? "always") === "always" && (
                            <Field>
                                <FieldLabel>重复触发</FieldLabel>
                                <FieldContent>
                                    <label className="flex items-center gap-2 border border-base-300 bg-base-100 px-2 py-2 text-xs font-medium">
                                        <Switch
                                            checked={card.retriggerAfterDisappear ?? false}
                                            onCheckedChange={(checked) => onUpdate({retriggerAfterDisappear: checked})}
                                        />
                                        目标消失后再触发
                                    </label>
                                </FieldContent>
                            </Field>
                        )}
                        {(card.activationMode ?? "always") !== "always" && (
                            <HotkeyField
                                controlsDisabled={!isNativeShell}
                                helperText={RECOGNITION_HOTKEY_HELPER_TEXT}
                                hotkey={card.activationHotkey ?? ""}
                                id={`${card.id}-activation-hotkey`}
                                isRecording={recordingTarget?.cardId === card.id && recordingTarget.field === "activationHotkey"}
                                onBeginHotkeyRecording={() => onBeginHotkeyRecording("activationHotkey")}
                                onHotkeyKeyDown={(event) => onHotkeyKeyDown("activationHotkey", undefined, event)}
                                onHotkeyRecorderBlur={onHotkeyRecorderBlur}
                            />
                        )}
                        {(card.activationMode ?? "always") === "timedHotkey" && (
                            <>
                            <Field>
                                <FieldLabel>限时时长 (ms)</FieldLabel>
                                <FieldContent>
                                    <Input
                                        type="number"
                                        min={100}
                                        max={600000}
                                        step={1000}
                                        value={card.activationDurationMs ?? "10000"}
                                        onChange={(e) => onUpdate({activationDurationMs: e.target.value})}
                                    />
                                </FieldContent>
                            </Field>
                            <Field>
                                <FieldLabel>触发次数</FieldLabel>
                                <FieldContent>
                                    <Input
                                        type="number"
                                        min={1}
                                        max={1000}
                                        step={1}
                                        value={card.activationTriggerCount ?? "1"}
                                        onChange={(e) => onUpdate({activationTriggerCount: e.target.value})}
                                    />
                                </FieldContent>
                            </Field>
                            </>
                        )}
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
                            <FieldLabel>参考图像</FieldLabel>
                            <FieldContent>
                                <div className="space-y-2">
                                    {referenceImagePaths.map((path, imageIndex) => (
                                        <ReferenceImageRow
                                            key={imageIndex}
                                            path={path}
                                            imageIndex={imageIndex}
                                            isNativeShell={isNativeShell}
                                            onChange={(value) => onUpdateReferenceImage(imageIndex, value)}
                                            onRemove={() => onRemoveReferenceImage(imageIndex)}
                                            onLoadPreview={onLoadReferencePreview}
                                        />
                                    ))}
                                    <div className="flex gap-2">
                                        <Button variant="secondary" size="sm" onClick={onAddReferenceImage}
                                                data-icon="inline-start">
                                            <RiAddLine className="size-4" aria-hidden="true"/>
                                            添加路径
                                        </Button>
                                        <Button
                                            variant="secondary"
                                            size="sm"
                                            onClick={onPickReferenceImages}
                                            disabled={!isNativeShell}
                                            title={isNativeShell ? "浏览图像文件，可多选" : "仅在桌面端可用"}
                                            data-icon="inline-start"
                                        >
                                            <RiFolderOpenLine className="size-4" aria-hidden="true"/>
                                            浏览图像...
                                        </Button>
                                    </div>
                                </div>
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
                                    <span className="font-mono text-xs font-bold text-base-content">
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
                        <FieldLabel>触发效果</FieldLabel>
                        <FieldContent>
                            <div className="grid gap-2 sm:grid-cols-3">
                                <label className="flex items-center gap-2 border border-base-300 bg-base-100 px-2 py-2 text-xs font-medium">
                                    <Switch
                                        checked={audioEffectEnabled}
                                        onCheckedChange={(checked) => onUpdate({audioEffectEnabled: checked})}
                                    />
                                    播放音频
                                </label>
                                <label className="flex items-center gap-2 border border-base-300 bg-base-100 px-2 py-2 text-xs font-medium">
                                    <Switch
                                        checked={hotkeyEffectEnabled}
                                        onCheckedChange={(checked) => onUpdate({hotkeyEffectEnabled: checked})}
                                    />
                                    按快捷键
                                </label>
                                <label className="flex items-center gap-2 border border-base-300 bg-base-100 px-2 py-2 text-xs font-medium">
                                    <Switch
                                        checked={clickEffectEnabled}
                                        onCheckedChange={(checked) => onUpdate({clickEffectEnabled: checked})}
                                    />
                                    点击
                                </label>
                            </div>
                        </FieldContent>
                    </Field>

                    {hotkeyEffectEnabled && (
                        <Field>
                            <FieldLabel>按键序列</FieldLabel>
                            <FieldContent>
                                <div className="space-y-2">
                                    {hotkeySteps.map((step, stepIndex) => (
                                        <div key={stepIndex} className="flex items-center gap-2">
                                            <HotkeyField
                                                controlsDisabled={!isNativeShell}
                                                helperText={RECOGNITION_HOTKEY_HELPER_TEXT}
                                                hotkey={step.hotkey}
                                                id={`${card.id}-effect-hotkey-${stepIndex}`}
                                                isRecording={recordingTarget?.cardId === card.id && recordingTarget.field === "effectHotkey" && recordingTarget.stepIndex === stepIndex}
                                                onBeginHotkeyRecording={() => onBeginHotkeyRecording("effectHotkey", stepIndex)}
                                                onHotkeyKeyDown={(event) => onHotkeyKeyDown("effectHotkey", stepIndex, event)}
                                                onHotkeyRecorderBlur={onHotkeyRecorderBlur}
                                            />
                                            <Input
                                                className="w-28 font-mono"
                                                type="number"
                                                min={0}
                                                max={600000}
                                                step={50}
                                                value={step.delayMs}
                                                onChange={(e) => {
                                                    const next = hotkeySteps.map((item, index) => index === stepIndex ? {...item, delayMs: e.target.value} : item);
                                                    onUpdate({hotkeyEffectSteps: next});
                                                }}
                                            />
                                            <Button
                                                variant="ghost"
                                                size="sm"
                                                disabled={hotkeySteps.length <= 1}
                                                onClick={() => {
                                                    const next = hotkeySteps.filter((_, index) => index !== stepIndex);
                                                    onUpdate({
                                                        effectHotkey: next[0]?.hotkey ?? "",
                                                        hotkeyEffectSteps: next,
                                                    });
                                                }}
                                            >
                                                <RiDeleteBinLine className="size-4 text-error" aria-hidden="true"/>
                                            </Button>
                                        </div>
                                    ))}
                                    <Button
                                        variant="secondary"
                                        size="sm"
                                        onClick={() => onUpdate({hotkeyEffectSteps: [...hotkeySteps, {hotkey: "", delayMs: "0"}]})}
                                        data-icon="inline-start"
                                    >
                                        <RiCheckLine className="size-4" aria-hidden="true"/>
                                        添加按键
                                    </Button>
                                </div>
                            </FieldContent>
                        </Field>
                    )}

                    {clickEffectEnabled && (
                        <>
                            <Field>
                                <FieldLabel>点击目标</FieldLabel>
                                <FieldContent>
                                    <Select
                                        value={card.clickMode ?? "customRegion"}
                                        onValueChange={(v) => onUpdate({clickMode: v as "customRegion" | "recognitionRegion"})}
                                    >
                                        <SelectTrigger>
                                            <SelectValue/>
                                        </SelectTrigger>
                                        <SelectContent>
                                            <SelectItem value="customRegion">自定义区域中心</SelectItem>
                                            <SelectItem value="recognitionRegion" disabled={isHotkey}>识别命中中心</SelectItem>
                                        </SelectContent>
                                    </Select>
                                </FieldContent>
                            </Field>
                            {(card.clickMode ?? "customRegion") === "customRegion" ? (
                                <Field>
                                    <FieldLabel>自定义点击区域</FieldLabel>
                                    <FieldContent>
                                        <div className="flex items-center gap-2">
                                            <Button
                                                variant="secondary"
                                                size="sm"
                                                onClick={onBeginCustomClickSelection}
                                                data-icon="inline-start"
                                            >
                                                <RiVolumeUpLine className="size-4" aria-hidden="true"/>
                                                {card.clickCustomRegion ? "重新框选" : "框选区域"}
                                            </Button>
                                            {card.clickCustomRegion && (
                                                <Badge variant="outline" className="font-mono text-xs">
                                                    {card.clickCustomRegion.x},{card.clickCustomRegion.y} / {card.clickCustomRegion.width}x{card.clickCustomRegion.height}
                                                </Badge>
                                            )}
                                        </div>
                                    </FieldContent>
                                </Field>
                            ) : isColor ? (
                                <Field>
                                    <FieldLabel>识色点击探针</FieldLabel>
                                    <FieldContent>
                                        <Select
                                            value={card.clickColorProbeIndex ?? ""}
                                            onValueChange={(value) => onUpdate({clickColorProbeIndex: value})}
                                        >
                                            <SelectTrigger>
                                                <SelectValue placeholder="选择探针"/>
                                            </SelectTrigger>
                                            <SelectContent>
                                                {card.colorProbes.map((_, probeIndex) => (
                                                    <SelectItem key={probeIndex} value={String(probeIndex)}>
                                                        探针 #{probeIndex + 1}
                                                    </SelectItem>
                                                ))}
                                            </SelectContent>
                                        </Select>
                                    </FieldContent>
                                </Field>
                            ) : null}
                        </>
                    )}
                </FieldGroup>

                {audioEffectEnabled && (
                    <FieldGroup>
                        <Field>
                            <FieldLabel>播放方式</FieldLabel>
                            <FieldContent>
                                <Select
                                    value={card.playMode}
                                    onValueChange={(v) => onUpdate({playMode: v as RecognitionCard["playMode"]})}
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
                )}

                <FieldGroup>
                    <Field>
                        <FieldLabel>触发冷却 (ms)</FieldLabel>
                        <FieldContent>
                            <Input
                                type="number"
                                min={isHotkey && (card.hotkeyRepeatMode ?? "once") === "whileHeld" ? 10 : 0}
                                max={60000}
                                step={100}
                                value={card.cooldownMs}
                                onChange={(e) => onUpdate({cooldownMs: e.target.value})}
                                title="匹配成功后多久内不重复触发"
                            />
                        </FieldContent>
                    </Field>
                </FieldGroup>
            </div>}
        </div>
    );
}, (previous, next) =>
    previous.card === next.card
    && previous.index === next.index
    && previous.position === next.position
    && previous.groupSize === next.groupSize
    && previous.cardGroups === next.cardGroups
    && previous.collapsed === next.collapsed
    && previous.isNativeShell === next.isNativeShell
    && previous.dispatch === next.dispatch
    && previous.adapter === next.adapter
    && recordingKey(previous.recordingTarget, previous.card.id)
        === recordingKey(next.recordingTarget, next.card.id));
