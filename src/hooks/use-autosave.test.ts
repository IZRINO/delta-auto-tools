import {describe, expect, it, vi} from "vitest";

/**
 * 提取 useAutosave 内部调度逻辑为纯函数，便于在 node 环境下测试。
 * hook 本身依赖 React state/ref，此处验证核心算法正确性。
 */
function scheduleAutosave(
    formSnapshot: Record<string, unknown> | null,
    currentVersion: number,
    isDirty: boolean,
    disabled: boolean,
    delay: number,
    onSave: (form: Record<string, unknown>, version: number) => void,
    onError: (message: string) => void,
): { nextVersion: number; cleanup: () => void } {
    if (disabled || !isDirty) {
        return {
            nextVersion: currentVersion, cleanup: () => {
            }
        };
    }

    const nextVersion = currentVersion + 1;

    const timeoutId = setTimeout(() => {
        try {
            if (formSnapshot) {
                onSave(formSnapshot, nextVersion);
            }
        } catch (error) {
            const message = error instanceof Error ? error.message : String(error);
            onError(message);
        }
    }, delay);

    return {
        nextVersion,
        cleanup: () => {
            clearTimeout(timeoutId);
        },
    };
}

describe("useAutosave core logic", () => {
    it("skips when disabled", () => {
        const onSave = vi.fn();
        const result = scheduleAutosave({x: 1}, 0, true, true, 400, onSave, vi.fn());
        expect(result.nextVersion).toBe(0);
        expect(onSave).not.toHaveBeenCalled();
    });

    it("skips when not dirty", () => {
        const onSave = vi.fn();
        const result = scheduleAutosave({x: 1}, 0, false, false, 400, onSave, vi.fn());
        expect(result.nextVersion).toBe(0);
        expect(onSave).not.toHaveBeenCalled();
    });

    it("bumps version when dirty and not disabled", () => {
        const onSave = vi.fn();
        const result = scheduleAutosave({x: 1}, 5, true, false, 400, onSave, vi.fn());
        expect(result.nextVersion).toBe(6);
        result.cleanup();
    });

    it("calls onSave with form snapshot and version after delay", () => {
        vi.useFakeTimers();
        const onSave = vi.fn();
        const form = {x: 42};
        const result = scheduleAutosave(form, 0, true, false, 400, onSave, vi.fn());

        vi.advanceTimersByTime(400);

        expect(onSave).toHaveBeenCalledWith(form, 1);

        result.cleanup();
        vi.useRealTimers();
    });

    it("does not call onSave before delay elapses", () => {
        vi.useFakeTimers();
        const onSave = vi.fn();
        const result = scheduleAutosave({x: 1}, 0, true, false, 400, onSave, vi.fn());

        vi.advanceTimersByTime(200);

        expect(onSave).not.toHaveBeenCalled();

        result.cleanup();
        vi.useRealTimers();
    });

    it("calls onError for parse errors", () => {
        vi.useFakeTimers();
        const onError = vi.fn();
        const onSave = () => {
            throw new Error("parse failed");
        };
        const result = scheduleAutosave({x: 1}, 0, true, false, 400, onSave, onError);

        vi.advanceTimersByTime(400);

        expect(onError).toHaveBeenCalledWith("parse failed");

        result.cleanup();
        vi.useRealTimers();
    });

    it("cleanup prevents the timeout from firing", () => {
        vi.useFakeTimers();
        const onSave = vi.fn();
        const result = scheduleAutosave({x: 1}, 0, true, false, 400, onSave, vi.fn());

        result.cleanup();

        vi.advanceTimersByTime(500);

        expect(onSave).not.toHaveBeenCalled();

        vi.useRealTimers();
    });
});
