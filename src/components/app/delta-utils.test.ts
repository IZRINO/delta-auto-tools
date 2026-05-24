import { describe, it, expect } from "vitest";
import {
  getTokenStatus,
  getTokenStatusLabel,
  getCapabilities,
  canRefreshToken,
  buildGameAuth,
  buildWegameTicket,
  extractQqSafeCode,
  getAccountDisplayName,
  getAcctypeForKind,
} from "@/components/app/delta-utils";
import type { DeltaAccountRecord } from "@/components/app/delta-types";
import { EXPIRING_THRESHOLD_MS } from "@/components/app/delta-types";

const now = Date.now();

function makeAccount(overrides: Partial<DeltaAccountRecord> = {}): DeltaAccountRecord {
  return {
    id: 1,
    kind: "qq",
    uinOrOpenid: "1234567890",
    cookieJson: "{}",
    openid: "openid_xxx",
    accessToken: "token_abc",
    extraJson: null,
    expiresAt: null,
    createdAt: 1700000000,
    updatedAt: 1700000000,
    ...overrides,
  };
}

describe("getTokenStatus", () => {
  it("returns 'none' when accessToken is null", () => {
    expect(getTokenStatus(makeAccount({ accessToken: null }))).toBe("none");
  });

  it("returns 'none' when accessToken is empty string", () => {
    expect(getTokenStatus(makeAccount({ accessToken: "" }))).toBe("none");
  });

  it("returns 'none' when expiresAt is null", () => {
    expect(getTokenStatus(makeAccount({ expiresAt: null }))).toBe("none");
  });

  it("returns 'expired' when expiresAt is in the past", () => {
    const past = Math.floor(now / 1000) - 100;
    expect(getTokenStatus(makeAccount({ expiresAt: past }))).toBe("expired");
  });

  it("returns 'expired' when expiresAt equals now (boundary)", () => {
    const boundary = Math.floor(now / 1000);
    expect(getTokenStatus(makeAccount({ expiresAt: boundary }))).toBe("expired");
  });

  it("returns 'expiring_soon' when within threshold (1ms before threshold)", () => {
    const soon = Math.floor((now + EXPIRING_THRESHOLD_MS - 1) / 1000);
    expect(getTokenStatus(makeAccount({ expiresAt: soon }))).toBe("expiring_soon");
  });

  it("returns 'valid' when exactly at threshold boundary", () => {
    const atThreshold = Math.floor((now + EXPIRING_THRESHOLD_MS) / 1000);
    expect(getTokenStatus(makeAccount({ expiresAt: atThreshold }))).toBe("expiring_soon");
  });

  it("returns 'valid' when far in the future", () => {
    const far = Math.floor((now + EXPIRING_THRESHOLD_MS + 86400000) / 1000);
    expect(getTokenStatus(makeAccount({ expiresAt: far }))).toBe("valid");
  });
});

describe("getTokenStatusLabel", () => {
  it("returns correct label for valid", () => {
    expect(getTokenStatusLabel("valid", null)).toBe("令牌有效");
  });

  it("returns correct label for expired", () => {
    expect(getTokenStatusLabel("expired", null)).toBe("已过期");
  });

  it("returns correct label for none", () => {
    expect(getTokenStatusLabel("none", null)).toBe("无令牌");
  });

  it("returns days remaining for expiring_soon", () => {
    const soon = Math.floor((now + 2 * 86400000) / 1000);
    const label = getTokenStatusLabel("expiring_soon", soon);
    expect(label).toContain("即将过期");
    expect(label).toContain("天");
  });

  it("returns generic label when expiresAt is null", () => {
    expect(getTokenStatusLabel("expiring_soon", null)).toBe("即将过期");
  });

  it("returns 0天 when expiresAt is already past (clamped)", () => {
    const past = Math.floor(now / 1000) - 100;
    const label = getTokenStatusLabel("expiring_soon", past);
    expect(label).toContain("0天");
  });
});

describe("getCapabilities", () => {
  it("returns game_data for qq", () => {
    expect(getCapabilities("qq")).toEqual(["game_data"]);
  });

  it("returns game_data for wechat", () => {
    expect(getCapabilities("wechat")).toEqual(["game_data"]);
  });

  it("returns qqsafe for qqSafe (camelCase)", () => {
    expect(getCapabilities("qqSafe")).toEqual(["qqsafe"]);
  });

  it("returns wegame for wegameQq (camelCase)", () => {
    expect(getCapabilities("wegameQq")).toEqual(["wegame"]);
  });

  it("returns wegame for wegameWechat (camelCase)", () => {
    expect(getCapabilities("wegameWechat")).toEqual(["wegame"]);
  });

  it("returns pioneer for pioneer", () => {
    expect(getCapabilities("pioneer")).toEqual(["pioneer"]);
  });
});

describe("canRefreshToken", () => {
  it("returns true for qq", () => {
    expect(canRefreshToken("qq")).toBe(true);
  });

  it("returns true for wechat", () => {
    expect(canRefreshToken("wechat")).toBe(true);
  });

  it("returns false for qqSafe", () => {
    expect(canRefreshToken("qqSafe")).toBe(false);
  });

  it("returns false for wegameQq", () => {
    expect(canRefreshToken("wegameQq")).toBe(false);
  });

  it("returns false for wegameWechat", () => {
    expect(canRefreshToken("wegameWechat")).toBe(false);
  });

  it("returns true for pioneer", () => {
    expect(canRefreshToken("pioneer")).toBe(true);
  });
});

describe("buildGameAuth", () => {
  it("returns null when openid is null", () => {
    expect(buildGameAuth(makeAccount({ openid: null }))).toBeNull();
  });

  it("returns null when openid is empty string", () => {
    expect(buildGameAuth(makeAccount({ openid: "" }))).toBeNull();
  });

  it("returns null when accessToken is null", () => {
    expect(buildGameAuth(makeAccount({ accessToken: null }))).toBeNull();
  });

  it("returns GameAuth with acctype 'qc' for qq", () => {
    const auth = buildGameAuth(makeAccount({ kind: "qq" }));
    expect(auth).toEqual({
      openid: "openid_xxx",
      accessToken: "token_abc",
      acctype: "qc",
    });
  });

  it("returns GameAuth with acctype 'wx' for wechat", () => {
    const auth = buildGameAuth(makeAccount({ kind: "wechat" }));
    expect(auth).toEqual({
      openid: "openid_xxx",
      accessToken: "token_abc",
      acctype: "wx",
    });
  });

  it("returns acctype 'qc' for qqSafe", () => {
    const auth = buildGameAuth(makeAccount({ kind: "qqSafe", openid: "oid" }));
    expect(auth?.acctype).toBe("qc");
  });

  it("returns acctype 'qc' for wegameWechat (uses ticket, not GameAuth)", () => {
    const auth = buildGameAuth(makeAccount({ kind: "wegameWechat", openid: "oid" }));
    expect(auth?.acctype).toBe("qc");
  });

  it("returns acctype 'qc' for wegameQq", () => {
    const auth = buildGameAuth(makeAccount({ kind: "wegameQq", openid: "oid" }));
    expect(auth?.acctype).toBe("qc");
  });
});

describe("buildWegameTicket", () => {
  it("returns null when accessToken is null", () => {
    expect(buildWegameTicket(makeAccount({ accessToken: null }))).toBeNull();
  });

  it("returns ticket object for valid account", () => {
    const ticket = buildWegameTicket(makeAccount({ kind: "wegameQq" }));
    expect(ticket).toEqual({
      id: "1234567890",
      ticket: "token_abc",
    });
  });
});

describe("extractQqSafeCode", () => {
  it("returns null for null input", () => {
    expect(extractQqSafeCode(null)).toBeNull();
  });

  it("returns null for empty string", () => {
    expect(extractQqSafeCode("")).toBeNull();
  });

  it("extracts string code from JSON", () => {
    expect(extractQqSafeCode('{"code":"abc123"}')).toBe("abc123");
  });

  it("converts number code to string", () => {
    expect(extractQqSafeCode('{"code":42}')).toBe("42");
  });

  it("converts zero code to string '0'", () => {
    expect(extractQqSafeCode('{"code":0}')).toBe("0");
  });

  it("returns null for invalid JSON", () => {
    expect(extractQqSafeCode("not json")).toBeNull();
  });

  it("returns null when code field is missing", () => {
    expect(extractQqSafeCode('{"other":"value"}')).toBeNull();
  });

  it("returns null when code is empty string", () => {
    expect(extractQqSafeCode('{"code":""}')).toBeNull();
  });

  it("returns null when code is boolean", () => {
    expect(extractQqSafeCode('{"code":true}')).toBeNull();
  });

  it("returns null when code is null", () => {
    expect(extractQqSafeCode('{"code":null}')).toBeNull();
  });

  it("returns null when code is nested object", () => {
    expect(extractQqSafeCode('{"code":{"nested":1}}')).toBeNull();
  });
});

describe("getAccountDisplayName", () => {
  it("returns uinOrOpenid for qq kind", () => {
    expect(getAccountDisplayName(makeAccount({ kind: "qq" }))).toBe("1234567890");
  });

  it("returns uinOrOpenid for qqSafe kind (camelCase)", () => {
    expect(getAccountDisplayName(makeAccount({ kind: "qqSafe" }))).toBe("1234567890");
  });

  it("returns uinOrOpenid for wegameQq kind (camelCase)", () => {
    expect(getAccountDisplayName(makeAccount({ kind: "wegameQq" }))).toBe("1234567890");
  });

  it("returns uinOrOpenid for pioneer kind", () => {
    expect(getAccountDisplayName(makeAccount({ kind: "pioneer" }))).toBe("1234567890");
  });

  it("truncates long openid for wechat kind", () => {
    const longId = "a".repeat(20);
    expect(getAccountDisplayName(makeAccount({ kind: "wechat", openid: longId }))).toBe(
      `${longId.slice(0, 8)}...`
    );
  });

  it("keeps short openid for wechat kind", () => {
    const shortId = "abc123";
    expect(getAccountDisplayName(makeAccount({ kind: "wechat", openid: shortId }))).toBe(shortId);
  });

  it("does not truncate at exactly 12 chars (boundary)", () => {
    const exactlyTwelve = "a".repeat(12);
    expect(getAccountDisplayName(makeAccount({ kind: "wechat", openid: exactlyTwelve }))).toBe(exactlyTwelve);
  });

  it("truncates at 13 chars", () => {
    const thirteen = "a".repeat(13);
    expect(getAccountDisplayName(makeAccount({ kind: "wechat", openid: thirteen }))).toBe(
      `${thirteen.slice(0, 8)}...`
    );
  });

  it("falls back to uinOrOpenid when openid is null for wechat", () => {
    expect(
      getAccountDisplayName(makeAccount({ kind: "wechat", openid: null, uinOrOpenid: "short" }))
    ).toBe("short");
  });

  it("returns uinOrOpenid for wegameWechat kind", () => {
    const longId = "b".repeat(20);
    expect(getAccountDisplayName(makeAccount({ kind: "wegameWechat", openid: longId }))).toBe(
      `${longId.slice(0, 8)}...`
    );
  });
});

describe("getAcctypeForKind", () => {
  it("returns 'qc' for qq", () => {
    expect(getAcctypeForKind("qq")).toBe("qc");
  });

  it("returns 'qc' for qqSafe (camelCase)", () => {
    expect(getAcctypeForKind("qqSafe")).toBe("qc");
  });

  it("returns 'qc' for wegameQq (camelCase)", () => {
    expect(getAcctypeForKind("wegameQq")).toBe("qc");
  });

  it("returns 'wx' for wechat", () => {
    expect(getAcctypeForKind("wechat")).toBe("wx");
  });

  it("returns 'wx' for wegameWechat (camelCase)", () => {
    expect(getAcctypeForKind("wegameWechat")).toBe("wx");
  });

  it("returns 'qc' for pioneer", () => {
    expect(getAcctypeForKind("pioneer")).toBe("qc");
  });
});
