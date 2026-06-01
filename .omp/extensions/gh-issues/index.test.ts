import { describe, expect, test } from "bun:test";
import registerGhIssues, { parseArgs } from "./index.ts";
import type {
    ExecOptions,
    ExecResult,
    ExtensionAPI,
    ExtensionCommandContext,
} from "@oh-my-pi/pi-coding-agent";

interface Deferred<T> {
    promise: Promise<T>;
    resolve(value: T): void;
    reject(reason?: unknown): void;
}

type TimerCallback = (...args: unknown[]) => void;
type TerminalHandler = (data: string) => { consume?: boolean; data?: string } | undefined;

interface CapturedTimer {
    callback: () => void;
    delay: number | undefined;
}

interface TimerCapture {
    scheduled: CapturedTimer[];
    restore(): void;
}

interface CommandRegistration {
    description?: string;
    handler(args: string, ctx: ExtensionCommandContext): Promise<void>;
}

interface NotificationEntry {
    message: string;
    type: "info" | "warning" | "error" | undefined;
}

interface SentMessage {
    message: {
        customType: string;
        content: string | unknown[];
        display: boolean;
        details?: unknown;
        attribution?: string;
    };
    options?: {
        triggerTurn?: boolean;
        deliverAs?: string;
    };
}

interface ExtensionHarness {
    commands: Record<string, CommandRegistration>;
    ctx: ExtensionCommandContext;
    terminalHandlers: TerminalHandler[];
    workingMessages: Array<string | undefined>;
    statuses: Array<{ key: string; text: string | undefined }>;
    notifications: NotificationEntry[];
    sentMessages: SentMessage[];
}

function createDeferred<T>(): Deferred<T> {
    let resolve!: (value: T) => void;
    let reject!: (reason?: unknown) => void;
    const promise = new Promise<T>((resolvePromise, rejectPromise) => {
        resolve = resolvePromise;
        reject = rejectPromise;
    });
    return { promise, resolve, reject };
}

function issuesResult(issues: ReadonlyArray<unknown>): ExecResult {
    return { stdout: JSON.stringify(issues), stderr: "", code: 0, killed: false };
}

function emptyIssuesResult(): ExecResult {
    return issuesResult([]);
}

function sampleIssue(number: number, title = `测试 issue ${number}`): unknown {
    return {
        number,
        title,
        url: `https://github.com/foo/bar/issues/${number}`,
        body: "测试内容",
        createdAt: "2026-06-01T00:00:00Z",
        author: { login: "tester" },
        labels: [],
    };
}


async function flushMicrotasks(): Promise<void> {
    await Promise.resolve();
    await Promise.resolve();
}
function installTimerCapture(): TimerCapture {
    const scheduled: CapturedTimer[] = [];
    const originalSetTimeout = globalThis.setTimeout;
    const originalClearTimeout = globalThis.clearTimeout;

    globalThis.setTimeout = ((callback: TimerCallback, delay?: number) => {
        scheduled.push({ callback: callback as () => void, delay });
        return scheduled.length as unknown as ReturnType<typeof setTimeout>;
    }) as typeof setTimeout;

    globalThis.clearTimeout = ((_handle?: ReturnType<typeof setTimeout>) => undefined) as typeof clearTimeout;

    return {
        scheduled,
        restore() {
            globalThis.setTimeout = originalSetTimeout;
            globalThis.clearTimeout = originalClearTimeout;
        },
    };
}

function createHarness(
    exec: (command: string, args: string[], options?: ExecOptions) => Promise<ExecResult>,
): ExtensionHarness {
    const commands: Record<string, CommandRegistration> = {};
    const terminalHandlers: TerminalHandler[] = [];
    const workingMessages: Array<string | undefined> = [];
    const statuses: Array<{ key: string; text: string | undefined }> = [];
    const notifications: NotificationEntry[] = [];
    const sentMessages: SentMessage[] = [];
    const pi = {
        on: () => undefined,
        registerCommand: (name: string, options: CommandRegistration) => {
            commands[name] = options;
        },
        exec,
        sendMessage: (message: SentMessage["message"], options?: SentMessage["options"]) => {
            sentMessages.push({ message, options });
        },
        sendUserMessage: () => undefined,
    } as unknown as ExtensionAPI;

    const ctx = {
        cwd: "D:/Project/rust/delta-auto-tools",
        ui: {
            notify: (message: string, type?: "info" | "warning" | "error") => {
                notifications.push({ message, type });
            },
            onTerminalInput: (handler: TerminalHandler) => {
                terminalHandlers.push(handler);
                return () => {
                    const index = terminalHandlers.indexOf(handler);
                    if (index !== -1) terminalHandlers.splice(index, 1);
                };
            },
            setStatus: (key: string, text: string | undefined) => {
                statuses.push({ key, text });
            },
            setWorkingMessage: (message?: string) => {
                workingMessages.push(message);
            },
        },
        isIdle: () => true,
    } as unknown as ExtensionCommandContext;

    registerGhIssues(pi);
    return { commands, ctx, terminalHandlers, workingMessages, statuses, notifications, sentMessages };
}

describe("parseArgs", () => {
    test("uses defaults when only prompt is provided", () => {
        expect(parseArgs("\"请逐个分析\"")).toEqual({ prompt: "请逐个分析" });
    });

    test("classifies repo interval and prompt by shape", () => {
        expect(parseArgs("owner/repo 30 \"请逐个分析\"")).toEqual({
            repo: "owner/repo",
            intervalMin: 30,
            prompt: "请逐个分析",
        });
    });

    test("supports interval plus prompt without repo", () => {
        expect(parseArgs("30 \"请逐个分析\"")).toEqual({
            intervalMin: 30,
            prompt: "请逐个分析",
        });
    });

    test("quoted slash content stays prompt", () => {
        expect(parseArgs("\"请分析 https://x.com/foo\"")).toEqual({
            prompt: "请分析 https://x.com/foo",
        });
    });

    test("non-positive and non-integer numbers stay prompt", () => {
        expect(parseArgs("0")).toEqual({ prompt: "0" });
        expect(parseArgs("-5")).toEqual({ prompt: "-5" });
        expect(parseArgs("30.7")).toEqual({ prompt: "30.7" });
    });

    test("extra prompt tokens are preserved", () => {
        expect(parseArgs("foo/bar 30 p1 extra")).toEqual({
            repo: "foo/bar",
            intervalMin: 30,
            prompt: "p1 extra",
        });
    });
});

describe("gh-issues poller", () => {
    test("starts the next interval only after the current poll finishes", async () => {
        const timers = installTimerCapture();
        const deferred = createDeferred<ExecResult>();
        const harness = createHarness(() => deferred.promise);

        try {
            const run = harness.commands["gh-issues"].handler("foo/bar 1", harness.ctx);
            await flushMicrotasks();
            expect(timers.scheduled).toHaveLength(0);
            expect(harness.workingMessages[0]).toContain("按 Esc 停止");
            expect(harness.statuses[0]?.key).toBe("gh-issues");
            expect(harness.statuses[0]?.text).toContain("foo/bar 每 1 分钟");
            expect(harness.statuses[0]?.text).toContain("上次输出 尚无");
            expect(harness.statuses[0]?.text).toContain("下次运行 立即执行");

            deferred.resolve(emptyIssuesResult());
            await flushMicrotasks();

            expect(timers.scheduled).toHaveLength(1);
            expect(timers.scheduled[0]?.delay).toBe(60_000);
            expect(harness.notifications.at(-1)?.message).toContain("上次输出：");
            expect(harness.notifications.at(-1)?.message).toContain("下次运行：");
            expect(harness.statuses.at(-1)?.text).toContain("下次运行");
            expect(harness.terminalHandlers[0]?.("\u001b")).toEqual({ consume: true });
            await run;
            expect(harness.workingMessages.at(-1)).toBeUndefined();
            expect(harness.statuses.at(-1)).toEqual({ key: "gh-issues", text: undefined });
        } finally {
            timers.restore();
        }
    });
    test("notifies when a later interval has no new issues", async () => {
        const timers = installTimerCapture();
        const results = [
            issuesResult([sampleIssue(1, "首个 issue")]),
            issuesResult([sampleIssue(1, "首个 issue")]),
        ];
        const harness = createHarness(() => Promise.resolve(results.shift() ?? emptyIssuesResult()));

        try {
            const run = harness.commands["gh-issues"].handler("foo/bar 3", harness.ctx);
            await flushMicrotasks();

            expect(harness.notifications.some((n) => n.message.includes("1 个新 issue"))).toBe(true);
            expect(timers.scheduled).toHaveLength(1);
            expect(timers.scheduled[0]?.delay).toBe(180_000);

            timers.scheduled[0]?.callback();
            await flushMicrotasks();

            expect(
                harness.notifications.some((n) =>
                    n.message.includes("本轮检查完成，未发现新 issue") &&
                    n.message.includes("上次输出：") &&
                    n.message.includes("下次运行："),
                ),
            ).toBe(true);
            expect(timers.scheduled[1]?.delay).toBe(180_000);

            harness.terminalHandlers[0]?.("\u001b");
            await run;
        } finally {
            timers.restore();
        }
    });

    test("notifies new issues discovered by a later interval", async () => {
        const timers = installTimerCapture();
        const results = [
            emptyIssuesResult(),
            issuesResult([sampleIssue(2, "三分钟后新增")]),
        ];
        const harness = createHarness(() => Promise.resolve(results.shift() ?? emptyIssuesResult()));

        try {
            const run = harness.commands["gh-issues"].handler("foo/bar 3", harness.ctx);
            await flushMicrotasks();

            expect(timers.scheduled[0]?.delay).toBe(180_000);

            timers.scheduled[0]?.callback();
            await flushMicrotasks();

            expect(harness.notifications.some((n) => n.message.includes("#2 三分钟后新增"))).toBe(true);
            expect(timers.scheduled[1]?.delay).toBe(180_000);

            harness.terminalHandlers[0]?.("\u001b");
            await run;
        } finally {
            timers.restore();
        }
    });

    test("auto-triggers agent execution when prompt is provided", async () => {
        const timers = installTimerCapture();
        const harness = createHarness(() => Promise.resolve(issuesResult([sampleIssue(7, "需要自动处理")])));

        try {
            const prompt = "读取待办的issues并处理，根据开发流程完成开发后更新版本号，使用bun run tauri build打包上传release。回复issues并关闭；";
            const run = harness.commands["gh-issues"].handler(`10 "${prompt}"`, harness.ctx);
            await flushMicrotasks();

            expect(harness.sentMessages).toHaveLength(1);
            expect(harness.sentMessages[0]?.message).toMatchObject({
                customType: "gh-issues-prompt",
                display: true,
                attribution: "user",
                details: {
                    repo: "IZRINO/delta-auto-tools",
                    issueNumbers: [7],
                    prompt,
                },
            });
            expect(harness.sentMessages[0]?.message.content).toContain("不要等待用户再次确认");
            expect(harness.sentMessages[0]?.message.content).toContain("用户提示：");
            expect(harness.sentMessages[0]?.message.content).toContain(prompt);
            expect(harness.sentMessages[0]?.options).toEqual({ deliverAs: "nextTurn", triggerTurn: true });
            expect(harness.notifications.some((n) => n.message.includes("已自动触发 Agent 执行"))).toBe(true);

            harness.terminalHandlers[0]?.("\u001b");
            await run;
        } finally {
            timers.restore();
        }
    });


    test("starting a new watcher aborts the old running poller", async () => {
        const timers = installTimerCapture();
        const deferreds: Deferred<ExecResult>[] = [];
        const signals: AbortSignal[] = [];
        const harness = createHarness((_command, _args, options) => {
            if (options?.signal) signals.push(options.signal);
            const deferred = createDeferred<ExecResult>();
            deferreds.push(deferred);
            return deferred.promise;
        });

        try {
            const firstRun = harness.commands["gh-issues"].handler("foo/bar 1", harness.ctx);
            await flushMicrotasks();
            expect(signals[0]?.aborted).toBe(false);

            const secondRun = harness.commands["gh-issues"].handler("bar/baz 1", harness.ctx);
            await flushMicrotasks();
            expect(signals[0]?.aborted).toBe(true);
            await firstRun;

            deferreds[0]?.resolve(emptyIssuesResult());
            deferreds[1]?.resolve(emptyIssuesResult());
            await flushMicrotasks();

            expect(timers.scheduled).toHaveLength(1);
            expect(timers.scheduled[0]?.delay).toBe(60_000);
            await harness.commands["gh-issues-stop"].handler("", harness.ctx);
            await secondRun;
            expect(harness.workingMessages.at(-1)).toBeUndefined();
        } finally {
            timers.restore();
        }
    });
});
