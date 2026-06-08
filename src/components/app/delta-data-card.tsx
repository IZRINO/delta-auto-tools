import type { ReactNode } from "react";
import { RiDatabase2Line } from "@remixicon/react";

import { Button } from "@/components/ui/button";
import { Spinner } from "@/components/ui/spinner";
import { CardBody, InlineNotice, JsonPreBlock, SectionHeader, TacticalCard, TacticalEmptyState } from "@/components/app/app-ui";
type DeltaDataCardProps = {
  eyebrow?: string;
  title: string;
  icon?: ReactNode;
  badge?: ReactNode;
  loading?: boolean;
  error?: string | null;
  emptyText?: string;
  data?: unknown;
  onRetry?: () => void;
  children?: ReactNode;
};

export function DeltaDataCard({ badge, children, data, emptyText = "暂无数据", error, eyebrow, icon, loading, onRetry, title }: DeltaDataCardProps) {
  return (
    <TacticalCard>
      <SectionHeader
        eyebrow={eyebrow}
        title={title}
        icon={icon}
        badge={badge}
      />
      <CardBody>
        {loading && (
          <div className="flex items-center justify-center gap-2 py-8 text-sm text-muted-foreground">
            <Spinner className="size-4" />
            <span>加载中...</span>
          </div>
        )}
        {!loading && error && (
          <div className="space-y-3">
            <InlineNotice>{error}</InlineNotice>
            {onRetry ? (
              <Button type="button" variant="outline" size="sm" onClick={onRetry}>
                重试
              </Button>
            ) : null}
          </div>
        )}
        {!loading && !error && data !== undefined ? <JsonPreBlock data={data} /> : null}
        {!loading && !error && data === undefined && children}
        {!loading && !error && data === undefined && !children ? (
          <TacticalEmptyState className="min-h-36" icon={<RiDatabase2Line />} title="暂无数据" description={emptyText} />
        ) : null}
      </CardBody>
    </TacticalCard>
  );
}
