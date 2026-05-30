import { describe, expect, it } from "vitest";
import {
  DETAIL_GAME_DATA_KEYS,
  GAME_DATA_KEYS,
  PRIMARY_GAME_DATA_KEYS,
  createInitialGameDataState,
  didPrimaryGameDataBatchFail,
  markGameDataKeysLoading,
  mergeGameDataResult,
  normalizeGameDataError,
  normalizeGameDataResponse,
  shouldLoadDetailGameData,
} from "@/components/app/delta-game-data-loader";
import type { ApiResponse } from "@/components/app/delta-types";

describe("游戏数据加载批次", () => {
  it("首批仅包含角色信息和战绩记录", () => {
    expect(PRIMARY_GAME_DATA_KEYS).toEqual(["player", "record"]);
  });

  it("详情批次包含资产、近期对局、成就、密码和绑定", () => {
    expect(DETAIL_GAME_DATA_KEYS).toEqual(["assets", "recent", "achievement", "password", "bind"]);
  });

  it("完整 key 列表覆盖首批和详情批次", () => {
    expect(GAME_DATA_KEYS).toEqual([
      "player",
      "record",
      "assets",
      "recent",
      "achievement",
      "password",
      "bind",
    ]);
  });
});

describe("createInitialGameDataState", () => {
  it("为 7 个游戏数据 key 创建空闲状态", () => {
    const state = createInitialGameDataState();

    expect(Object.keys(state)).toEqual([...GAME_DATA_KEYS]);
    for (const key of GAME_DATA_KEYS) {
      expect(state[key]).toEqual({ data: null, loading: false, error: null });
    }
  });
});

describe("markGameDataKeysLoading", () => {
  it("只标记指定 key 为 loading，且不修改原状态", () => {
    const state = createInitialGameDataState();
    const next = markGameDataKeysLoading(state, PRIMARY_GAME_DATA_KEYS);

    expect(next.player).toEqual({ data: null, loading: true, error: null });
    expect(next.record).toEqual({ data: null, loading: true, error: null });
    expect(next.assets).toEqual({ data: null, loading: false, error: null });
    expect(state.player).toEqual({ data: null, loading: false, error: null });
  });
});

describe("normalizeGameDataResponse", () => {
  it("将 code 为 0 的响应转为成功结果", () => {
    const response: ApiResponse<{ level: number }> = { code: 0, msg: "成功", data: { level: 12 } };

    expect(normalizeGameDataResponse(response)).toEqual({ ok: true, data: { level: 12 } });
  });

  it("将 code 非 0 的响应转为卡片错误", () => {
    const response: ApiResponse<null> = { code: 1001, msg: "鉴权失败", data: null };

    expect(normalizeGameDataResponse(response)).toEqual({ ok: false, error: "鉴权失败" });
  });

  it("code 非 0 且 msg 为空时使用默认错误", () => {
    const response: ApiResponse<null> = { code: 1001, msg: "", data: null };

    expect(normalizeGameDataResponse(response)).toEqual({ ok: false, error: "请求失败" });
  });
});

describe("normalizeGameDataError", () => {
  it("将 invoke 抛错转为 String(error)", () => {
    expect(normalizeGameDataError("网络超时")).toEqual({ ok: false, error: "网络超时" });
  });
});

describe("mergeGameDataResult", () => {
  it("把成功结果归并到对应 key", () => {
    const state = markGameDataKeysLoading(createInitialGameDataState(), ["player"]);
    const next = mergeGameDataResult(state, "player", { ok: true, data: { name: "角色" } });

    expect(next.player).toEqual({ data: { name: "角色" }, loading: false, error: null });
    expect(state.player).toEqual({ data: null, loading: true, error: null });
  });

  it("把失败结果归并到对应 key", () => {
    const state = markGameDataKeysLoading(createInitialGameDataState(), ["record"]);
    const next = mergeGameDataResult(state, "record", { ok: false, error: "请求失败" });

    expect(next.record).toEqual({ data: null, loading: false, error: "请求失败" });
  });
});

describe("shouldLoadDetailGameData", () => {
  it("player 和 record 都失败时不允许启动详情批次", () => {
    const results = [
      { ok: false, error: "角色信息失败" },
      { ok: false, error: "战绩记录失败" },
    ] as const;

    expect(didPrimaryGameDataBatchFail(results)).toBe(true);
    expect(shouldLoadDetailGameData(results)).toBe(false);
  });

  it("player 成功且 record 失败时仍允许启动详情批次", () => {
    const results = [
      { ok: true, data: { name: "角色" } },
      { ok: false, error: "战绩记录失败" },
    ] as const;

    expect(didPrimaryGameDataBatchFail(results)).toBe(false);
    expect(shouldLoadDetailGameData(results)).toBe(true);
  });

  it("没有首批结果时不允许启动详情批次", () => {
    expect(didPrimaryGameDataBatchFail([])).toBe(false);
    expect(shouldLoadDetailGameData([])).toBe(false);
  });
});
