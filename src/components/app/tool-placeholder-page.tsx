import {RiAlertLine, RiRoadMapLine} from "@remixicon/react";

import {Badge} from "@/components/ui/badge";
import {Empty, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle,} from "@/components/ui/empty";
import {AppPage, CardBody, MacroHeader, SectionHeader, TacticalCard} from "@/components/app/app-ui";

type ToolPlaceholderPageProps = {
    title: string;
    shortLabel: string;
    description: string;
};

const pendingItems = [
    "接入对应命令与任务编排接口。",
    "补充页面级参数与校验规则。",
    "连通真实数据后开放联动配置。",
];

export function ToolPlaceholderPage({title, shortLabel, description}: ToolPlaceholderPageProps) {
    return (
        <AppPage>
            <MacroHeader
                code="--"
                title={title}
                subtitle={description}
                badges={
                    <>
                        <Badge variant="secondary">未开放</Badge>
                        <Badge variant="outline">{shortLabel}</Badge>
                    </>
                }
            />

            <div className="col-span-12 grid gap-3 xl:grid-cols-12">
                <TacticalCard className="xl:col-span-8 p-0">
                    <CardBody className="flex min-h-72 items-center justify-center">
                        <Empty
                            className="min-h-52 border border-dashed border-base-300 bg-base-200 px-6 py-8 text-center">
                            <EmptyHeader>
                                <EmptyMedia variant="icon">
                                    <RiAlertLine/>
                                </EmptyMedia>
                                <EmptyTitle>{shortLabel} 模块暂未开放</EmptyTitle>
                                <EmptyDescription>当前只保留菜单入口，页面内容将在后续接入。</EmptyDescription>
                            </EmptyHeader>
                        </Empty>
                    </CardBody>
                </TacticalCard>

                <TacticalCard className="xl:col-span-4 p-0">
                    <SectionHeader
                        eyebrow="准备清单"
                        icon={<RiRoadMapLine/>}
                        title="准备项"
                        description="保持桌面工具壳层，等待模块补全。"
                    />
                    <CardBody>
                        <ul className="grid gap-px border border-base-300 bg-base-content">
                            {pendingItems.map((item) => (
                                <li key={item}
                                    className="bg-base-100 px-3 py-3 font-mono text-[0.68rem] font-bold leading-relaxed text-base-content">
                                    {item}
                                </li>
                            ))}
                        </ul>
                    </CardBody>
                </TacticalCard>
            </div>
        </AppPage>
    );
}
