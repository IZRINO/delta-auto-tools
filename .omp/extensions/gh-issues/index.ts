/**
 * gh-issues OMP 扩展
 *
 * 注册两个 slash command：
 * - `/gh-issues [repo] [interval-min] [prompt]`：启动 GitHub issues 轮询
 * - `/gh-issues-stop`：停止当前轮询器
 *
 * 轮询器通过 `gh issue list` 查询指定仓库的开放 issue，发现新增条目后：
 * - 未提供 prompt：通过 `ctx.ui.notify` 通知
 * - 提供 prompt：把 issue 列表 + prompt 入队为用户消息（`sendUserMessage` + `deliverAs: "followUp"`），
 *   用户在交互模式下可审阅后按 Enter 提交
 *
 * 默认仓库为本项目 `IZRINO/delta-auto-tools`，默认间隔 60 分钟。
 */

import type {
    ExtensionAPI,
    ExtensionCommandContext,
    ExecResult,
} from "@oh-my-pi/pi-coding-agent";

/** 单条 GitHub issue（`gh issue list --json ...` 输出）。
 *  由于我们固定按 `--state open` 过滤，`gh` 会省略 `state` 字段，因此不在此处声明。 */
export interface Issue {
    number: number;
    title: string;
    url: string;
    body: string;
    createdAt: string;
    author: { login: string } | null;
    labels: { name: string }[];
}

/** `/gh-issues` 命令参数解析结果 */
export interface ParsedArgs {
    repo?: string;
    intervalMin?: number;
    prompt?: string;
}

/** 轮询器运行时状态 */
export interface PollState {
    repo: string;
    intervalMs: number;
    prompt: string | null;
    seen: Set<number>;
    timer: ReturnType<typeof setTimeout> | null;
    abortController: AbortController | null;
    running: boolean;
    stopped: boolean;
    resolveStopped: (() => void) | null;
    lastOutputAt: Date | null;
    nextRunAt: Date | null;
}

function formatTimestamp(date: Date | null): string {
    if (!date) return "尚无";
    const year = date.getFullYear();
    const month = String(date.getMonth() + 1).padStart(2, "0");
    const day = String(date.getDate()).padStart(2, "0");
    const hour = String(date.getHours()).padStart(2, "0");
    const minute = String(date.getMinutes()).padStart(2, "0");
    const second = String(date.getSeconds()).padStart(2, "0");
    return `${year}-${month}-${day} ${hour}:${minute}:${second}`;
}

function formatIntervalMs(intervalMs: number): string {
    const minutes = intervalMs / 60_000;
    return Number.isInteger(minutes) ? `${minutes} 分钟` : `${Math.round(intervalMs / 1000)} 秒`;
}

function describeNextRun(state: PollState): string {
    if (state.nextRunAt) return formatTimestamp(state.nextRunAt);
    return state.running ? "正在执行" : "立即执行";
}

function formatScheduleLines(state: PollState): string {
    return `上次输出：${formatTimestamp(state.lastOutputAt)}\n下次运行：${describeNextRun(state)}`;
}

function withScheduleLines(message: string, state: PollState): string {
    return `${message}\n${formatScheduleLines(state)}`;
}

function updatePollerUi(ctx: ExtensionCommandContext, state: PollState): void {
    const intervalText = formatIntervalMs(state.intervalMs);
    const lastOutput = formatTimestamp(state.lastOutputAt);
    const nextRun = describeNextRun(state);
    ctx.ui.setWorkingMessage(
        `[gh-issues] 监听 ${state.repo}，每 ${intervalText}；上次输出：${lastOutput}；下次运行：${nextRun}；按 Esc 停止`,
    );
    ctx.ui.setStatus(
        "gh-issues",
        `${state.repo} 每 ${intervalText}；上次输出 ${lastOutput}；下次运行 ${nextRun}；Esc 停止`,
    );
}

function markOutputAndNextRun(state: PollState): void {
    const outputAt = new Date();
    state.lastOutputAt = outputAt;
    state.nextRunAt = new Date(outputAt.getTime() + state.intervalMs);
}

function createStoppedPromise(): { promise: Promise<void>; resolve: () => void } {
    let resolve!: () => void;
    const promise = new Promise<void>((resolvePromise) => {
        resolve = resolvePromise;
    });
    return { promise, resolve };
}

const DEFAULT_REPO = "IZRINO/delta-auto-tools";
const DEFAULT_INTERVAL_MIN = 60;
const ISSUES_PER_PAGE = 30;

let active: PollState | null = null;

function stopActivePoller(): boolean {
    const state = active;
    if (!state) return false;

    state.stopped = true;
    if (state.timer) {
        clearTimeout(state.timer);
        state.timer = null;
    }
    if (state.abortController) {
        state.abortController.abort();
        state.abortController = null;
    }
    if (state.resolveStopped) {
        state.resolveStopped();
        state.resolveStopped = null;
    }
    active = null;
    return true;
}

function isActivePoller(state: PollState): boolean {
    return active === state && !state.stopped;
}

export default function (pi: ExtensionAPI): void {
    pi.on("session_shutdown", () => {
        stopActivePoller();
    });

    pi.registerCommand("gh-issues", {
        description: [
            "监听 GitHub 仓库的 issue。",
            "参数（按内容自动归类，未填用默认；带引号的 token 一律视为 prompt）：",
            "  /gh-issues                                          # 仅通知（默认本仓库、60 分钟）",
            "  /gh-issues owner/repo                               # 自定义仓库 + 通知",
            "  /gh-issues 30                                       # 自定义间隔 + 通知",
            "  /gh-issues \"请逐个分析\"                            # 自定义 prompt + 入队",
            "  /gh-issues owner/repo 30                            # 自定义仓库 + 间隔",
            "  /gh-issues owner/repo 30 \"请逐个分析\"              # 全部自定义",
        ].join("\n"),
        handler: async (args: string, ctx: ExtensionCommandContext) => {
            const parsed = parseArgs(args);
            const repo = parsed.repo ?? DEFAULT_REPO;
            const intervalMin = parsed.intervalMin ?? DEFAULT_INTERVAL_MIN;
            const prompt = parsed.prompt ?? null;
            const intervalMs = intervalMin * 60_000;

            const replacedExisting = stopActivePoller();
            const stopped = createStoppedPromise();
            const state: PollState = {
                repo,
                intervalMs,
                prompt,
                seen: new Set<number>(),
                timer: null,
                abortController: null,
                running: false,
                stopped: false,
                resolveStopped: stopped.resolve,
                lastOutputAt: null,
                nextRunAt: null,
            };
            active = state;
            updatePollerUi(ctx, state);
            ctx.ui.notify(
                withScheduleLines(
                    `[gh-issues] ${replacedExisting ? "已停止旧轮询器并" : ""}监听 ${repo}，每 ${intervalMin} 分钟一次${
                        prompt ? "（新 issue 将入队为用户消息）" : "（仅通知）"
                    }；按 Esc 停止`,
                    state,
                ),
                "info",
            );

            const unsubscribeInput = ctx.ui.onTerminalInput((data) => {
                if (data !== "\u001b") return undefined;
                stopActivePoller();
                ctx.ui.notify("[gh-issues] 已通过 Esc 停止", "info");
                return { consume: true };
            });

            const tick = async (): Promise<void> => {
                if (!isActivePoller(state) || state.running) return;
                const abortController = new AbortController();
                state.running = true;
                state.abortController = abortController;
                state.nextRunAt = null;
                updatePollerUi(ctx, state);

                try {
                    const issues = await fetchIssues(pi, repo, ctx.cwd, abortController.signal);
                    if (!isActivePoller(state)) return;

                    const fresh = issues.filter((i) => !state.seen.has(i.number));
                    for (const i of fresh) state.seen.add(i.number);

                    if (fresh.length > 0) {
                        markOutputAndNextRun(state);
                        if (prompt) {
                            const message = formatPrompt(repo, fresh, prompt);
                            pi.sendUserMessage(message, { deliverAs: "followUp" });
                            ctx.ui.notify(
                                withScheduleLines(
                                    `[gh-issues] ${fresh.length} 个新 issue 已入队（带提示词），按 Enter 提交`,
                                    state,
                                ),
                                "info",
                            );
                        } else {
                            ctx.ui.notify(withScheduleLines(formatNotify(fresh), state), "info");
                        }
                    } else {
                        markOutputAndNextRun(state);
                        ctx.ui.notify(
                            withScheduleLines(
                                `[gh-issues] 本轮检查完成，未发现新 issue（已跟踪 ${state.seen.size} 个开放 issue）`,
                                state,
                            ),
                            "info",
                        );
                    }
                    updatePollerUi(ctx, state);
                } catch (err) {
                    if (abortController.signal.aborted) return;
                    const msg = err instanceof Error ? err.message : String(err);
                    markOutputAndNextRun(state);
                    ctx.ui.notify(withScheduleLines(`[gh-issues] 轮询失败：${msg}`, state), "error");
                    updatePollerUi(ctx, state);
                } finally {
                    state.running = false;
                    if (state.abortController === abortController) {
                        state.abortController = null;
                    }
                    if (isActivePoller(state)) {
                        if (!state.nextRunAt) {
                            state.nextRunAt = new Date(Date.now() + state.intervalMs);
                        }
                        updatePollerUi(ctx, state);
                        state.timer = setTimeout(tick, state.intervalMs);
                    }
                }
            };

            // 首轮立即在后台执行；handler 保持未完成，让主 OMP 维持 working 状态；Esc/stop/新命令会 resolve
            void tick();
            try {
                await stopped.promise;
            } finally {
                unsubscribeInput();
                const shouldClearUi = active === state || active === null;
                if (active === state) {
                    stopActivePoller();
                }
                if (shouldClearUi) {
                    ctx.ui.setStatus("gh-issues", undefined);
                    ctx.ui.setWorkingMessage();
                }
            }
        },
    });

    pi.registerCommand("gh-issues-stop", {
        description: "停止 /gh-issues 轮询器",
        handler: async (_args: string, ctx: ExtensionCommandContext) => {
            if (!stopActivePoller()) {
                ctx.ui.notify("[gh-issues] 当前无活跃轮询器", "info");
                return;
            }
            ctx.ui.notify("[gh-issues] 已停止", "info");
        },
    });
}

/**
 * 解析 `/gh-issues` 的参数串。
 *
 * 规则：每个 token 按内容形状自动归类。
 * - 形如 `owner/repo`（匹配 `^[\w.-]+\/[\w.-]+$` 且未带引号）→ repo
 * - 正整数（未带引号）→ intervalMin
 * - 其余一律归入 prompt（多个 token 用空格连接）
 *
 * 带双引号包裹的 token 强制视为 prompt，这样像 `"请分析 https://x.com/foo"`
 * 这种带 `/` 的中文 prompt 也不会被错认为 repo。
 *
 * 用户可以填任一个、任两个或全部三个参数；未填的用默认值。
 *
 * 例：
 * - `/gh-issues`                                            → {}
 * - `/gh-issues "请逐个分析"`                                 → { prompt: "请逐个分析" }
 * - `/gh-issues 30 "请逐个分析"`                              → { intervalMin: 30, prompt: "请逐个分析" }
 * - `/gh-issues owner/repo 30 "请逐个分析"`                   → { repo, intervalMin, prompt }
 * - `/gh-issues "请分析 https://x.com/foo"`                   → { prompt: "请分析 https://x.com/foo" }
 */
export function parseArgs(input: string): ParsedArgs {
    const out: ParsedArgs = {};
    const trimmed = input.trim();
    if (!trimmed) return out;

    const tokens = tokenizeWithQuotes(trimmed);
    if (tokens.length === 0) return out;

    const promptParts: string[] = [];

    for (const { text, quoted } of tokens) {
        if (!quoted && out.repo === undefined && GITHUB_REPO_RE.test(text)) {
            out.repo = text;
        } else if (!quoted && out.intervalMin === undefined && POSITIVE_INT_RE.test(text)) {
            out.intervalMin = Math.floor(Number(text));
        } else {
            promptParts.push(text);
        }
    }

    if (promptParts.length > 0) {
        out.prompt = promptParts.join(" ");
    }

    return out;
}

const GITHUB_REPO_RE = /^[\w.-]+\/[\w.-]+$/;
// 仅匹配正整数（不允许 0 或前导零；不匹配浮点与负数）
const POSITIVE_INT_RE = /^[1-9]\d*$/;

interface Token {
    text: string;
    quoted: boolean;
}

function tokenizeWithQuotes(input: string): Token[] {
    const tokens: Token[] = [];
    let buf = "";
    let inQuote = false;
    let bufWasQuoted = false;
    for (let i = 0; i < input.length; i++) {
        const c = input[i];
        if (c === '"') {
            inQuote = !inQuote;
            bufWasQuoted = true;
            continue;
        }
        if (!inQuote && /\s/.test(c)) {
            if (buf) {
                tokens.push({ text: buf, quoted: bufWasQuoted });
                buf = "";
                bufWasQuoted = false;
            }
            continue;
        }
        buf += c;
    }
    if (buf) tokens.push({ text: buf, quoted: bufWasQuoted });
    return tokens;
}

async function fetchIssues(
    pi: ExtensionAPI,
    repo: string,
    cwd: string,
    signal: AbortSignal,
): Promise<Issue[]> {
    const result: ExecResult = await pi.exec(
        "gh",
        [
            "issue",
            "list",
            "--repo",
            repo,
            "--state",
            "open",
            "--limit",
            String(ISSUES_PER_PAGE),
            "--json",
            "number,title,body,url,createdAt,author,labels",
        ],
        { cwd, signal },
    );

    if (result.killed) throw new Error("gh 命令被取消");
    if (result.code !== 0) {
        throw new Error(result.stderr.trim() || `gh 退出码 ${result.code}`);
    }

    const parsed: unknown = JSON.parse(result.stdout);
    if (!Array.isArray(parsed)) throw new Error("gh 返回非数组");
    return parsed.filter(isIssue);
}

/** 类型守卫：过滤出符合 Issue 形状的条目（gh 偶尔返回混合数组） */
function isIssue(value: unknown): value is Issue {
    if (typeof value !== "object" || value === null) return false;
    const v = value as Record<string, unknown>;
    return (
        typeof v.number === "number" &&
        typeof v.title === "string" &&
        typeof v.url === "string"
    );
}

function formatNotify(issues: ReadonlyArray<Issue>): string {
    const head = issues
        .slice(0, 5)
        .map((i) => `  #${i.number} ${i.title}`)
        .join("\n");
    const more = issues.length > 5 ? `\n  ... 还有 ${issues.length - 5} 个` : "";
    return `[gh-issues] ${issues.length} 个新 issue：\n${head}${more}`;
}

function formatPrompt(
    repo: string,
    issues: ReadonlyArray<Issue>,
    prompt: string | null,
): string {
    const sections = issues
        .map((i) => {
            const labels = (i.labels ?? []).map((l) => l.name).join(", ") || "(无)";
            const author = i.author?.login ?? "unknown";
            const bodyText = i.body ?? "";
            const excerpt = bodyText.slice(0, 200).replace(/\s+/g, " ").trim();
            const excerptLine = excerpt
                ? `摘要: ${excerpt}${bodyText.length > 200 ? "…" : ""}`
                : "(无描述)";
            return [
                `#${i.number} — ${i.title}`,
                `URL: ${i.url}`,
                `作者: ${author}  标签: ${labels}  创建: ${i.createdAt}`,
                excerptLine,
            ].join("\n");
        })
        .join("\n\n---\n\n");

    const header = `[gh-issues] 仓库 ${repo} 新增 ${issues.length} 个 issue：`;
    return prompt
        ? `${header}\n\n${sections}\n\n---\n\n${prompt}`
        : `${header}\n\n${sections}`;
}
