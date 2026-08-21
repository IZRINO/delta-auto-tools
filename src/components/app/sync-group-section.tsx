import {Button} from "@/components/ui/button";
import {Input} from "@/components/ui/input";
import {Slider} from "@/components/ui/slider";
import {Switch} from "@/components/ui/switch";
import {Collapsible, CollapsibleContent, CollapsibleTrigger} from "@/components/ui/collapsible";
import {RiDeleteBinLine, RiMapPinLine} from "@remixicon/react";
import {ConfigRow, FieldUnit, StampFold} from "@/components/app/app-ui";
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
                <FieldUnit
                    key={group.id}
                    padBody={false}
                    header={(
                        <Input
                            className="h-auto w-full border-0 bg-transparent p-0 font-mono text-xs font-semibold"
                            disabled={controlsDisabled}
                            value={group.name}
                            onChange={(event) => onGroupUpdate(group.id, {name: event.currentTarget.value})}
                            aria-label="分组名称"
                        />
                    )}
                    description={`${targetLabel}分组 · ${group.enabled ? "已启用" : "已关闭"} · ${effectiveCount(group.id)} 张有效卡片`}
                    headerActions={(
                        <>
                            <Switch
                                checked={group.enabled}
                                disabled={controlsDisabled}
                                aria-label={`${targetLabel}分组启用`}
                                onCheckedChange={(checked) => onGroupUpdate(group.id, {enabled: checked})}
                            />
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
                            <Button
                                disabled={!canDelete(group.id)}
                                onClick={() => onGroupDelete(group.id)}
                                type="button"
                                variant="outline"
                                className="shrink-0"
                                size="icon-sm"
                                aria-label="删除分组"
                            >
                                <RiDeleteBinLine/>
                            </Button>
                        </>
                    )}
                >
                    <Collapsible defaultOpen={false}>
                        <CollapsibleTrigger asChild>
                            <StampFold label="显示参数"/>
                        </CollapsibleTrigger>
                        <CollapsibleContent>
                            <ConfigRow
                                label="字体透明度"
                                value={(
                                    <div className="flex w-full items-center gap-3">
                                        <Slider
                                            disabled={controlsDisabled || !group.display}
                                            min={0.1}
                                            max={1}
                                            step={0.05}
                                            value={[Number.parseFloat(group.display?.fontOpacity ?? "0.9")]}
                                            onValueChange={([value]) => onUpdateDisplay(group.id, {fontOpacity: value.toFixed(2)})}
                                        />
                                        <span className="w-10 text-right font-mono text-xs">
                                            {group.display?.fontOpacity ?? "--"}
                                        </span>
                                    </div>
                                )}
                            />
                            <ConfigRow
                                label="窗口宽度"
                                value={(
                                    <Input
                                        disabled={controlsDisabled || !group.display}
                                        inputMode="numeric"
                                        min="320"
                                        className="h-7 w-28 font-mono text-xs"
                                        value={group.display?.rect?.width ?? 320}
                                        onChange={(event) =>
                                            onUpdateRect(group.id, {width: Number.parseInt(event.currentTarget.value, 10) || 320})
                                        }
                                    />
                                )}
                            />
                        </CollapsibleContent>
                    </Collapsible>
                </FieldUnit>
            ))}
        </div>
    );
}
