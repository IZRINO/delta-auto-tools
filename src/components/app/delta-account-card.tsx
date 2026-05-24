import type { DeltaAccountRecord } from "@/components/app/delta-types";
import { ACCOUNT_KIND_LABELS, CAPABILITY_LABELS } from "@/components/app/delta-types";
import { getCapabilities, getAccountDisplayName, getTokenStatus, getTokenStatusLabel } from "@/components/app/delta-utils";
import { Badge } from "@/components/ui/badge";
import { TacticalCard } from "@/components/app/app-ui";
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

  return (
    <TacticalCard
      active={selected}
      className="cursor-pointer transition-all"
      onClick={() => onSelect(account.id)}
    >
      <div className="flex flex-col gap-2 p-4">
        <div className="flex items-center gap-2">
          <Badge variant="outline" className="shrink-0 font-mono text-[0.62rem]">
            {ACCOUNT_KIND_LABELS[account.kind]}
          </Badge>
          <span className="truncate text-sm font-medium text-foreground">
            {getAccountDisplayName(account)}
          </span>
        </div>
        <div className="flex items-center gap-2">
          <TokenBadge status={tokenStatus} label={tokenLabel} />
          {capabilities.map((cap) => (
            <Badge key={cap} variant="secondary" className="text-[0.58rem]">
              {CAPABILITY_LABELS[cap]}
            </Badge>
          ))}
        </div>
      </div>
    </TacticalCard>
  );
}
