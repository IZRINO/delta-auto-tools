import type { ApiResponse } from "@/components/app/delta-types";

export const PRIMARY_GAME_DATA_KEYS = ["player", "record"] as const;
export const DETAIL_GAME_DATA_KEYS = ["assets", "recent", "achievement", "password", "bind"] as const;
export const GAME_DATA_KEYS = [
  "player",
  "record",
  "assets",
  "recent",
  "achievement",
  "password",
  "bind",
] as const;

export type GameDataKey = (typeof GAME_DATA_KEYS)[number];

export type GameDataItemState<T = unknown> = {
  data: T | null;
  loading: boolean;
  error: string | null;
};

export type GameDataState<T = unknown> = Record<GameDataKey, GameDataItemState<T>>;

export type GameDataLoadResult<T = unknown> =
  | { ok: true; data: T }
  | { ok: false; error: string };

export const GAME_DATA_COMMANDS: Record<GameDataKey, string> = {
  player: "delta_game_get_player",
  record: "delta_game_get_record",
  assets: "delta_game_get_assets",
  recent: "delta_game_get_recent",
  achievement: "delta_game_get_achievement",
  password: "delta_game_get_password",
  bind: "delta_game_get_bind",
};

export function createInitialGameDataState<T = unknown>(): GameDataState<T> {
  return {
    player: createIdleGameDataItemState<T>(),
    record: createIdleGameDataItemState<T>(),
    assets: createIdleGameDataItemState<T>(),
    recent: createIdleGameDataItemState<T>(),
    achievement: createIdleGameDataItemState<T>(),
    password: createIdleGameDataItemState<T>(),
    bind: createIdleGameDataItemState<T>(),
  };
}

export function markGameDataKeysLoading<T>(
  state: Readonly<GameDataState<T>>,
  keys: readonly GameDataKey[],
): GameDataState<T> {
  const next = { ...state };
  for (const key of keys) {
    next[key] = { ...state[key], loading: true, error: null };
  }
  return next;
}

export function normalizeGameDataResponse<T>(response: ApiResponse<T>): GameDataLoadResult<T> {
  if (response.code === 0) {
    return { ok: true, data: response.data };
  }
  return { ok: false, error: response.msg || "请求失败" };
}

export function normalizeGameDataError(error: unknown): GameDataLoadResult<never> {
  return { ok: false, error: String(error) };
}

export function mergeGameDataResult<T>(
  state: Readonly<GameDataState<T>>,
  key: GameDataKey,
  result: GameDataLoadResult<T>,
): GameDataState<T> {
  return {
    ...state,
    [key]: result.ok
      ? { data: result.data, loading: false, error: null }
      : { data: null, loading: false, error: result.error },
  };
}

export function didPrimaryGameDataBatchFail(
  results: readonly GameDataLoadResult<unknown>[],
): boolean {
  return results.length > 0 && results.every((result) => !result.ok);
}

export function shouldLoadDetailGameData(
  primaryResults: readonly GameDataLoadResult<unknown>[],
): boolean {
  return primaryResults.some((result) => result.ok);
}

function createIdleGameDataItemState<T>(): GameDataItemState<T> {
  return { data: null, loading: false, error: null };
}
