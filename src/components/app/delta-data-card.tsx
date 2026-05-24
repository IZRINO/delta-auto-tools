import type { ReactNode } from "react";
import { TacticalCard, SectionHeader, CardBody } from "@/components/app/app-ui";
import { Spinner } from "@/components/ui/spinner";

type DeltaDataCardProps = {
  eyebrow?: string;
  title: string;
  icon?: ReactNode;
  badge?: ReactNode;
  loading?: boolean;
  error?: string | null;
  onRetry?: () => void;
  children?: ReactNode;
};

export function DeltaDataCard({ eyebrow, title, icon, badge, loading, error, onRetry, children }: DeltaDataCardProps) {
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
          <div className="flex flex-col items-center gap-2 py-6 text-sm">
            <span className="text-destructive">{error}</span>
            {onRetry && (
              <button
                type="button"
                onClick={onRetry}
                className="text-xs text-primary underline underline-offset-2 hover:text-primary/80"
              >
                重试
              </button>
            )}
          </div>
        )}
        {!loading && !error && children}
      </CardBody>
    </TacticalCard>
  );
}
