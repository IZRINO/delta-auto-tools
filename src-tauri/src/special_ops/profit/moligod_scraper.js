(() => {
    "use strict";

    const TITLE_PREFIX = "DELTA_SPECIAL_OPS_PROFIT_RESULT:";
    const SEARCH_PLACEHOLDER = "搜索物品或兑换材料...";
    const PROFIT_LABEL = "预估净利润";
    const DETAIL_LABEL = "详情";
    const NEXT_LABEL = "下一页";
    const LOADING_LABEL = "加载军需处兑换价格中...";
    const SAMPLE_INTERVAL_MS = 250;
    const STABLE_SAMPLE_COUNT = 3;
    const MAX_WAIT_MS = 15_000;
    const MAX_PAGES = 100;
    const config = window.__DELTA_SPECIAL_OPS_MOLIGOD_CONFIG__;
    let published = false;

    function delay(milliseconds) {
        return new Promise((resolve) => setTimeout(resolve, milliseconds));
    }

    function isVisible(element) {
        for (let current = element; current; current = current.parentElement) {
            if (current.hidden || current.getAttribute("aria-hidden") === "true") {
                return false;
            }
            const inlineStyle = (current.getAttribute("style") || "").replaceAll(" ", "").toLowerCase();
            if (inlineStyle.includes("display:none") || inlineStyle.includes("visibility:hidden")) {
                return false;
            }
            if (typeof getComputedStyle === "function") {
                try {
                    const style = getComputedStyle(current);
                    if (style.display === "none" || style.visibility === "hidden") {
                        return false;
                    }
                } catch {
                    // DOM fixture 没有完整 CSSOM；HTML 隐藏属性仍可判定。
                }
            }
        }
        return true;
    }

    function normalizedText(element) {
        return (element.textContent || "").trim();
    }

    function visibleElements(selector, root = document) {
        return Array.from(root.querySelectorAll(selector)).filter(isVisible);
    }

    function visibleTextLeaves(exactText, root = document) {
        return visibleElements("*", root).filter((element) => {
            if (normalizedText(element) !== exactText) {
                return false;
            }
            return !Array.from(element.children).some(
                (child) => isVisible(child) && normalizedText(child).length > 0,
            );
        });
    }

    function pageSummary() {
        const leafText = visibleElements("*", document)
            .filter(
                (element) =>
                    normalizedText(element).length > 0 &&
                    !Array.from(element.children).some(
                        (child) => isVisible(child) && normalizedText(child).length > 0,
                    ),
            )
            .map(normalizedText);
        const inputValues = visibleElements("input", document).map((input) => input.value || "");
        const buttonStates = visibleElements("button,[role='button']", document).map(
            (button) => `${normalizedText(button)}:${button.disabled || button.getAttribute("aria-disabled") === "true"}`,
        );
        return JSON.stringify([leafText, inputValues, buttonStates]);
    }

    function isPageLoading() {
        return visibleTextLeaves(LOADING_LABEL, document).length > 0;
    }

    async function waitForStableSummary(previousSummary = null) {
        const deadline = Date.now() + MAX_WAIT_MS;
        let lastSummary = null;
        let consistentSamples = 0;
        while (Date.now() <= deadline) {
            const summary = pageSummary();
            if (summary !== previousSummary && summary === lastSummary) {
                consistentSamples += 1;
            } else if (summary !== previousSummary) {
                lastSummary = summary;
                consistentSamples = 1;
            } else {
                consistentSamples = 0;
            }
            if (consistentSamples >= STABLE_SAMPLE_COUNT && !isPageLoading()) {
                return summary;
            }
            await delay(SAMPLE_INTERVAL_MS);
        }
        throw new Error(previousSummary === null ? "页面结果未稳定" : "翻页后页面结果未变化");
    }

    function setSearchValue(input, value) {
        const descriptor = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value");
        if (!descriptor?.set) {
            throw new Error("搜索框原生 value setter 不可用");
        }
        descriptor.set.call(input, value);
        input.dispatchEvent(new Event("input", {bubbles: true}));
        input.dispatchEvent(new Event("change", {bubbles: true}));
    }

    async function findSearchInput() {
        const deadline = Date.now() + MAX_WAIT_MS;
        while (Date.now() <= deadline) {
            const inputs = visibleElements("input", document).filter(
                (input) => input.getAttribute("placeholder") === SEARCH_PLACEHOLDER,
            );
            if (inputs.length === 1) {
                return inputs[0];
            }
            if (inputs.length > 1) {
                throw new Error(`搜索框数量异常：${inputs.length}`);
            }
            await delay(SAMPLE_INTERVAL_MS);
        }
        throw new Error("搜索框数量异常：0");
    }

    function findResultRow(nameLeaf) {
        for (let current = nameLeaf.parentElement; current && current !== document.body; current = current.parentElement) {
            const detailButtons = visibleElements("button,[role='button']", current).filter(
                (button) => normalizedText(button) === DETAIL_LABEL,
            );
            if (detailButtons.length > 0) {
                return current;
            }
        }
        throw new Error("精确名称命中行缺少详情按钮");
    }

    function parseProfitText(value) {
        const normalized = value.trim().replace(/^\+/, "").replaceAll(",", "");
        if (!/^-?\d+$/.test(normalized)) {
            throw new Error("利润不是整数");
        }
        const parsed = BigInt(normalized);
        if (parsed < -(2n ** 63n) || parsed > 2n ** 63n - 1n) {
            throw new Error("利润超出 i64");
        }
        return normalized;
    }

    function readProfit(row) {
        const labels = visibleTextLeaves(PROFIT_LABEL, row);
        if (labels.length !== 1) {
            throw new Error(`预估净利润标签数量异常：${labels.length}`);
        }
        const label = labels[0];
        const siblingCandidates = [label.nextElementSibling, label.parentElement?.nextElementSibling].filter(Boolean);
        const value = siblingCandidates.find(
            (candidate) => isVisible(candidate) && normalizedText(candidate).length > 0,
        );
        if (!value) {
            throw new Error("预估净利润值缺失");
        }
        return parseProfitText(normalizedText(value));
    }

    function scanCurrentPage(exactName, seenRows, matches) {
        for (const nameLeaf of visibleTextLeaves(exactName, document)) {
            try {
                const row = findResultRow(nameLeaf);
                if (seenRows.has(row)) {
                    continue;
                }
                seenRows.add(row);
                matches.push({profit: readProfit(row)});
            } catch (error) {
                matches.push({error: error instanceof Error ? error.message : String(error)});
            }
        }
    }

    function findNextButton() {
        const buttons = visibleElements("button,[role='button']", document).filter(
            (button) => normalizedText(button) === NEXT_LABEL,
        );
        if (buttons.length > 1) {
            throw new Error(`下一页按钮数量异常：${buttons.length}`);
        }
        return buttons[0] || null;
    }

    function isDisabled(button) {
        return button.disabled || button.getAttribute("aria-disabled") === "true";
    }

    async function scanTarget(target) {
        const input = await findSearchInput();
        setSearchValue(input, target.exactName);
        let summary = await waitForStableSummary();
        const seenSummaries = new Set();
        const seenRows = new Set();
        const matches = [];

        for (let page = 0; page < MAX_PAGES; page += 1) {
            if (seenSummaries.has(summary)) {
                throw new Error("分页结果发生循环");
            }
            seenSummaries.add(summary);
            scanCurrentPage(target.exactName, seenRows, matches);
            const nextButton = findNextButton();
            if (!nextButton || isDisabled(nextButton)) {
                break;
            }
            const previousSummary = summary;
            nextButton.click();
            summary = await waitForStableSummary(previousSummary);
            if (page === MAX_PAGES - 1) {
                throw new Error("分页数量超过限制");
            }
        }

        if (matches.length !== 1) {
            throw new Error(`精确名称命中数量异常：${matches.length}`);
        }
        if (matches[0].error) {
            throw new Error(matches[0].error);
        }
        return matches[0].profit;
    }

    function encodeBase64Url(value) {
        const bytes = new TextEncoder().encode(value);
        let binary = "";
        for (let offset = 0; offset < bytes.length; offset += 8192) {
            binary += String.fromCharCode(...bytes.subarray(offset, offset + 8192));
        }
        return btoa(binary).replaceAll("+", "-").replaceAll("/", "_").replace(/=+$/, "");
    }

    function publish(results) {
        if (published) {
            return;
        }
        published = true;
        const payload = {
            generation: config.generation,
            nonce: config.nonce,
            results,
        };
        document.title = `${TITLE_PREFIX}${config.nonce}:${encodeBase64Url(JSON.stringify(payload))}`;
    }

    async function run() {
        if (
            !config ||
            !Number.isSafeInteger(config.generation) ||
            typeof config.nonce !== "string" ||
            config.nonce.length === 0 ||
            !Array.isArray(config.targets)
        ) {
            throw new Error("Moligod 只读配置无效");
        }
        const results = [];
        for (const target of config.targets) {
            try {
                const profit = await scanTarget(target);
                results.push({
                    ruleId: target.ruleId,
                    exactName: target.exactName,
                    profit,
                    status: "matched",
                });
            } catch (error) {
                results.push({
                    ruleId: target.ruleId,
                    exactName: target.exactName,
                    status: "sourceFailure",
                    detail: error instanceof Error ? error.message : String(error),
                });
            }
        }
        publish(results);
    }

    window.__DELTA_SPECIAL_OPS_MOLIGOD_DONE__ = run();
})();
