import type { AccountKind, Capability, TokenStatus, DeltaAccountRecord, GameAuth } from "@/components/app/delta-types";
import { EXPIRING_THRESHOLD_MS, ACCOUNT_KIND_CAPABILITIES } from "@/components/app/delta-types";

export function getTokenStatus(account: DeltaAccountRecord): TokenStatus {
  if (!account.accessToken) return "none";
  if (account.expiresAt === null) return "none";
  const now = Date.now();
  const expires = account.expiresAt * 1000;
  if (expires <= now) return "expired";
  if (expires <= now + EXPIRING_THRESHOLD_MS) return "expiring_soon";
  return "valid";
}

export function getTokenStatusLabel(status: TokenStatus, expiresAt: number | null): string {
  switch (status) {
    case "valid": return "令牌有效";
    case "expiring_soon": {
      if (expiresAt === null) return "即将过期";
      const days = Math.max(0, Math.ceil((expiresAt * 1000 - Date.now()) / 86400000));
      return `即将过期 ${days}天`;
    }
    case "expired": return "已过期";
    case "none": return "无令牌";
  }
}

export function getCapabilities(kind: AccountKind): Capability[] {
  return ACCOUNT_KIND_CAPABILITIES[kind] ?? [];
}

export function canRefreshToken(kind: AccountKind): boolean {
  return kind === "qq" || kind === "wechat" || kind === "pioneer";
}

export function buildGameAuth(account: DeltaAccountRecord): GameAuth | null {
  if (!account.openid || !account.accessToken) return null;
  return {
    openid: account.openid,
    accessToken: account.accessToken,
    acctype: account.kind === "wechat" ? "wx" : "qc",
  };
}

export function buildWegameTicket(account: DeltaAccountRecord): { id: string; ticket: string } | null {
  if (!account.accessToken) return null;
  return {
    id: account.uinOrOpenid,
    ticket: account.accessToken,
  };
}

export function extractQqSafeCode(extraJson: string | null): string | null {
  if (!extraJson) return null;
  try {
    const parsed = JSON.parse(extraJson);
    if (typeof parsed.code === "string" && parsed.code) return parsed.code;
    if (typeof parsed.code === "number") return String(parsed.code);
    return null;
  } catch {
    return null;
  }
}

export function getAccountDisplayName(account: DeltaAccountRecord): string {
  if (account.kind === "qq" || account.kind === "qqSafe" || account.kind === "wegameQq" || account.kind === "pioneer") {
    return account.uinOrOpenid;
  }
  const oid = account.openid ?? account.uinOrOpenid;
  return oid.length > 12 ? `${oid.slice(0, 8)}...` : oid;
}

export function getAcctypeForKind(kind: AccountKind): "qc" | "wx" {
  return kind === "wechat" || kind === "wegameWechat" ? "wx" : "qc";
}
