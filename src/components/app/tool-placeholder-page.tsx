import { RiAlertLine, RiRoadMapLine } from "@remixicon/react";

import { Badge } from "@/components/ui/badge";
import {
  Empty,
  EmptyDescription,
  EmptyHeader,
  EmptyMedia,
  EmptyTitle,
} from "@/components/ui/empty";
import { AppPage, CardBody, PageHero, SectionHeader, TacticalCard } from "@/components/app/app-ui";

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

export function ToolPlaceholderPage({ title, shortLabel, description }: ToolPlaceholderPageProps) {
  return (
    <AppPage>
      <PageHero
        eyebrow="Utility Module"
        title={title}
        description={description}
        badges={
          <>
            <Badge variant="secondary">未开放</Badge>
            <Badge variant="outline">{shortLabel}</Badge>
          </>
        }
      />

      <div className="grid flex-1 gap-5 xl:grid-cols-[minmax(0,1.1fr)_320px]">
        <TacticalCard className="min-h-72">
          <CardBody className="flex h-full items-center justify-center">
            <Empty className="border-[var(--surface-border)] bg-[linear-gradient(145deg,var(--surface-tile),color-mix(in_oklch,var(--card)_34%,transparent))]">
              <EmptyHeader>
                <EmptyMedia variant="icon">
                  <RiAlertLine />
                </EmptyMedia>
                <EmptyTitle>{shortLabel} 模块暂未开放</EmptyTitle>
                <EmptyDescription>当前只保留菜单入口，页面内容将在后续接入。</EmptyDescription>
              </EmptyHeader>
            </Empty>
          </CardBody>
        </TacticalCard>

        <TacticalCard>
          <SectionHeader
            eyebrow="Roadmap"
            icon={<RiRoadMapLine />}
            title="准备项"
            description="保持桌面工具壳层，等待模块补全。"
          />
          <CardBody>
            <ul className="grid gap-2 text-xs/relaxed text-muted-foreground">
              {pendingItems.map((item) => (
                <li key={item} className="rounded-lg border border-[var(--surface-border)] bg-[linear-gradient(145deg,var(--surface-tile),color-mix(in_oklch,var(--card)_38%,transparent))] px-3 py-2.5">
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
