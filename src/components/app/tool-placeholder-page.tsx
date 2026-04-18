import { RiAlertLine, RiRoadMapLine } from "@remixicon/react";

import { Badge } from "@/components/ui/badge";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import {
  Empty,
  EmptyDescription,
  EmptyHeader,
  EmptyMedia,
  EmptyTitle,
} from "@/components/ui/empty";

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
    <div className="flex flex-1 flex-col gap-3">
      <div className="desktop-toolbar">
        <div className="min-w-0">
          <p className="desktop-caption">Utility Module</p>
          <div className="flex flex-wrap items-center gap-2">
            <h1 className="text-base font-semibold tracking-tight">{title}</h1>
            <Badge variant="secondary">未开放</Badge>
          </div>
          <p className="mt-1 text-xs text-muted-foreground">{description}</p>
        </div>
        <Badge variant="outline">{shortLabel}</Badge>
      </div>

      <div className="grid flex-1 gap-3 xl:grid-cols-[minmax(0,1.1fr)_300px]">
        <Card className="min-h-72" size="sm">
          <CardContent className="flex h-full items-center justify-center">
            <Empty className="border-border bg-muted/20">
              <EmptyHeader>
                <EmptyMedia variant="icon">
                  <RiAlertLine />
                </EmptyMedia>
                <EmptyTitle>{shortLabel} 模块暂未开放</EmptyTitle>
                <EmptyDescription>当前只保留菜单入口，页面内容将在后续接入。</EmptyDescription>
              </EmptyHeader>
            </Empty>
          </CardContent>
        </Card>

        <Card size="sm">
          <CardHeader>
            <div className="flex items-center gap-2">
              <RiRoadMapLine className="text-muted-foreground" />
              <CardTitle>准备项</CardTitle>
            </div>
            <CardDescription>保持桌面工具壳层，等待模块补全。</CardDescription>
          </CardHeader>
          <CardContent>
            <ul className="grid gap-2 text-xs/relaxed text-muted-foreground">
              {pendingItems.map((item) => (
                <li key={item} className="desktop-subpanel px-3 py-2.5">
                  {item}
                </li>
              ))}
            </ul>
          </CardContent>
        </Card>
      </div>
    </div>
  );
}
