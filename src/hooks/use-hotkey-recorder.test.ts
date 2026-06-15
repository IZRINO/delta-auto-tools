import {describe, expect, it, vi} from "vitest";

/**
 * 提取 useHotkeyRecorder 核心调度逻辑为纯函数。
 * hook 依赖 React state/ref，此处验证核心算法正确性。
 */

type MockKeyboardEvent = {
    key: string;
    ctrlKey?: boolean;
    altKey?: boolean;
    shiftKey?: boolean;
    metaKey?: boolean;
    preventDefault: () => void;
    stopPropagation: () => void;
};

function createMockEvent(overrides: Partial<MockKeyboardEvent> & { key: string }): MockKeyboardEvent {
    return {
        ctrlKey: false,
        altKey: false,
        shiftKey: false,
        metaKey: false,
        preventDefault: vi.fn(),
        stopPropagation: vi.fn(),
        ...overrides,
    };
}

/**
 * 模拟录制状态机：
 * beginRecording → handleKeyDown → handleBlur
 */
function createRecorderState(formatKey: (event: MockKeyboardEvent) => string | null, validate?: (key: string, event: MockKeyboardEvent) => boolean) {
    let isRecording = false;
    let draft = "";
    const commits: Array<{ key: string }> = [];
    const cancels: Array<{ draft: string }> = [];
    const messages: string[] = [];

    function beginRecording(currentValue: string) {
        draft = currentValue;
        isRecording = true;
        messages.push("recording-active");
    }

    function handleKeyDown(event: MockKeyboardEvent) {
        if (!isRecording) return;
        if (event.key === "Tab") return;

        event.preventDefault();
        event.stopPropagation();

        const nextKey = formatKey(event);
        if (!nextKey) {
            messages.push("key-rejected");
            return;
        }

        if (validate && !validate(nextKey, event)) {
            messages.push("validate-failed");
            return;
        }

        commits.push({key: nextKey});
        isRecording = false;
        messages.push(`key-recorded:${nextKey}`);
    }

    function handleBlur() {
        if (!isRecording) return;
        cancels.push({draft});
        isRecording = false;
        messages.push("recording-cancelled");
    }

    return {isRecording: () => isRecording, beginRecording, handleKeyDown, handleBlur, commits, cancels, messages};
}

describe("useHotkeyRecorder core logic", () => {
    it("beginRecording sets isRecording=true and saves draft", () => {
        const recorder = createRecorderState(() => "A");
        recorder.beginRecording("Ctrl+K");
        expect(recorder.isRecording()).toBe(true);
        expect(recorder.messages).toContain("recording-active");
    });

    it("valid keydown commits key and stops recording", () => {
        const recorder = createRecorderState(() => "Ctrl+A");
        recorder.beginRecording("old");
        recorder.handleKeyDown(createMockEvent({key: "a", ctrlKey: true}));
        expect(recorder.isRecording()).toBe(false);
        expect(recorder.commits).toEqual([{key: "Ctrl+A"}]);
        expect(recorder.messages).toContain("key-recorded:Ctrl+A");
    });

    it("Tab keydown is ignored", () => {
        const recorder = createRecorderState(() => "Tab");
        recorder.beginRecording("old");
        recorder.handleKeyDown(createMockEvent({key: "Tab"}));
        expect(recorder.isRecording()).toBe(true);
        expect(recorder.commits).toHaveLength(0);
    });

    it("null formatKey triggers rejection message", () => {
        const recorder = createRecorderState(() => null);
        recorder.beginRecording("old");
        recorder.handleKeyDown(createMockEvent({key: "Control"}));
        expect(recorder.isRecording()).toBe(true);
        expect(recorder.messages).toContain("key-rejected");
    });

    it("validate returning false rejects key without commit", () => {
        const recorder = createRecorderState(() => "Ctrl+A", (_key, _event) => false);
        recorder.beginRecording("old");
        recorder.handleKeyDown(createMockEvent({key: "a", ctrlKey: true}));
        expect(recorder.isRecording()).toBe(true);
        expect(recorder.commits).toHaveLength(0);
        expect(recorder.messages).toContain("validate-failed");
    });

    it("blur cancels recording and calls onCancel with draft", () => {
        const recorder = createRecorderState(() => "A");
        recorder.beginRecording("Ctrl+K");
        recorder.handleBlur();
        expect(recorder.isRecording()).toBe(false);
        expect(recorder.cancels).toEqual([{draft: "Ctrl+K"}]);
        expect(recorder.messages).toContain("recording-cancelled");
    });

    it("blur when not recording does nothing", () => {
        const recorder = createRecorderState(() => "A");
        recorder.handleBlur();
        expect(recorder.cancels).toHaveLength(0);
    });

    it("keydown when not recording does nothing", () => {
        const recorder = createRecorderState(() => "A");
        recorder.handleKeyDown(createMockEvent({key: "a"}));
        expect(recorder.commits).toHaveLength(0);
        expect(recorder.isRecording()).toBe(false);
    });

    it("preventDefault and stopPropagation called on keydown", () => {
        const recorder = createRecorderState(() => "A");
        recorder.beginRecording("old");
        const event = createMockEvent({key: "a"});
        recorder.handleKeyDown(event);
        expect(event.preventDefault).toHaveBeenCalled();
        expect(event.stopPropagation).toHaveBeenCalled();
    });

    it("rapidfire targetKey validation: reject key containing +", () => {
        const recorder = createRecorderState(
            () => "Ctrl+A",
            (key) => !key.includes("+"),
        );
        recorder.beginRecording("K");
        recorder.handleKeyDown(createMockEvent({key: "a", ctrlKey: true}));
        expect(recorder.isRecording()).toBe(true);
        expect(recorder.commits).toHaveLength(0);
        expect(recorder.messages).toContain("validate-failed");
    });
});
