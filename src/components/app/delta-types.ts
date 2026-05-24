export type AccountKind = "qq" | "wechat" | "qqSafe" | "wegameQq" | "wegameWechat";

export type DeltaAccountRecord = {
  id: number;
  kind: AccountKind;
  uinOrOpenid: string;
  cookieJson: string;
  openid: string | null;
  accessToken: string | null;
  extraJson: string | null;
  expiresAt: number | null;
  createdAt: number;
  updatedAt: number;
};

export type GameAuth = {
  openid: string;
  accessToken: string;
  acctype: "qc" | "wx";
};

export type WegameTicket = {
  id: string;
  ticket: string;
};

export type TokenStatus = "valid" | "expiring_soon" | "expired" | "none";

export type LoginFlowKind =
  | "qq"
  | "wechat"
  | "qqsafe"
  | "wegame_qq"
  | "wegame_wechat"
  | "pioneer";

export type LoginStep =
  | "select_type"
  | "qr_code"
  | "polling"
  | "fetching_token"
  | "success"
  | "error";

export type QqPollCode = 0 | 1 | 2 | -2 | -3 | -4;
export type WechatPollCode = 1 | 2 | 3 | -2 | -3 | -4;

export type QqLoginQrResult = {
  qrImage: string;
  qrToken: string;
  qrSig: string;
  loginSig: string;
  cookie: string;
};

export type WechatLoginQrResult = {
  qrCode: string;
  uuid: string;
};

export type QqAccessTokenResult = {
  accountId: number;
  account: DeltaAccountRecord;
  auth: { openid: string; accessToken: string; expiresIn: number };
};

export type WechatAccessTokenResult = {
  accountId: number;
  account: DeltaAccountRecord;
  auth: {
    accessToken: string;
    refreshToken: string | null;
    openid: string;
    unionid: string | null;
    expiresIn: number;
  };
};

export type WegameQqAccessResult = {
  accountId: number;
  account: DeltaAccountRecord;
  auth: { tgpId: string; tgpTicket: string };
};

export type ApiResponse<T> = {
  code: number;
  msg: string;
  data: T;
};

export type Capability =
  | "game_data"
  | "wegame"
  | "qqsafe"
  | "pioneer";

export type QqSafeBannedListRequest = {
  openid: string;
  accessToken: string;
  code: string;
};

export type QqSafeReportRequest = {
  openid: string;
  accessToken: string;
  userId: string;
};

export type GameItemsQuery = {
  typeId: number;
  subType: number;
  itemId?: string;
};

export type GamePriceQuery = {
  args: number[];
  withRecent?: boolean;
};

export type GameGunsQuery = {
  gunId: string;
};

export type GameLogsQuery = {
  auth: GameAuth;
  logType: number;
  page: number;
};

export type GameFirearmModQuery = {
  page: number;
  pageSize: number;
};

export type GameRecommendationQuery = {
  place: string;
};

export type QueryWorkbenchKind =
  | "items"
  | "price"
  | "guns"
  | "logs"
  | "firearm_mod_list"
  | "recommendation";

export type EXPIRING_THRESHOLD_MS = typeof EXPIRING_THRESHOLD_MS;
export const EXPIRING_THRESHOLD_MS = 3 * 24 * 60 * 60 * 1000;

export const ACCOUNT_KIND_LABELS: Record<AccountKind, string> = {
  qq: "QQ",
  wechat: "微信",
  qqSafe: "QQSafe",
  wegameQq: "Wegame QQ",
  wegameWechat: "Wegame 微信",
};

export const ACCOUNT_KIND_CAPABILITIES: Record<AccountKind, Capability[]> = {
  qq: ["game_data"],
  wechat: ["game_data"],
  qqSafe: ["qqsafe"],
  wegameQq: ["wegame"],
  wegameWechat: ["wegame"],
};

export const CAPABILITY_LABELS: Record<Capability, string> = {
  game_data: "游戏数据",
  wegame: "Wegame",
  qqsafe: "QQSafe",
  pioneer: "先遣服",
};

export const LOGIN_FLOW_KINDS: LoginFlowKind[] = [
  "qq",
  "wechat",
  "qqsafe",
  "wegame_qq",
  "wegame_wechat",
  "pioneer",
];

export const LOGIN_FLOW_KIND_LABELS: Record<LoginFlowKind, string> = {
  qq: "QQ",
  wechat: "微信",
  qqsafe: "QQSafe",
  wegame_qq: "Wegame QQ",
  wegame_wechat: "Wegame 微信",
  pioneer: "先遣服",
};

export const LOGIN_FLOW_MODE_MAP: Record<LoginFlowKind, "qq" | "wechat"> = {
  qq: "qq",
  qqsafe: "qq",
  wegame_qq: "qq",
  pioneer: "qq",
  wechat: "wechat",
  wegame_wechat: "wechat",
};

export const QUERY_WORKBENCH_KINDS: QueryWorkbenchKind[] = [
  "items",
  "price",
  "guns",
  "logs",
  "firearm_mod_list",
  "recommendation",
];

export const QUERY_WORKBENCH_LABELS: Record<QueryWorkbenchKind, string> = {
  items: "物品查询",
  price: "物价查询",
  guns: "枪械详情",
  logs: "操作日志",
  firearm_mod_list: "改装方案",
  recommendation: "地图推荐",
};
