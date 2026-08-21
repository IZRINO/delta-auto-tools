export function isPrereleaseVersion(version: string | undefined): boolean {
    return Boolean(version?.includes("-"));
}

export function notAvailableLabel(isPrerelease: boolean): string {
    return isPrerelease ? "暂无正式版可升" : "已是最新";
}

export const BETA_UPDATE_NOTICE =
    "当前是测试版。测试包之间不会自动更新，请从 GitHub Release 手动下载。正式版发布后，可在此检查并升级。";
