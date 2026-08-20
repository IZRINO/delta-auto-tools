import {useEffect} from "react";

import {scrollElementIntoView} from "@/lib/utils";

/**
 * 收藏页高亮跳转：滚动到目标卡片并添加 1.5s 高亮动画。
 *
 * @param highlightCardId - 高亮目标（含 cardId 与 nonce）
 * @param kind - 卡片类型（timer / counter / rapidfire）
 */
export function useHighlightScroll(
    highlightCardId: { cardId: string; nonce: number } | null,
    kind: "timer" | "counter" | "rapidfire",
) {
    useEffect(() => {
        if (!highlightCardId) {
            return;
        }
        const selector = `[data-favorite-card="${kind}:${highlightCardId.cardId}"]`;
        const handle = window.setTimeout(() => {
            const element = document.querySelector<HTMLElement>(selector);
            if (!element) {
                return;
            }
            element.classList.remove("favorite-highlight");
            // 强制 reflow 重新触发动画
            void element.offsetWidth;
            element.classList.add("favorite-highlight");
            scrollElementIntoView(element, "center");
        }, 80);
        return () => {
            window.clearTimeout(handle);
        };
    }, [highlightCardId, kind]);
}
