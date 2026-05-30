import { describe, expect, it } from "vitest";
import {
  buildAccessTokenInvokeArgs,
  buildLoginQrInvokeArgs,
  buildQqPollInvokeArgs,
  buildWechatPollInvokeArgs,
  extractPollSessionKey,
  extractQqQrImage,
  extractQqQrToken,
} from "@/components/app/delta-login-utils";

const qqSession = {
  qrToken: "token-1",
  qrSig: "sig-1",
  loginSig: "login-1",
  sessionKey: "session-1",
};

describe("delta-login-utils invoke args", () => {
  it("wraps QQ polling payload with req", () => {
    expect(buildQqPollInvokeArgs("qq", qqSession)).toEqual({
      req: qqSession,
    });
  });

  it("wraps Pioneer polling payload with req", () => {
    expect(buildQqPollInvokeArgs("pioneer", qqSession)).toEqual({
      req: qqSession,
    });
  });

  it("wraps Wegame QQ polling payload with request.request", () => {
    expect(buildQqPollInvokeArgs("wegame_qq", qqSession)).toEqual({
      request: {
        request: qqSession,
      },
    });
  });

  it("uses bare args for QR requests", () => {
    expect(buildLoginQrInvokeArgs("pioneer")).toEqual({});
    expect(buildLoginQrInvokeArgs("qq")).toEqual({});
  });

  it("wraps WeChat polling for Wegame WeChat only", () => {
    expect(buildWechatPollInvokeArgs("wechat", "uuid-1")).toEqual({ uuid: "uuid-1" });
    expect(buildWechatPollInvokeArgs("wegame_wechat", "uuid-1")).toEqual({
      request: { uuid: "uuid-1" },
    });
  });

  it("wraps access token requests with one-time sessionKey", () => {
    expect(buildAccessTokenInvokeArgs("qq", "session-1")).toEqual({
      req: { sessionKey: "session-1" },
    });
    expect(buildAccessTokenInvokeArgs("pioneer", "session-1")).toEqual({
      req: { sessionKey: "session-1" },
    });
    expect(buildAccessTokenInvokeArgs("wegame_qq", "session-1")).toEqual({
      request: { sessionKey: "session-1" },
    });
    expect(buildAccessTokenInvokeArgs("wechat", "session-1")).toEqual({
      req: { sessionKey: "session-1" },
    });
    expect(buildAccessTokenInvokeArgs("wegame_wechat", "session-1")).toEqual({
      request: { sessionKey: "session-1" },
    });
  });
});

describe("delta-login-utils response extraction", () => {
  it("reads the QR image from current Rust field name", () => {
    expect(extractQqQrImage({ image: "base64-image" })).toBe("base64-image");
  });

  it("reads numeric QR token", () => {
    expect(extractQqQrToken({ token: 123 })).toBe("123");
  });

  it("reads sessionKey from ApiResponse data", () => {
    expect(extractPollSessionKey({ code: 0, msg: "ok", data: { sessionKey: "session-1" } })).toBe(
      "session-1",
    );
  });

  it("returns null when sessionKey is missing", () => {
    expect(extractPollSessionKey({ code: 0, msg: "ok", data: {} })).toBeNull();
  });
});
