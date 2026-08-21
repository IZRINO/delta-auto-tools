import {DisplaySettingsInline} from "@/components/app/app-ui";
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
                <DisplaySettingsInline
                    key={group.id}
                    canDelete={canDelete(group.id)}
                    controlsDisabled={controlsDisabled}
                    display={group.display}
                    group={{enabled: group.enabled, name: group.name}}
                    onGroupDelete={() => onGroupDelete(group.id)}
                    onGroupUpdate={(value) => onGroupUpdate(group.id, value)}
                    onPositionSelection={() => onPositionSelection(group.id)}
                    onUpdate={(value) => onUpdateDisplay(group.id, value)}
                    onUpdateRect={(value) => onUpdateRect(group.id, value)}
                    statusMessage={`${group.enabled ? "分组已启用" : "分组已关闭"} · ${effectiveCount(group.id)} 张有效卡片`}
                    targetLabel={targetLabel}
                />
            ))}
        </div>
    );
}
