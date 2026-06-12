import type { DeltaAccountRecord } from "@/components/app/delta-types";
import { ACCOUNT_KIND_LABELS, CAPABILITY_LABELS } from "@/components/app/delta-types";
import { getCapabilities, getAccountDisplayName, getTokenStatus, getTokenStatusLabel } from "@/components/app/delta-utils";
import { Badge } from "@/components/ui/badge";
import { TacticalCard } from "@/components/app/app-ui";
import { cn } from "@/lib/utils";
import { TokenBadge } from "@/components/app/delta-token-badge";

type DeltaAccountCardProps = {
  account: DeltaAccountRecord;
  selected: boolean;
  onSelect: (id: number) => void;
};

export function DeltaAccountCard({ account, selected, onSelect }: DeltaAccountCardProps) {
  const tokenStatus = getTokenStatus(account);
  const tokenLabel = getTokenStatusLabel(tokenStatus, account.expiresAt);
  const capabilities = getCapabilities(account.kind);
  const isExpired = tokenStatus === "expired";


  return (
    <TacticalCard
      active={selected}
      className={cn(
        "relative cursor-pointer p-0 transition-colors",
        isExpired && "border-[var(--amber)]",
      )}
      onClick={() => onSelect(account.id)}
    >
      {isExpired && (
        <div className="pointer-events-none absolute right-2 top-1/2 -translate-y-1/2 rotate-12 select-none">
          <span className="block border-2 border-[var(--amber)] px-2 py-0.5 font-mono text-[0.7rem] font-black tracking-[0.15em] text-[var(--amber)] uppercase opacity-80">已过期</span>
        </div>
      )}
      <div className="grid gap-px bg-[var(--chalk)]">
        <div className="grid gap-px bg-[var(--chalk)] sm:grid-cols-[9rem_minmax(0,1fr)]">
          <div className="bg-[var(--chalk)] px-3 py-3 font-mono text-[0.62rem] font-black tracking-[0.18em] text-[var(--carbon)] uppercase">
            账号档案 {String(account.id).padStart(2, "0")}
          </div>
          <div className="bg-[var(--carbon)] px-3 py-3">
            <div className="flex min-w-0 flex-wrap items-center gap-2">
              <Badge variant="outline" className="shrink-0 font-mono text-[0.62rem]">
                {ACCOUNT_KIND_LABELS[account.kind]}
              </Badge>
              <span className="min-w-0 truncate text-sm font-black uppercase text-[var(--chalk)]">
                {getAccountDisplayName(account)}
              </span>
            </div>
          </div>
        </div>

        <div className="bg-[var(--slate)] px-3 py-3">
          <div className="flex flex-wrap items-center gap-2">
            <TokenBadge status={tokenStatus} label={tokenLabel} />
            {capabilities.map((cap) => (
              <Badge key={cap} variant="secondary" className="text-[0.58rem]">
                {CAPABILITY_LABELS[cap]}
              </Badge>
            ))}
          </div>
          <p className="mt-3 font-mono text-[0.62rem] font-black tracking-[0.12em] text-[var(--zinc)] uppercase">
            {selected ? "当前路由已锁定此账号" : "点击载入此账号，右键查看更多命令"}
          </p>
        </div>
      </div>
    </TacticalCard>
  );
}
