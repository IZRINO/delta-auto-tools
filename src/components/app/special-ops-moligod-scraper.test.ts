import {parseHTML} from "linkedom";
import {afterEach, beforeEach, describe, expect, it, vi} from "vitest";

import fixtureHtml from "./fixtures/moligod-ammo-exchange.html?raw";
import scraperSource from "../../../src-tauri/src/special_ops/profit/moligod_scraper.js?raw";

const TITLE_PREFIX = "DELTA_SPECIAL_OPS_PROFIT_RESULT:";

type ScraperConfig = {
    generation: number;
    nonce: string;
    targets: Array<{ruleId: string; exactName: string}>;
};

type ScraperResult = {
    generation: number;
    nonce: string;
    results: Array<{
        ruleId: string;
        exactName: string;
        profit?: string;
        status: "matched" | "sourceFailure";
        detail?: string;
    }>;
};

function installFixtureBehavior(document: Document): void {
    const input = document.querySelector<HTMLInputElement>('input[placeholder="搜索物品或兑换材料..."]');
    const pages = Array.from(document.querySelectorAll<HTMLElement>("[data-result-page]"));
    const next = document.querySelector<HTMLButtonElement>("[data-next-page]");
    if (!input || pages.length === 0 || !next) {
        return;
    }

    let pageIndex = 0;
    const apply = () => {
        const query = input.value;
        pages.forEach((page, index) => {
            page.hidden = index !== pageIndex;
            page.querySelectorAll<HTMLElement>("[data-result-row]").forEach((row) => {
                const name = row.querySelector<HTMLElement>("[data-result-name]")?.textContent?.trim() ?? "";
                row.hidden = query.length > 0 && !name.includes(query);
            });
        });
        next.disabled = pageIndex >= pages.length - 1;
    };
    input.addEventListener("input", () => {
        pageIndex = 0;
        apply();
    });
    next.addEventListener("click", () => {
        if (pageIndex < pages.length - 1) {
            pageIndex += 1;
            apply();
        }
    });
    apply();
}

function decodeTitle(title: string, nonce: string): ScraperResult {
    const prefix = `${TITLE_PREFIX}${nonce}:`;
    expect(title.startsWith(prefix)).toBe(true);
    const encoded = title.slice(prefix.length).replace(/-/g, "+").replace(/_/g, "/");
    const padded = encoded.padEnd(Math.ceil(encoded.length / 4) * 4, "=");
    const bytes = Uint8Array.from(atob(padded), (character) => character.charCodeAt(0));
    return JSON.parse(new TextDecoder().decode(bytes)) as ScraperResult;
}

async function runScraper(
    targets: ScraperConfig["targets"],
    mutate?: (document: Document) => void,
): Promise<ScraperResult> {
    const {window} = parseHTML(fixtureHtml);
    const document = window.document as unknown as Document;
    mutate?.(document);
    installFixtureBehavior(document);
    const config: ScraperConfig = {generation: 7, nonce: "nonce-7", targets};
    Object.assign(window, {__DELTA_SPECIAL_OPS_MOLIGOD_CONFIG__: config});

    const execute = new Function(
        "window",
        "document",
        "HTMLInputElement",
        "Event",
        "setTimeout",
        "getComputedStyle",
        "btoa",
        "TextEncoder",
        `${scraperSource}\nreturn window.__DELTA_SPECIAL_OPS_MOLIGOD_DONE__;`,
    );
    const done = execute(
        window,
        document,
        window.HTMLInputElement,
        window.Event,
        setTimeout,
        window.getComputedStyle?.bind(window),
        btoa,
        TextEncoder,
    ) as Promise<void>;
    await vi.runAllTimersAsync();
    await done;
    return decodeTitle(document.title, config.nonce);
}

function findRow(document: Document, exactName: string): HTMLElement {
    const name = Array.from(document.querySelectorAll<HTMLElement>("[data-result-name]")).find(
        (candidate) => candidate.textContent?.trim() === exactName && !candidate.closest("[aria-hidden='true']"),
    );
    if (!name) {
        throw new Error(`fixture 缺少目标：${exactName}`);
    }
    return name.closest<HTMLElement>("[data-result-row]")!;
}

describe("Moligod DOM scraper", () => {
    beforeEach(() => vi.useFakeTimers());
    afterEach(() => vi.useRealTimers());

    it("只命中可见精确名称并保留 i64 十进制字符串", async () => {
        const result = await runScraper([
            {ruleId: "rule-a", exactName: "5.45x39mm BT"},
            {ruleId: "rule-zero", exactName: "零利润弹"},
        ]);

        expect(result).toEqual({
            generation: 7,
            nonce: "nonce-7",
            results: [
                {
                    ruleId: "rule-a",
                    exactName: "5.45x39mm BT",
                    profit: "270458",
                    status: "matched",
                },
                {
                    ruleId: "rule-zero",
                    exactName: "零利润弹",
                    profit: "0",
                    status: "matched",
                },
            ],
        });
    });

    it("点击下一页后读取负利润", async () => {
        const result = await runScraper([{ruleId: "rule-b", exactName: ".300 BLK"}]);

        expect(result.results).toEqual([
            {
                ruleId: "rule-b",
                exactName: ".300 BLK",
                profit: "-12300",
                status: "matched",
            },
        ]);
    });

    it.each([
        ["无结果", (document: Document) => document],
        ["重复精确名称", (document: Document) => {
            const row = findRow(document, "5.45x39mm BT");
            row.parentElement?.append(row.cloneNode(true));
        }],
        ["缺详情按钮", (document: Document) => {
            findRow(document, "5.45x39mm BT").querySelector("button")?.remove();
        }],
    ])("%s 返回 sourceFailure", async (caseName, mutate) => {
        const exactName = caseName === "无结果" ? "不存在的子弹" : "5.45x39mm BT";
        const result = await runScraper([{ruleId: "rule-a", exactName}], mutate);

        expect(result.results[0]).toMatchObject({
            ruleId: "rule-a",
            exactName,
            status: "sourceFailure",
        });
    });

    it.each(["", "--", "1.5", "abc", "9223372036854775808", "-9223372036854775809"])(
        "拒绝非法利润 %j",
        async (profit) => {
            const result = await runScraper(
                [{ruleId: "rule-a", exactName: "5.45x39mm BT"}],
                (document) => {
                    findRow(document, "5.45x39mm BT").querySelector<HTMLElement>("[data-profit]")!.textContent = profit;
                },
            );

            expect(result.results[0]?.status).toBe("sourceFailure");
        },
    );

    it("搜索框缺失时全部目标返回 sourceFailure", async () => {
        const result = await runScraper(
            [{ruleId: "rule-a", exactName: "5.45x39mm BT"}],
            (document) => document.querySelector("input")?.remove(),
        );

        expect(result.results[0]).toMatchObject({status: "sourceFailure"});
    });

    it("等待页面加载后才读取延迟出现的搜索框", async () => {
        const result = await runScraper(
            [{ruleId: "rule-a", exactName: "5.45x39mm BT"}],
            (document) => {
                const input = document.querySelector<HTMLInputElement>("input")!;
                input.hidden = true;
                setTimeout(() => {
                    input.hidden = false;
                }, 500);
            },
        );

        expect(result.results[0]).toMatchObject({status: "matched", profit: "270458"});
    });

    it("等待兑换列表结束加载后再扫描首页目标", async () => {
        const result = await runScraper(
            [{ruleId: "rule-a", exactName: "5.45x39mm BT"}],
            (document) => {
                const name = findRow(document, "5.45x39mm BT").querySelector<HTMLElement>("[data-result-name]")!;
                const originalName = name.textContent;
                const loading = document.createElement("div");
                loading.textContent = "加载军需处兑换价格中...";
                document.body.append(loading);
                name.textContent = "";
                setTimeout(() => {
                    name.textContent = originalName;
                    name.closest<HTMLElement>("[data-result-row]")!.hidden = false;
                    loading.remove();
                }, 1_000);
            },
        );

        expect(result.results[0]).toMatchObject({
            status: "matched",
            profit: "270458",
        });
    });
});
