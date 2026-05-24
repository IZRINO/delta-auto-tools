import { describe, expect, it } from "vitest";
import {
  buildAccessTokenInvokeArgs,
  buildLoginQrInvokeArgs,
  buildQqPollInvokeArgs,
  buildWechatPollInvokeArgs,
  extractQqPollCookie,
  extractQqQrImage,
  extractWechatCode,
} from "@/components/app/delta-login-utils";

const qqSession = {
  qrToken: "token-1",
  qrSig: "sig-1",
  loginSig: "login-1",
  cookie: "{\"qrsig\":\"sig-1\"}",
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

  it("wraps Pioneer QR request with req", () => {
    expect(buildLoginQrInvokeArgs("pioneer")).toEqual({
      req: {},
    });
  });

  it("uses bare args for normal QQ QR request", () => {
    expect(buildLoginQrInvokeArgs("qq")).toEqual({});
  });

  it("wraps WeChat polling for Wegame WeChat only", () => {
    expect(buildWechatPollInvokeArgs("wechat", "uuid-1")).toEqual({ uuid: "uuid-1" });
    expect(buildWechatPollInvokeArgs("wegame_wechat", "uuid-1")).toEqual({
      request: { uuid: "uuid-1" },
    });
  });

  it("wraps access token requests by command signature", () => {
    expect(buildAccessTokenInvokeArgs("qq", "cookie-1")).toEqual({
      req: { cookie: "cookie-1" },
    });
    expect(buildAccessTokenInvokeArgs("pioneer", "cookie-1")).toEqual({
      req: { cookie: "cookie-1" },
    });
    expect(buildAccessTokenInvokeArgs("wegame_qq", "cookie-1")).toEqual({
      request: { cookie: "cookie-1" },
    });
    expect(buildAccessTokenInvokeArgs("wechat", undefined, "code-1")).toEqual({
      req: { code: "code-1" },
    });
    expect(buildAccessTokenInvokeArgs("wegame_wechat", undefined, "code-1")).toEqual({
      request: { code: "code-1" },
    });
  });
});

describe("delta-login-utils response extraction", () => {
  it("reads the QR image from current Rust field name", () => {
    expect(extractQqQrImage({ image: "base64-image" })).toBe("base64-image");
  });

  it("prefers the logged-in cookie returned by polling", () => {
    expect(extractQqPollCookie({ code: 0, msg: "ok", data: { cookie: "{\"p_skey\":\"abc\"}" } })).toBe(
      "{\"p_skey\":\"abc\"}",
    );
  });

  it("serializes object cookie payloads for compatibility with php-shaped responses", () => {
    expect(extractQqPollCookie({ code: 0, msg: "ok", data: { cookie: { p_skey: "abc" } } })).toBe(
      "{\"p_skey\":\"abc\"}",
    );
  });

  it("reads wxCode from ApiResponse data", () => {
    expect(extractWechatCode({ code: 3, msg: "ok", data: { wxCode: "wx-code-1" } })).toBe("wx-code-1");
  });
});
