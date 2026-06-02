import type {
    ExecOptions,
    ExecResult,
    ExtensionAPI,
    ExtensionCommandContext,
} from "@oh-my-pi/pi-coding-agent";
import { getKeybindings, matchesKey } from "@oh-my-pi/pi-tui";

export interface IssueAuthor {
    login: string;
}

export interface IssueLabel {
    name: string;
}

export interface Issue {
    number: number;
    title: string;
    url: string;
    body: string;
    createdAt: string;
    author: IssueAuthor | null;
    labels: IssueLabel[];
}

export interface ParsedArgs {
    repo?: string;
    intervalMin?: number;
    prompt?: string;
}

interface WatcherConfig {
    repo: string;
    intervalMin: number;
    intervalMs: number;
    prompt: string | null;
}

interface WatcherStatus {
    seenIssueNumbers: Set<number>;
    lastOutputAt: Date | null;
    nextRunAt: Date | null;
    running: boolean;
    stopped: boolean;
}

interface DeferredStop {
    promise: Promise<void>;
    resolve(): void;
}

interface InterruptKeybindings {
    getDefinition(keybinding: string): unknown;
    getKeys(keybinding: string): string[];
    matches(data: string, keybinding: string): boolean;
}

interface PollResult {
    freshIssues: Issue[];
}

interface GhIssuesPromptDetails {
    repo: string;
    issueNumbers: number[];
    prompt: string;
}


const DEFAULT_REPO = "IZRINO/delta-auto-tools";
const DEFAULT_INTERVAL_MIN = 60;
const ISSUES_PER_PAGE = 30;
const GH_ISSUES_PROMPT_TYPE = "gh-issues-prompt";
const APP_INTERRUPT_KEYBINDING = "app.interrupt";
const MODIFY_OTHER_KEYS_ESCAPE = "\u001b[27;1;27~";
const GITHUB_REPO_RE = /^[\w.-]+\/[\w.-]+$/;
const POSITIVE_INT_RE = /^[1-9]\d*$/;
const KEY_HINT_LABELS: Record<string, string> = {
    alt: "Alt",
    ctrl: "Ctrl",
    esc: "Esc",
    escape: "Esc",
    shift: "Shift",
    super: "Super",
};

let activeWatcher: GhIssuesWatcher | undefined;

export default function registerGhIssues(pi: ExtensionAPI): void {
    pi.on("session_shutdown", () => {
        stopActiveWatcher();
    });

    pi.registerCommand("gh-issues", {
        description: commandDescription(),
        handler: async (args: string, ctx: ExtensionCommandContext) => {
            const config = configFromArgs(args);
            const replacedExisting = stopActiveWatcher();
            const watcher = new GhIssuesWatcher(pi, ctx, config);
            activeWatcher = watcher;
            await watcher.run(replacedExisting);
        },
    });

    pi.registerCommand("gh-issues-stop", {
        description: "停止 /gh-issues 轮询器",
        handler: async (_args: string, ctx: ExtensionCommandContext) => {
            if (!stopActiveWatcher()) {
                ctx.ui.notify("[gh-issues] 当前无活跃轮询器", "info");
                return;
            }
            ctx.ui.notify("[gh-issues] 已停止", "info");
        },
    });
}

class GhIssuesWatcher {
    readonly #pi: ExtensionAPI;
    readonly #ctx: ExtensionCommandContext;
    readonly #config: WatcherConfig;
    readonly #stopped: DeferredStop;
    readonly #status: WatcherStatus;
    #timer: Timer | undefined;
    #abortController: AbortController | undefined;
    #unsubscribeInput: (() => void) | undefined;

    constructor(pi: ExtensionAPI, ctx: ExtensionCommandContext, config: WatcherConfig) {
        this.#pi = pi;
        this.#ctx = ctx;
        this.#config = config;
        this.#stopped = createDeferredStop();
        this.#status = {
            seenIssueNumbers: new Set<number>(),
            lastOutputAt: null,
            nextRunAt: null,
            running: false,
            stopped: false,
        };
    }

    async run(replacedExisting: boolean): Promise<void> {
        this.#bindInterruptKey();
        this.#renderStatus();
        this.#notifyStarted(replacedExisting);
        void this.#pollOnce();

        try {
            await this.#stopped.promise;
        } finally {
            this.#cleanup();
        }
    }

    stop(): void {
        if (this.#status.stopped) return;
        this.#status.stopped = true;
        this.#clearTimer();
        this.#abortController?.abort();
        this.#abortController = undefined;
        this.#stopped.resolve();
    }

    #bindInterruptKey(): void {
        this.#unsubscribeInput = this.#ctx.ui.onTerminalInput((data) => {
            if (!isInterruptInput(data)) return undefined;
            stopActiveWatcher();
            this.#ctx.ui.notify("[gh-issues] 已通过中断键停止", "info");
            return { consume: true };
        });
    }

    async #pollOnce(): Promise<void> {
        if (!this.#isActive() || this.#status.running) return;

        const abortController = new AbortController();
        this.#abortController = abortController;
        this.#status.running = true;
        this.#status.nextRunAt = null;
        this.#renderStatus();

        try {
            const result = await this.#fetchAndClassify(abortController.signal);
            if (!this.#isActive()) return;
            this.#markOutput();
            this.#emitPollResult(result);
            this.#renderStatus();
        } catch (error: unknown) {
            if (abortController.signal.aborted) return;
            this.#markOutput();
            this.#ctx.ui.notify(this.#withSchedule(`轮询失败：${getErrorMessage(error)}`), "error");
            this.#renderStatus();
        } finally {
            this.#status.running = false;
            if (this.#abortController === abortController) {
                this.#abortController = undefined;
            }
            if (this.#isActive()) {
                this.#scheduleNextPoll();
            }
        }
    }

    async #fetchAndClassify(signal: AbortSignal): Promise<PollResult> {
        const issues = await fetchIssues(this.#pi, this.#config.repo, this.#ctx.cwd, signal);
        const freshIssues = issues.filter((issue) => !this.#status.seenIssueNumbers.has(issue.number));
        for (const issue of freshIssues) {
            this.#status.seenIssueNumbers.add(issue.number);
        }
        return { freshIssues };
    }

    #emitPollResult(result: PollResult): void {
        if (result.freshIssues.length === 0) {
            this.#ctx.ui.notify(
                this.#withSchedule(`本轮检查完成，未发现新 issue（已跟踪 ${this.#status.seenIssueNumbers.size} 个开放 issue）`),
                "info",
            );
            return;
        }

        if (!this.#config.prompt) {
            this.#ctx.ui.notify(this.#withSchedule(formatIssueNotification(result.freshIssues)), "info");
            return;
        }

        const prompt = this.#config.prompt;
        this.#pi.sendMessage<GhIssuesPromptDetails>(
            {
                customType: GH_ISSUES_PROMPT_TYPE,
                content: formatAgentPrompt(this.#config.repo, result.freshIssues, prompt),
                display: true,
                details: {
                    repo: this.#config.repo,
                    issueNumbers: result.freshIssues.map((issue) => issue.number),
                    prompt,
                },
                attribution: "user",
            },
            { deliverAs: "nextTurn", triggerTurn: true },
        );
        this.#ctx.ui.notify(
            this.#withSchedule(`${result.freshIssues.length} 个新 issue 已自动触发 Agent 执行（带提示词）`),
            "info",
        );
    }

    #scheduleNextPoll(): void {
        if (!this.#status.nextRunAt) {
            this.#status.nextRunAt = new Date(Date.now() + this.#config.intervalMs);
        }
        this.#renderStatus();
        this.#timer = setTimeout(() => {
            void this.#pollOnce();
        }, this.#config.intervalMs);
    }

    #notifyStarted(replacedExisting: boolean): void {
        const mode = this.#config.prompt ? "新 issue 将自动触发 Agent 执行" : "仅通知";
        const prefix = replacedExisting ? "已停止旧轮询器并" : "";
        this.#ctx.ui.notify(
            this.#withSchedule(`${prefix}监听 ${this.#config.repo}，每 ${this.#config.intervalMin} 分钟一次（${mode}）；按 ${formatInterruptHint()} 停止`),
            "info",
        );
    }

    #renderStatus(): void {
        const intervalText = formatInterval(this.#config.intervalMs);
        const nextRun = describeNextRun(this.#status);
        const lastOutput = formatTimestamp(this.#status.lastOutputAt);
        const interruptHint = formatInterruptHint();
        this.#ctx.ui.setWorkingMessage(
            `[gh-issues] 监听 ${this.#config.repo}，每 ${intervalText}；上次输出：${lastOutput}；下次运行：${nextRun}；按 ${interruptHint} 停止`,
        );
        this.#ctx.ui.setStatus(
            "gh-issues",
            `${this.#config.repo} 每 ${intervalText}；上次输出 ${lastOutput}；下次运行 ${nextRun}；${interruptHint} 停止`,
        );
    }

    #markOutput(): void {
        const now = new Date();
        this.#status.lastOutputAt = now;
        this.#status.nextRunAt = new Date(now.getTime() + this.#config.intervalMs);
    }

    #withSchedule(message: string): string {
        return `[gh-issues] ${message}\n上次输出：${formatTimestamp(this.#status.lastOutputAt)}\n下次运行：${describeNextRun(this.#status)}`;
    }

    #isActive(): boolean {
        return activeWatcher === this && !this.#status.stopped;
    }

    #cleanup(): void {
        this.#unsubscribeInput?.();
        this.#unsubscribeInput = undefined;
        this.#clearTimer();
        if (activeWatcher === this) {
            activeWatcher = undefined;
        }
        if (!activeWatcher) {
            this.#ctx.ui.setStatus("gh-issues", undefined);
            this.#ctx.ui.setWorkingMessage();
        }
    }

    #clearTimer(): void {
        if (!this.#timer) return;
        clearTimeout(this.#timer);
        this.#timer = undefined;
    }
}

export function parseArgs(input: string): ParsedArgs {
    const result: ParsedArgs = {};
    const promptParts: string[] = [];

    for (const token of tokenizeArgs(input.trim())) {
        if (!token.quoted && result.repo === undefined && GITHUB_REPO_RE.test(token.text)) {
            result.repo = token.text;
            continue;
        }
        if (!token.quoted && result.intervalMin === undefined && POSITIVE_INT_RE.test(token.text)) {
            result.intervalMin = Number(token.text);
            continue;
        }
        promptParts.push(token.text);
    }

    if (promptParts.length > 0) {
        result.prompt = promptParts.join(" ");
    }
    return result;
}

function configFromArgs(input: string): WatcherConfig {
    const parsed = parseArgs(input);
    const repo = parsed.repo ?? DEFAULT_REPO;
    const intervalMin = parsed.intervalMin ?? DEFAULT_INTERVAL_MIN;
    return {
        repo,
        intervalMin,
        intervalMs: intervalMin * 60_000,
        prompt: parsed.prompt ?? null,
    };
}

function stopActiveWatcher(): boolean {
    const watcher = activeWatcher;
    if (!watcher) return false;
    activeWatcher = undefined;
    watcher.stop();
    return true;
}

async function fetchIssues(pi: ExtensionAPI, repo: string, cwd: string, signal: AbortSignal): Promise<Issue[]> {
    const execOptions: ExecOptions = { cwd, signal };
    const result = await pi.exec(
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
        execOptions,
    );

    assertExecSucceeded(result);
    return parseIssues(result.stdout);
}

function assertExecSucceeded(result: ExecResult): void {
    if (result.killed) {
        throw new Error("gh 命令被取消");
    }
    if (result.code !== 0) {
        throw new Error(result.stderr.trim() || `gh 退出码 ${result.code}`);
    }
}

function parseIssues(stdout: string): Issue[] {
    let parsed: unknown;
    try {
        parsed = JSON.parse(stdout);
    } catch (error: unknown) {
        throw new Error(`gh 返回 JSON 无法解析：${getErrorMessage(error)}`);
    }
    if (!Array.isArray(parsed)) {
        throw new Error("gh 返回非数组");
    }

    const issues: Issue[] = [];
    for (const item of parsed) {
        const issue = parseIssue(item);
        if (issue) issues.push(issue);
    }
    return issues;
}

function parseIssue(value: unknown): Issue | undefined {
    if (!isRecord(value)) return undefined;
    if (typeof value.number !== "number" || !Number.isFinite(value.number)) return undefined;
    if (typeof value.title !== "string" || typeof value.url !== "string") return undefined;

    return {
        number: value.number,
        title: value.title,
        url: value.url,
        body: typeof value.body === "string" ? value.body : "",
        createdAt: typeof value.createdAt === "string" ? value.createdAt : "",
        author: parseIssueAuthor(value.author),
        labels: Array.isArray(value.labels) ? value.labels.filter(isIssueLabel) : [],
    };
}

function parseIssueAuthor(value: unknown): IssueAuthor | null {
    if (!isRecord(value) || typeof value.login !== "string") return null;
    return { login: value.login };
}

function isIssueLabel(value: unknown): value is IssueLabel {
    return isRecord(value) && typeof value.name === "string";
}

function isRecord(value: unknown): value is Record<string, unknown> {
    return typeof value === "object" && value !== null;
}

interface Token {
    text: string;
    quoted: boolean;
}

function tokenizeArgs(input: string): Token[] {
    const tokens: Token[] = [];
    let text = "";
    let quote: '"' | "'" | null = null;
    let quoted = false;

    for (const char of input) {
        if (char === '"' || char === "'") {
            if (!quote) {
                quote = char;
                quoted = true;
                continue;
            }
            if (quote === char) {
                quote = null;
                continue;
            }
        }
        if (!quote && /\s/.test(char)) {
            pushToken(tokens, text, quoted);
            text = "";
            quoted = false;
            continue;
        }
        text += char;
    }
    pushToken(tokens, text, quoted);
    return tokens;
}

function pushToken(tokens: Token[], text: string, quoted: boolean): void {
    if (text.length === 0) return;
    tokens.push({ text, quoted });
}

function formatIssueNotification(issues: readonly Issue[]): string {
    const head = issues
        .slice(0, 5)
        .map((issue) => `  #${issue.number} ${issue.title}`)
        .join("\n");
    const more = issues.length > 5 ? `\n  ... 还有 ${issues.length - 5} 个` : "";
    return `${issues.length} 个新 issue：\n${head}${more}`;
}

function formatAgentPrompt(repo: string, issues: readonly Issue[], prompt: string): string {
    const sections = issues.map(formatIssueForPrompt).join("\n\n---\n\n");
    return [
        `[gh-issues] 仓库 ${repo} 新增 ${issues.length} 个 issue。以下消息由 /gh-issues 自动触发，请根据 issue 列表直接执行用户提示，不要等待用户再次确认：`,
        sections,
        "---",
        "用户提示：",
        prompt,
    ].join("\n\n");
}

function formatIssueForPrompt(issue: Issue): string {
    const labels = issue.labels.map((label) => label.name).join(", ") || "(无)";
    const author = issue.author?.login ?? "unknown";
    const excerpt = issue.body.slice(0, 200).replace(/\s+/g, " ").trim();
    const description = excerpt ? `摘要: ${excerpt}${issue.body.length > 200 ? "…" : ""}` : "(无描述)";
    return [
        `#${issue.number} — ${issue.title}`,
        `URL: ${issue.url}`,
        `作者: ${author}  标签: ${labels}  创建: ${issue.createdAt}`,
        description,
    ].join("\n");
}

function createDeferredStop(): DeferredStop {
    let resolveStop!: () => void;
    const promise = new Promise<void>((resolve) => {
        resolveStop = resolve;
    });
    return { promise, resolve: resolveStop };
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

function formatInterval(intervalMs: number): string {
    const minutes = intervalMs / 60_000;
    return Number.isInteger(minutes) ? `${minutes} 分钟` : `${Math.round(intervalMs / 1000)} 秒`;
}

function describeNextRun(status: WatcherStatus): string {
    if (status.nextRunAt) return formatTimestamp(status.nextRunAt);
    return status.running ? "正在执行" : "立即执行";
}

function getInterruptKeybindings(): InterruptKeybindings {
    return getKeybindings() as unknown as InterruptKeybindings;
}

function getInterruptKeys(): string[] {
    try {
        return getInterruptKeybindings().getKeys(APP_INTERRUPT_KEYBINDING);
    } catch {
        return [];
    }
}

function hasInterruptKeybinding(): boolean {
    try {
        return getInterruptKeybindings().getDefinition(APP_INTERRUPT_KEYBINDING) !== undefined;
    } catch {
        return false;
    }
}

function isInterruptInput(data: string): boolean {
    const keybindings = getInterruptKeybindings();
    if (hasInterruptKeybinding()) {
        const keys = getInterruptKeys();
        if (keys.length === 0) return false;
        if (keybindings.matches(data, APP_INTERRUPT_KEYBINDING)) return true;
        return data === MODIFY_OTHER_KEYS_ESCAPE && keys.some(isEscapeKey);
    }
    return data === MODIFY_OTHER_KEYS_ESCAPE || matchesKey(data, "escape") || matchesKey(data, "esc");
}

function isEscapeKey(key: string): boolean {
    const normalized = key.toLowerCase();
    return normalized === "escape" || normalized === "esc";
}

function formatInterruptHint(): string {
    const keys = getInterruptKeys();
    if (hasInterruptKeybinding() && keys.length === 0) return "已禁用的中断键";
    return keys.length === 0 ? "Esc" : keys.map(formatKeyHint).join("/");
}

function formatKeyHint(key: string): string {
    return key
        .split("+")
        .map((part) => {
            const label = KEY_HINT_LABELS[part.toLowerCase()];
            if (label) return label;
            return part.length === 1 ? part.toUpperCase() : part;
        })
        .join("+");
}

function getErrorMessage(error: unknown): string {
    return error instanceof Error ? error.message : String(error);
}

function commandDescription(): string {
    return [
        "监听 GitHub 仓库的 issue。",
        "参数（按内容自动归类，未填用默认；带引号的 token 一律视为 prompt）：",
        "  /gh-issues                                          # 仅通知（默认本仓库、60 分钟）",
        "  /gh-issues owner/repo                               # 自定义仓库 + 通知",
        "  /gh-issues 30                                       # 自定义间隔 + 通知",
        "  /gh-issues \"请逐个分析\"                            # 自定义 prompt + 自动触发 Agent",
        "  /gh-issues owner/repo 30                            # 自定义仓库 + 间隔",
        "  /gh-issues owner/repo 30 \"请逐个分析\"              # 全部自定义",
    ].join("\n");
}
