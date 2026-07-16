import {useEffect, useRef} from "react";

/** 自动保存 hook 选项 */
export interface UseAutosaveOptions<TForm> {
    /** 当前表单状态（hook 内部快照，避免闭包过期） */
    form: TForm | null;
    /** 表单是否有未保存的变更 */
    isDirty: boolean;
    /** 合并守卫：true 时跳过自动保存（覆盖 overlayMode / !isNativeShell / loading / 录制中等） */
    disabled: boolean;
    /** 延迟到期后调用，接收 form 快照和版本号 */
    onSave: (formSnapshot: TForm, nextVersion: number) => Promise<void>;
    /** 非陈旧保存出错时调用 */
    onError?: (message: string) => void;
    /** 防抖延迟（毫秒），默认 400 */
    delay?: number;
    /** 外部版本号引用（由 useBootstrapForm 提供）；不传则内部创建 */
    autosaveVersionRef?: React.MutableRefObject<number>;
}

/** 自动保存 hook 返回值 */
export interface UseAutosaveReturn {
    /** 版本号引用，页面 saveSettings 可读取此 ref 做 stale guard */
    autosaveVersionRef: React.MutableRefObject<number>;
}

/**
 * 提取三个工具页（Morse / Timer / Rapidfire）共享的自动保存管线：
 * debounce + versionRef 防陈旧覆盖 + 卸载清理。
 *
 * 调用方在 saveSettings 中检查 autosaveVersionRef.current 判断
 * 异步保存响应是否已过期（pendingVersion !== autosaveVersionRef.current）。
 *
 * 当与 useBootstrapForm 组合使用时，useBootstrapForm 先调用（内部创建 autosaveVersionRef），
 * 然后将其传入 useAutosave 的 autosaveVersionRef 选项。
 */
export function useAutosave<TForm>(options: UseAutosaveOptions<TForm>): UseAutosaveReturn {
    const {form, isDirty, disabled, onSave, onError, delay = 400, autosaveVersionRef: externalVersionRef} = options;

    const internalVersionRef = useRef(0);
    const autosaveVersionRef = externalVersionRef ?? internalVersionRef;
    const saveTimeoutRef = useRef<number | null>(null);
    const onSaveRef = useRef(onSave);
    const onErrorRef = useRef(onError);

    useEffect(() => {
        return () => {
            if (saveTimeoutRef.current !== null) {
                window.clearTimeout(saveTimeoutRef.current);
            }
        };
    }, []);

    // 保持回调 ref 最新，避免 useEffect 闭包过期
    useEffect(() => {
        onSaveRef.current = onSave;
        onErrorRef.current = onError;
    }, [onSave, onError]);

    useEffect(() => {
        if (disabled || !isDirty) {
            return;
        }

        const nextVersion = autosaveVersionRef.current + 1;
        autosaveVersionRef.current = nextVersion;
        const formSnapshot = form;

        saveTimeoutRef.current = window.setTimeout(() => {
            try {
                if (formSnapshot) {
                    void onSaveRef.current(formSnapshot, nextVersion).catch((error: unknown) => {
                        if (nextVersion !== autosaveVersionRef.current) return;
                        const message = error instanceof Error ? error.message : String(error);
                        onErrorRef.current?.(message);
                    });
                }
            } catch (error) {
                if (nextVersion !== autosaveVersionRef.current) {
                    return;
                }

                const message = error instanceof Error ? error.message : String(error);
                onErrorRef.current?.(message);
            }
        }, delay);

        return () => {
            if (saveTimeoutRef.current !== null) {
                window.clearTimeout(saveTimeoutRef.current);
                saveTimeoutRef.current = null;
            }
        };
    }, [autosaveVersionRef, delay, disabled, form, isDirty, saveTimeoutRef]);

    return {autosaveVersionRef};
}
