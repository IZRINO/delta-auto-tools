import {Button} from "@/components/ui/button";
import {Field, FieldContent, FieldLabel} from "@/components/ui/field";
import {Slider} from "@/components/ui/slider";
import {Input} from "@/components/ui/input";
import {Switch} from "@/components/ui/switch";
import {Collapsible, CollapsibleContent, CollapsibleTrigger} from "@/components/ui/collapsible";
import {RiArrowDownSLine, RiDeleteBinLine, RiMapPinLine} from "@remixicon/react";
import {ControlTile, InlineControl} from "@/components/app/app-ui";
import type {TimerGroupForm} from "@/components/app/timer-types";
import {cn} from "@/lib/utils";

type SyncGroupSectionProps = {
    groups: TimerGroupForm[];
    targetLabel: string;
    controlsDisabled: boolean;
    canDelete: (groupId: string) => boolean;
    effectiveCount: (groupId: string) => number;
    onGroupUpdate: (id: string, value: Partial<TimerGroupForm>) => void;
    onGroupDelete: (id: string) => void;
    onPositionSelection: (groupId: string) => void;
    onUpdateDisplay: (id: string, value: Partial<TimerGroupForm["display"]>) => void;
    onUpdateRect: (id: string, value: Partial<TimerGroupForm["display"]["rect"]>) => void;
    className?: string;
};

/**
 * 同步系统分组列表：封装 DisplaySettingsInline 的重复渲染。
 */
export function SyncGroupSection({
                                     groups,
                                     targetLabel,
                                     controlsDisabled,
                                     canDelete,
                                     effectiveCount,
                                     onGroupUpdate,
                                     onGroupDelete,
                                     onPositionSelection,
                                     onUpdateDisplay,
                                     onUpdateRect,
                                     className,
                                 }: SyncGroupSectionProps) {
    return (
        <div className={cn("col-span-12 flex flex-col gap-2", className)}>
            {groups.map((group) => (
                <ControlTile key={group.id} className="flex flex-col gap-3 bg-[var(--carbon)]">
                    <div className="flex flex-wrap items-center gap-3">
                        <Switch
                            checked={group.enabled}
                            disabled={controlsDisabled}
                            onCheckedChange={(checked) => onGroupUpdate(group.id, {enabled: checked})}
                        />
                        <p className="font-mono text-xs font-medium tracking-[0.12em] text-[var(--chalk)] uppercase">
                            {targetLabel}分组 · {group.name}
                        </p>
                        <Input
                            className="w-28 font-mono text-sm"
                            disabled={controlsDisabled}
                            value={group.name}
                            onChange={(event) => onGroupUpdate(group.id, {name: event.currentTarget.value})}
                            aria-label="分组名称"
                        />
                        <Button
                            disabled={!canDelete(group.id)}
                            onClick={() => onGroupDelete(group.id)}
                            type="button"
                            variant="ghost"
                            className="shrink-0"
                            size="icon-sm"
                        >
                            <RiDeleteBinLine/>
                        </Button>
                        <Button
                            className="shrink-0"
                            disabled={controlsDisabled}
                            onClick={() => onPositionSelection(group.id)}
                            type="button"
                            variant="outline"
                            size="sm"
                        >
                            <RiMapPinLine data-icon="inline-start"/>
                            位置
                        </Button>
                    </div>

                    <Collapsible defaultOpen={false}>
                        <InlineControl className="p-0">
                            <CollapsibleTrigger asChild>
                                <Button
                                    className="w-full justify-between px-2 py-1.5 font-mono text-xs font-medium tracking-[0.12em] uppercase"
                                    type="button"
                                    variant="ghost"
                                >
                                    显示参数
                                    <RiArrowDownSLine className="size-3.5"/>
                                </Button>
                            </CollapsibleTrigger>
                            <CollapsibleContent className="border-t border-[var(--chalk)] px-2 py-2">
                                <div className="flex flex-wrap items-center gap-4">
                                    <Field className="min-w-0 flex-1">
                                        <FieldLabel className="font-mono text-xs">字体透明度</FieldLabel>
                                        <FieldContent>
                                            <div className="flex items-center gap-3">
                                                <Slider
                                                    disabled={controlsDisabled || !group.display}
                                                    min={0.1}
                                                    max={1}
                                                    step={0.05}
                                                    value={[Number.parseFloat(group.display?.fontOpacity ?? "0.9")]}
                                                    onValueChange={([value]) => onUpdateDisplay(group.id, {fontOpacity: value.toFixed(2)})}
                                                />
                                                <span
                                                    className="w-10 text-right font-mono text-xs text-muted-foreground">
                          {group.display?.fontOpacity ?? "--"}
                        </span>
                                            </div>
                                        </FieldContent>
                                    </Field>
                                    <Field className="w-36 shrink-0">
                                        <FieldLabel className="font-mono text-xs">窗口宽度</FieldLabel>
                                        <FieldContent>
                                            <Input
                                                disabled={controlsDisabled || !group.display}
                                                inputMode="numeric"
                                                min="320"
                                                className="h-7 font-mono text-xs"
                                                value={group.display?.rect?.width ?? 320}
                                                onChange={(event) =>
                                                    onUpdateRect(group.id, {width: Number.parseInt(event.currentTarget.value, 10) || 320})
                                                }
                                            />
                                        </FieldContent>
                                    </Field>
                                </div>
                            </CollapsibleContent>
                        </InlineControl>
                    </Collapsible>

                    <p className="font-mono text-xs font-medium tracking-[0.08em] text-muted-foreground uppercase">
                        {group.enabled ? "分组已启用" : "分组已关闭"} · {effectiveCount(group.id)} 张有效卡片
                    </p>
                </ControlTile>
            ))}
        </div>
    );
}
