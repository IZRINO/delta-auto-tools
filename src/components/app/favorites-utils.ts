/**
 * 收藏（Favorites）模块的纯逻辑工具函数。
 *
 * 该模块只承载：收藏模型、fractional indexing 排序、本地存储读写与 view 偏好。
 * 不包含 React 组件、副作用与 Tauri 命令调用，方便被单元测试覆盖。
 */

/**
 * 支持收藏的卡片类型。
 * - `timer`：计时\计数器工作台里的计时器卡片
 * - `counter`：计时\计数器工作台里的计数器卡片
 * - `rapidfire`：连发器工作台里的连发器卡片
 */
export type FavoriteCardKind = "timer" | "counter" | "rapidfire";

/**
 * 收藏项。
 * - `kind` / `cardId` 一起作为逻辑键
 * - `sortKey` 是 fractional indexing 用的浮点序号
 */
export type FavoriteItem = {
    kind: FavoriteCardKind;
    cardId: string;
    sortKey: number;
};

/**
 * 收藏页 UI 偏好（用户可调）。
 * - `showHotkey`：是否显示快捷键
 * - `showProgress`：计时器是否显示进度
 * - `showCounter`：计数器是否显示当前值
 * - `compactMode`：单行紧凑 vs 多行详情
 */
export type FavoritesView = {
    showHotkey: boolean;
    showProgress: boolean;
    showCounter: boolean;
    compactMode: boolean;
};

/**
 * 收藏模块的本地存储结构。
 */
export type FavoritesState = {
    items: FavoriteItem[];
    view: FavoritesView;
};

export type FavoriteSourceIds = Partial<Record<FavoriteCardKind, ReadonlySet<string>>>;

/**
 * 默认 view 偏好：全部显示。
 */
export const DEFAULT_FAVORITES_VIEW: FavoritesView = {
    showHotkey: true,
    showProgress: true,
    showCounter: true,
    compactMode: false,
};

/**
 * 默认 state：空收藏 + 全开 view 偏好。
 */
export const DEFAULT_FAVORITES_STATE: FavoritesState = {
    items: [],
    view: {...DEFAULT_FAVORITES_VIEW},
};

/** 默认 sortKey 步长：插入到末尾时使用 `max + step`，插入到开头使用 `min - step`。 */
const SORT_KEY_STEP = 1024;

/** 判断给定 kind 字符串是否合法。 */
function isFavoriteCardKind(value: unknown): value is FavoriteCardKind {
    return value === "timer" || value === "counter" || value === "rapidfire";
}

/**
 * 工具函数：构造 favorite key（用于 `kind + cardId` 的去重）。
 */
export function favoriteKey(kind: FavoriteCardKind, cardId: string): string {
    return `${kind}:${cardId}`;
}

/**
 * 工具函数：从 key 解析 favorite。
 * - key 形如 `timer:timer-1` / `counter:counter-2` / `rapidfire:rapidfire-3`
 * - 解析失败时返回 null
 */
export function parseFavoriteKey(key: string): { kind: FavoriteCardKind; cardId: string } | null {
    const index = key.indexOf(":");
    if (index <= 0 || index >= key.length - 1) {
        return null;
    }
    const kindRaw = key.slice(0, index);
    const cardId = key.slice(index + 1);
    if (cardId.length === 0) {
        return null;
    }
    if (!isFavoriteCardKind(kindRaw)) {
        return null;
    }
    return {kind: kindRaw, cardId};
}

/**
 * 判断给定 kind/cardId 是否已收藏。
 */
export function isFavorite(
    state: FavoritesState,
    kind: FavoriteCardKind,
    cardId: string,
): boolean {
    return state.items.some((item) => item.kind === kind && item.cardId === cardId);
}

/**
 * 加入收藏：若已存在则保持原样；否则按 sortKey 升序追加。
 */
export function addFavorite(
    state: FavoritesState,
    kind: FavoriteCardKind,
    cardId: string,
): FavoritesState {
    if (state.items.some((item) => item.kind === kind && item.cardId === cardId)) {
        return state;
    }
    const nextSortKey = state.items.length === 0
        ? SORT_KEY_STEP
        : Math.max(...state.items.map((item) => item.sortKey)) + SORT_KEY_STEP;
    return {
        ...state,
        items: [...state.items, {kind, cardId, sortKey: nextSortKey}],
    };
}

/**
 * 移除收藏。
 */
export function removeFavorite(
    state: FavoritesState,
    kind: FavoriteCardKind,
    cardId: string,
): FavoritesState {
    const filtered = state.items.filter(
        (item) => !(item.kind === kind && item.cardId === cardId),
    );
    if (filtered.length === state.items.length) {
        return state;
    }
    return {...state, items: filtered};
}

/**
 * 切换收藏状态：已收藏则移除，未收藏则加入。
 */
export function toggleFavorite(
    state: FavoritesState,
    kind: FavoriteCardKind,
    cardId: string,
): FavoritesState {
    return isFavorite(state, kind, cardId)
        ? removeFavorite(state, kind, cardId)
        : addFavorite(state, kind, cardId);
}

/**
 * 把收藏项按 sortKey 升序排列。
 */
export function sortFavorites(items: ReadonlyArray<FavoriteItem>): FavoriteItem[] {
    return [...items].sort((a, b) => a.sortKey - b.sortKey);
}

/**
 * 重新分配 sortKey：传入顺序即最终顺序，sortKey 按 1..N 重新分配。
 * 用于 fractional indexing 精度耗尽时的兜底。
 */
export function renumberFavorites(items: ReadonlyArray<FavoriteItem>): FavoriteItem[] {
    return items.map((item, index) => ({...item, sortKey: (index + 1) * SORT_KEY_STEP}));
}


/**
 * 计算移动到目标位置后的新 sortKey。
 * - `position === "start"`：放到列表最前
 * - `position === "end"`：放到列表最后
 * - `position === { before: cardId }`：放到指定 cardId 之前
 * - `position === { after: cardId }`：放到指定 cardId 之后
 *
 * 精度不足时自动 renumber 整个列表再返回。
 */
export function moveFavorite(
    state: FavoritesState,
    kind: FavoriteCardKind,
    cardId: string,
    position:
        | "start"
        | "end"
        | { before: string }
        | { after: string },
): FavoritesState {
    const sorted = sortFavorites(state.items);
    const activeIndex = sorted.findIndex((item) => item.kind === kind && item.cardId === cardId);
    if (activeIndex < 0) {
        return state;
    }
    const active = sorted[activeIndex];
    const withoutActive = sorted.filter((_, index) => index !== activeIndex);

    let targetIndex: number;
    if (position === "start") {
        targetIndex = 0;
    } else if (position === "end") {
        targetIndex = withoutActive.length;
    } else if ("before" in position) {
        targetIndex = withoutActive.findIndex((item) => item.cardId === position.before);
        if (targetIndex < 0) {
            targetIndex = withoutActive.length;
        }
    } else {
        targetIndex = withoutActive.findIndex((item) => item.cardId === position.after);
        if (targetIndex < 0) {
            targetIndex = withoutActive.length;
        } else {
            targetIndex += 1;
        }
    }

    const prev = targetIndex > 0 ? withoutActive[targetIndex - 1] : null;
    const next = targetIndex < withoutActive.length ? withoutActive[targetIndex] : null;

    let nextSortKey: number;
    if (prev && next) {
        nextSortKey = (prev.sortKey + next.sortKey) / 2;
    } else if (prev) {
        nextSortKey = prev.sortKey + SORT_KEY_STEP;
    } else if (next) {
        nextSortKey = next.sortKey - SORT_KEY_STEP;
    } else {
        nextSortKey = SORT_KEY_STEP;
    }

    // 精度耗尽兜底：当 gap 小于 1e-6 时全表 renumber。
    if (prev && next && Math.abs(next.sortKey - prev.sortKey) < 1e-6) {
        const relocated = [
            ...withoutActive.slice(0, targetIndex),
            {kind: active.kind, cardId: active.cardId, sortKey: 0},
            ...withoutActive.slice(targetIndex),
        ];
        const renumbered = renumberFavorites(relocated);
        return {...state, items: renumbered};
    }

    const relocated = [
        ...withoutActive.slice(0, targetIndex),
        {kind: active.kind, cardId: active.cardId, sortKey: nextSortKey},
        ...withoutActive.slice(targetIndex),
    ];

    return {...state, items: sortFavorites(relocated)};
}

/**
 * 更新 view 偏好。
 */
export function updateFavoritesView(
    state: FavoritesState,
    patch: Partial<FavoritesView>,
): FavoritesState {
    return {...state, view: {...state.view, ...patch}};
}

/**
 * 清理无效项：把 items 里 `cardId` 不在 `validKeys` 集合中的项移除。
 * 用于卡片被删除后自动清理孤儿收藏。
 */
export function pruneFavorites(
    state: FavoritesState,
    validKeys: ReadonlySet<string>,
): FavoritesState {
    const filtered = state.items.filter((item) => validKeys.has(favoriteKey(item.kind, item.cardId)));
    if (filtered.length === state.items.length) {
        return state;
    }
    return {...state, items: filtered};
}

export function pruneFavoritesWithSources(
    state: FavoritesState,
    sources: FavoriteSourceIds,
): FavoritesState {
    const {timer, counter, rapidfire} = sources;
    if (!timer || !counter || !rapidfire) {
        return state;
    }

    const validKeys = new Set<string>();
    for (const cardId of timer) validKeys.add(favoriteKey("timer", cardId));
    for (const cardId of counter) validKeys.add(favoriteKey("counter", cardId));
    for (const cardId of rapidfire) validKeys.add(favoriteKey("rapidfire", cardId));
    return pruneFavorites(state, validKeys);
}

export async function settleFavoriteBootstraps<Timer, Counter, Rapidfire>(
    timerLoad: Promise<Timer>,
    counterLoad: Promise<Counter>,
    rapidfireLoad: Promise<Rapidfire>,
): Promise<{timer: Timer | null; counter: Counter | null; rapidfire: Rapidfire | null}> {
    const [timer, counter, rapidfire] = await Promise.allSettled([
        timerLoad,
        counterLoad,
        rapidfireLoad,
    ] as const);
    return {
        timer: timer.status === "fulfilled" ? timer.value : null,
        counter: counter.status === "fulfilled" ? counter.value : null,
        rapidfire: rapidfire.status === "fulfilled" ? rapidfire.value : null,
    };
}

const STORAGE_KEY = "delta-auto-tools:favorites:v1";

/**
 * 读取 storage 抽象（可注入到测试）。
 */
type StorageLike = Pick<Storage, "getItem" | "setItem"> | null;

function getDefaultStorage(): StorageLike {
    if (typeof window === "undefined" || !window.localStorage) {
        return null;
    }
    return window.localStorage;
}

/**
 * 反序列化收藏 state。损坏 / 解析失败时回落到默认 state。
 *
 * 校验：
 * - 必须为对象
 * - `items` 必须为数组；每个元素必须有合法 kind / cardId / 数字 sortKey
 * - `view` 字段缺失时回落到默认值；类型不符时按字段单独回落到默认值
 */
export function readStoredFavorites(
    storage: StorageLike = getDefaultStorage(),
): FavoritesState {
    if (storage === null) {
        return {items: [], view: {...DEFAULT_FAVORITES_VIEW}};
    }
    let raw: string | null;
    try {
        raw = storage.getItem(STORAGE_KEY);
    } catch {
        return {items: [], view: {...DEFAULT_FAVORITES_VIEW}};
    }
    if (raw === null || raw.length === 0) {
        return {items: [], view: {...DEFAULT_FAVORITES_VIEW}};
    }
    let parsed: unknown;
    try {
        parsed = JSON.parse(raw);
    } catch {
        return {items: [], view: {...DEFAULT_FAVORITES_VIEW}};
    }
    if (!parsed || typeof parsed !== "object") {
        return {items: [], view: {...DEFAULT_FAVORITES_VIEW}};
    }
    const candidate = parsed as Record<string, unknown>;
    const items = parseFavoritesItems(candidate.items);
    const view = parseFavoritesView(candidate.view);
    return {items, view};
}

function parseFavoritesItems(value: unknown): FavoriteItem[] {
    if (!Array.isArray(value)) {
        return [];
    }
    const result: FavoriteItem[] = [];
    const seen = new Set<string>();
    for (const item of value) {
        if (!item || typeof item !== "object") {
            continue;
        }
        const record = item as Record<string, unknown>;
        const kind = record.kind;
        const cardId = record.cardId;
        const sortKey = record.sortKey;
        if (typeof cardId !== "string" || cardId.length === 0) {
            continue;
        }
        if (typeof sortKey !== "number" || !Number.isFinite(sortKey)) {
            continue;
        }
        if (!isFavoriteCardKind(kind)) {
            continue;
        }
        const key = favoriteKey(kind, cardId);
        if (seen.has(key)) {
            continue;
        }
        seen.add(key);
        result.push({kind, cardId, sortKey});
    }
    return result;
}

function parseFavoritesView(value: unknown): FavoritesView {
    if (!value || typeof value !== "object") {
        return {...DEFAULT_FAVORITES_VIEW};
    }
    const record = value as Record<string, unknown>;
    return {
        showHotkey: typeof record.showHotkey === "boolean" ? record.showHotkey : DEFAULT_FAVORITES_VIEW.showHotkey,
        showProgress: typeof record.showProgress === "boolean" ? record.showProgress : DEFAULT_FAVORITES_VIEW.showProgress,
        showCounter: typeof record.showCounter === "boolean" ? record.showCounter : DEFAULT_FAVORITES_VIEW.showCounter,
        compactMode: typeof record.compactMode === "boolean" ? record.compactMode : DEFAULT_FAVORITES_VIEW.compactMode,
    };
}

/**
 * 序列化收藏 state。损坏数据通过 `try/catch` 静默吞掉。
 */
export function writeStoredFavorites(
    state: FavoritesState,
    storage: StorageLike = getDefaultStorage(),
): void {
    if (storage === null) {
        return;
    }
    try {
        storage.setItem(STORAGE_KEY, JSON.stringify(state));
    } catch {
        // 隐私模式 / 配额限制下会抛错；保持主流程不被破坏。
    }
}
