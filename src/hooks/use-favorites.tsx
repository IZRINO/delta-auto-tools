import {createContext, type ReactNode, useCallback, useContext, useEffect, useMemo, useState} from "react";

import {
    addFavorite as addFavoriteUtil,
    DEFAULT_FAVORITES_STATE,
    type FavoriteCardKind,
    type FavoriteItem,
    type FavoriteSourceIds,
    type FavoritesState,
    type FavoritesView,
    isFavorite as isFavoriteUtil,
    moveFavorite as moveFavoriteUtil,
    pruneFavoritesWithSources,
    readStoredFavorites,
    removeFavorite as removeFavoriteUtil,
    toggleFavorite as toggleFavoriteUtil,
    updateFavoritesView as updateFavoritesViewUtil,
    writeStoredFavorites,
} from "@/components/app/favorites-utils";

type FavoritesContextValue = {
    /** 已按 sortKey 升序排列的收藏项 */
    items: ReadonlyArray<FavoriteItem>;
    /** 完整 state（包含原始 sortKey），用于更新函数 */
    state: FavoritesState;
    /** 视图偏好 */
    view: FavoritesView;
    /** 是否已收藏 */
    isFavorite: (kind: FavoriteCardKind, cardId: string) => boolean;
    /** 加入收藏（已收藏则忽略） */
    addFavorite: (kind: FavoriteCardKind, cardId: string) => void;
    /** 移除收藏 */
    removeFavorite: (kind: FavoriteCardKind, cardId: string) => void;
    /** 切换收藏 */
    toggleFavorite: (kind: FavoriteCardKind, cardId: string) => void;
    /** 移动到指定位置（fractional indexing） */
    moveFavorite: (kind: FavoriteCardKind, cardId: string, position: "start" | "end" | { before: string } | {
        after: string
    }) => void;
    /** 更新视图偏好 */
    updateView: (patch: Partial<FavoritesView>) => void;
    /** 三类来源全部 ready 后清理孤儿收藏项。 */
    prune: (sources: FavoriteSourceIds) => void;
};

const FavoritesContext = createContext<FavoritesContextValue | null>(null);

export function useFavorites(): FavoritesContextValue {
    const ctx = useContext(FavoritesContext);
    if (!ctx) {
        throw new Error("useFavorites must be used within FavoritesProvider");
    }
    return ctx;
}

type FavoritesProviderProps = {
    children: ReactNode;
};

export function FavoritesProvider({children}: FavoritesProviderProps) {
    const [state, setState] = useState<FavoritesState>(() => readStoredFavorites());

    // 任何 state 变更都同步写回 localStorage；隐私模式 / 配额限制下会静默吞掉。
    useEffect(() => {
        writeStoredFavorites(state);
    }, [state]);

    const addFavorite = useCallback((kind: FavoriteCardKind, cardId: string) => {
        setState((current) => addFavoriteUtil(current, kind, cardId));
    }, []);

    const removeFavorite = useCallback((kind: FavoriteCardKind, cardId: string) => {
        setState((current) => removeFavoriteUtil(current, kind, cardId));
    }, []);

    const toggleFavorite = useCallback((kind: FavoriteCardKind, cardId: string) => {
        setState((current) => toggleFavoriteUtil(current, kind, cardId));
    }, []);

    const moveFavorite = useCallback((
        kind: FavoriteCardKind,
        cardId: string,
        position: "start" | "end" | { before: string } | { after: string },
    ) => {
        setState((current) => moveFavoriteUtil(current, kind, cardId, position));
    }, []);

    const updateView = useCallback((patch: Partial<FavoritesView>) => {
        setState((current) => updateFavoritesViewUtil(current, patch));
    }, []);

    const prune = useCallback((sources: FavoriteSourceIds) => {
        setState((current) => pruneFavoritesWithSources(current, sources));
    }, []);

    const isFavoriteFn = useCallback(
        (kind: FavoriteCardKind, cardId: string) => isFavoriteUtil(state, kind, cardId),
        [state],
    );

    const value = useMemo<FavoritesContextValue>(() => {
        const items = [...state.items].sort((a, b) => a.sortKey - b.sortKey);
        return {
            items,
            state,
            view: state.view,
            isFavorite: isFavoriteFn,
            addFavorite,
            removeFavorite,
            toggleFavorite,
            moveFavorite,
            updateView,
            prune,
        };
    }, [state, isFavoriteFn, addFavorite, removeFavorite, toggleFavorite, moveFavorite, updateView, prune]);

    return (
        <FavoritesContext.Provider value={value}>
            {children}
        </FavoritesContext.Provider>
    );
}

export const __FAVORITES_TEST_HELPERS = {
    DEFAULT_FAVORITES_STATE,
};
