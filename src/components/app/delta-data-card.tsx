import type { ReactNode } from "react";
import { RiDatabase2Line } from "@remixicon/react";

import { CardBody, InlineNotice, JsonPreBlock, SectionHeader, TacticalCard } from "@/components/app/app-ui";
import { Button } from "@/components/ui/button";
import { Spinner } from "@/components/ui/spinner";
import { cn } from "@/lib/utils";

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
  className?: string;
};

export function DeltaDataCard({ badge, children, className, data, emptyText = "暂无数据", error, eyebrow, icon, loading, onRetry, title }: DeltaDataCardProps) {
  return (
    <TacticalCard className={cn("col-span-12 p-0", className)}>
      <SectionHeader
        eyebrow={eyebrow}
        title={title}
        icon={icon}
        badge={badge}
      />
      <CardBody className="space-y-3">
        {loading && (
          <div className="flex min-h-48 items-center justify-center gap-2 border-2 border-[var(--ink)] bg-[var(--data-well)] px-4 py-6 font-mono text-xs font-bold tracking-[0.08em] text-[var(--paper)] uppercase">
            <Spinner className="size-4" />
            <span>数据载入中...</span>
          </div>
        )}

        {!loading && error && (
          <div className="space-y-3">
            <InlineNotice title="读取失败">{error}</InlineNotice>
            {onRetry ? (
              <Button type="button" variant="outline" size="sm" onClick={onRetry}>
                重试
              </Button>
            ) : null}
          </div>
        )}

        {!loading && !error && data != null ? <JsonPreBlock className="min-h-48" maxHeightClassName="max-h-[28rem]" data={data} /> : null}
        {!loading && !error && data == null && children ? children : null}

        {!loading && !error && data == null && !children ? (
          <div className="flex min-h-40 flex-col items-center justify-center gap-3 border-2 border-dashed border-[var(--ink)] bg-[var(--bone)] px-4 py-8 text-center">
            <RiDatabase2Line className="size-5 text-[var(--alert-red)]" />
            <p className="text-sm font-black uppercase text-[var(--ink)]">暂无数据</p>
            <p className="max-w-xl font-mono text-[0.68rem] font-bold leading-relaxed tracking-[0.08em] text-[var(--steel)] uppercase">{emptyText}</p>
          </div>
        ) : null}
      </CardBody>
    </TacticalCard>
  );
}
