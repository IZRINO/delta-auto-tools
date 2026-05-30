import type { LoginFlowKind } from "@/components/app/delta-types";

export type QqLikeSession = {
  qrToken: string;
  qrSig: string;
  loginSig: string;
  sessionKey: string;
};

type UnknownRecord = Record<string, unknown>;

function asRecord(value: unknown): UnknownRecord | null {
  return value && typeof value === "object" ? value as UnknownRecord : null;
}

export function buildLoginQrInvokeArgs(_kind: LoginFlowKind): UnknownRecord {
  return {};
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
  sessionKey?: string,
): UnknownRecord {
  if (kind === "wegame_qq" || kind === "wegame_wechat") {
    return { request: { sessionKey } };
  }

  return { req: { sessionKey } };
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

export function extractPollSessionKey(response: unknown): string | null {
  const responseRecord = asRecord(response);
  const dataRecord = asRecord(responseRecord?.data);
  const sessionKey = dataRecord?.sessionKey ?? responseRecord?.sessionKey;
  return typeof sessionKey === "string" && sessionKey ? sessionKey : null;
}