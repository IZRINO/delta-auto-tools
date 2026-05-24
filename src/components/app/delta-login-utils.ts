import type { LoginFlowKind } from "@/components/app/delta-types";

export type QqLikeSession = {
  qrToken: string;
  qrSig: string;
  loginSig: string;
  cookie: string;
};

type UnknownRecord = Record<string, unknown>;

function asRecord(value: unknown): UnknownRecord | null {
  return value && typeof value === "object" ? value as UnknownRecord : null;
}

export function buildLoginQrInvokeArgs(kind: LoginFlowKind): UnknownRecord {
  return kind === "pioneer" ? { req: {} } : {};
}

export function buildQqPollInvokeArgs(kind: LoginFlowKind, session: QqLikeSession): UnknownRecord {
  if (kind === "wegame_qq") {
    return {
      request: {
        request: session,
      },
    };
  }

  return { req: session };
}

export function buildWechatPollInvokeArgs(kind: LoginFlowKind, uuid: string): UnknownRecord {
  return kind === "wegame_wechat" ? { request: { uuid } } : { uuid };
}

export function buildAccessTokenInvokeArgs(
  kind: LoginFlowKind,
  cookie?: string,
  code?: string,
): UnknownRecord {
  if (kind === "wegame_qq") {
    return { request: { cookie } };
  }

  if (kind === "wegame_wechat") {
    return { request: { code } };
  }

  if (kind === "wechat") {
    return { req: { code } };
  }

  return { req: { cookie } };
}

export function extractQqQrImage(data: unknown): string | null {
  const record = asRecord(data);
  if (!record) return null;
  const image = record.image ?? record.qrImage;
  return typeof image === "string" && image ? image : null;
}

export function extractQqQrToken(data: unknown): string | null {
  const record = asRecord(data);
  if (!record) return null;
  const token = record.token ?? record.qrToken;
  if (typeof token === "number") return String(token);
  return typeof token === "string" && token ? token : null;
}

export function extractQqPollCookie(response: unknown): string | null {
  const responseRecord = asRecord(response);
  const dataRecord = asRecord(responseRecord?.data);
  const cookie = dataRecord?.cookie;
  if (typeof cookie === "string" && cookie) return cookie;
  if (cookie && typeof cookie === "object") return JSON.stringify(cookie);
  return null;
}

export function extractWechatCode(response: unknown): string | null {
  const responseRecord = asRecord(response);
  const dataRecord = asRecord(responseRecord?.data);
  const code = dataRecord?.wxCode ?? dataRecord?.wx_code ?? responseRecord?.wxCode ?? responseRecord?.wx_code;
  return typeof code === "string" && code ? code : null;
}
