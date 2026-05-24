import type { TokenStatus } from "@/components/app/delta-types";
import { Badge } from "@/components/ui/badge";

const statusConfig: Record<TokenStatus, { label: string; variant: "secondary" | "outline" | "destructive" }> = {
  valid: { label: "令牌有效", variant: "secondary" },
  expiring_soon: { label: "即将过期", variant: "outline" },
  expired: { label: "已过期", variant: "destructive" },
  none: { label: "无令牌", variant: "outline" },
};

type TokenBadgeProps = {
  status: TokenStatus;
  label?: string;
};

export function TokenBadge({ status, label }: TokenBadgeProps) {
  const config = statusConfig[status];
  return (
    <Badge variant={config.variant} className="gap-1.5 text-[0.62rem]">
      <span
        className="inline-block size-1.5 shrink-0 rounded-full"
        data-token-status={status}
      />
      {label ?? config.label}
    </Badge>
  );
}
