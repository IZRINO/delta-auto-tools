import { startTransition, useCallback, useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useSyncedRef } from "@/hooks/use-synced-ref";
import { computeIsDirty, isStaleSave } from "@/hooks/use-bootstrap-form-logic";
import { getErrorMessage } from "@/lib/error-utils";

/** 定义工具页与 Rust 后端交互的规范 */
export interface BootstrapFormSpec<TBootstrap extends { settings: Record<string, unknown> }, TSettings, TForm> {
  /** Tauri 命令名，获取 bootstrap */
  getBootstrapCommand: string;
  /** Tauri 命令名，保存设置 */
  saveSettingsCommand: string;
  /** 将 bootstrap.settings 转换为表单可编辑态 */
  settingsToForm: (settings: TBootstrap["settings"]) => TForm;
  /** 将表单态解析回设置态（含验证） */
  parseSettingsForm: (form: TForm) => TSettings;
}

/** useBootstrapForm 配置 */
export interface UseBootstrapFormOptions<TBootstrap extends { settings: Record<string, unknown> }, TSettings, TForm> {
  spec: BootstrapFormSpec<TBootstrap, TSettings, TForm>;
  /** 是否在 Tauri 桌面环境中运行 */
  isNativeShell: boolean;
  /** 是否跳过初始加载（overlay 模式） */
  skipInitialLoad?: boolean;
  /** 加载中状态消息 */
  loadStatusMessage?: string;
  /** 加载完成就绪消息 */
  readyStatusMessage?: string;
  /** 浏览器预览模式状态消息 */
  previewStatusMessage?: string;
  /** 保存成功消息，可以是静态字符串或根据 bootstrap 动态生成 */
  saveSuccessMessage?: string | ((next: TBootstrap) => string);
  /** 保存进行中消息（Morse 需要） */
  saveInProgressMessage?: string;
  /** 是否使用 startTransition 包裹状态更新（Morse 需要） */
  useStartTransition?: boolean;
  /** updateForm 前置钩子（Rapidfire 的 clearStaleConfigError） */
  beforeUpdateForm?: () => void;
}

/** syncBootstrap 选项 */
export interface SyncBootstrapOptions {
  /** Morse: "full" | "regions" | "none"；Timer/Rapidfire: 同 syncForm */
  syncMode?: string;
  /** 是否同步 form（Timer/Rapidfire 使用） */
  syncForm?: boolean;
}

export interface UseBootstrapFormReturn<TBootstrap extends { settings: Record<string, unknown> }, TSettings, TForm> {
  bootstrap: TBootstrap | null;
  setBootstrap: React.Dispatch<React.SetStateAction<TBootstrap | null>>;
  form: TForm | null;
  setForm: React.Dispatch<React.SetStateAction<TForm | null>>;
  isDirty: boolean;
  updateForm: <K extends keyof TForm>(key: K, value: TForm[K]) => void;
  saveSettings: (settingsValue: TSettings, pendingVersion?: number) => Promise<void>;
  syncBootstrap: (opts?: SyncBootstrapOptions) => Promise<TBootstrap>;
  loading: boolean;
  saving: boolean;
  pageError: string | null;
  setPageError: React.Dispatch<React.SetStateAction<string | null>>;
  statusMessage: string;
  setStatusMessage: React.Dispatch<React.SetStateAction<string>>;
  /** autosave 版本号引用，传给 useAutosave 使用 */
  autosaveVersionRef: React.MutableRefObject<number>;
}

/**
 * 工具页共享的 Bootstrap/Form 双状态生命周期 hook。
 *
 * 管理：bootstrap + form 双状态、syncBootstrap、updateForm、isDirty（往返比较）、
 * saveSettings（含 stale guard）、初始加载 useEffect、loading/saving/pageError/statusMessage。
 *
 * 与 useAutosave 正交组合：useAutosave 提供 autosaveVersionRef，
 * useBootstrapForm 的 saveSettings 使用该 ref 做 stale guard。
 */
export function useBootstrapForm<TBootstrap extends { settings: Record<string, unknown> }, TSettings, TForm>(
  options: UseBootstrapFormOptions<TBootstrap, TSettings, TForm>,
): UseBootstrapFormReturn<TBootstrap, TSettings, TForm> {
  const {
    spec,
    isNativeShell,
    skipInitialLoad = false,
    loadStatusMessage = "正在加载...",
    readyStatusMessage = "就绪。",
    previewStatusMessage = "浏览器预览模式：当前仅验证布局，原生命令请在桌面端运行。",
    saveSuccessMessage = "设置已保存。",
    saveInProgressMessage,
    useStartTransition: shouldUseStartTransition = false,
    beforeUpdateForm,
  } = options;

  const autosaveVersionRef = useRef(0);
  const [bootstrap, setBootstrap] = useState<TBootstrap | null>(null);
  const [form, setForm] = useState<TForm | null>(null);
  const formRef = useSyncedRef(form);
  const [loading, setLoading] = useState(isNativeShell && !skipInitialLoad);
  const [saving, setSaving] = useState(false);
  const [statusMessage, setStatusMessage] = useState(
    isNativeShell && !skipInitialLoad ? loadStatusMessage : previewStatusMessage,
  );
  const [pageError, setPageError] = useState<string | null>(null);

  // 统一状态更新：可选 startTransition
  const updateState = useCallback(
    (updater: () => void) => {
      if (shouldUseStartTransition) {
        startTransition(updater);
      } else {
        updater();
      }
    },
    [shouldUseStartTransition],
  );

  // isDirty：往返比较（与 Timer/Rapidfire 现有方式一致，更健壮）
  const isDirty = useMemo(
    () =>
      computeIsDirty(
        form,
        bootstrap?.settings ?? null,
        spec.settingsToForm as unknown as (s: Record<string, unknown>) => TForm,
        spec.parseSettingsForm as unknown as (f: TForm) => Record<string, unknown>,
      ),
    [bootstrap, form, spec],
  );

  // updateForm
  const updateForm = useCallback(
    <K extends keyof TForm>(key: K, value: TForm[K]) => {
      beforeUpdateForm?.();
      setForm((current) => (current ? { ...current, [key]: value } : current));
    },
    [beforeUpdateForm],
  );

  // saveSettings（含 stale guard）
  const saveSettings = useCallback(
    async (settingsValue: TSettings, pendingVersion?: number) => {
      try {
        setSaving(true);
        if (saveInProgressMessage) {
          setStatusMessage(saveInProgressMessage);
        }
        const next = await invoke<TBootstrap>(spec.saveSettingsCommand, { settingsValue });

        if (isStaleSave(pendingVersion, autosaveVersionRef)) {
          return;
        }

        const msg = typeof saveSuccessMessage === "function"
          ? saveSuccessMessage(next)
          : saveSuccessMessage;

        updateState(() => {
          setBootstrap(next);
          setForm(spec.settingsToForm(next.settings));
          setPageError(null);
          setStatusMessage(msg);
        });
      } catch (error) {
        if (isStaleSave(pendingVersion, autosaveVersionRef)) {
          return;
        }
        const message = getErrorMessage(error);
        setPageError(message);
        setStatusMessage(message);
      } finally {
        setSaving(false);
      }
    },
    [autosaveVersionRef, saveInProgressMessage, saveSuccessMessage, spec, updateState],
  );

  // syncBootstrap
  const syncBootstrap = useCallback(
    async (opts: SyncBootstrapOptions = {}): Promise<TBootstrap> => {
      const { syncMode, syncForm } = opts;
      const shouldSyncForm = syncMode === "full" || syncForm === true || formRef.current === null;

      const next = await invoke<TBootstrap>(spec.getBootstrapCommand);

      updateState(() => {
        setBootstrap(next);
        setPageError(null);

        if (shouldSyncForm) {
          setForm(spec.settingsToForm(next.settings));
        } else if (syncMode === "regions") {
          setForm((current) =>
            current
              ? { ...current, regions: next.settings.regions }
              : spec.settingsToForm(next.settings),
          );
        }
        // syncMode "none" 或 syncForm false：仅更新 bootstrap，不动 form
      });

      return next;
    },
    [formRef, spec, updateState],
  );

  // 初始加载 useEffect
  useEffect(() => {
    if (skipInitialLoad) {
      return;
    }

    if (!isNativeShell) {
      setLoading(false);
      setPageError(null);
      setStatusMessage(previewStatusMessage);
      return;
    }

    let disposed = false;

    const load = async () => {
      try {
        setLoading(true);
        await syncBootstrap({ syncForm: true });
        if (!disposed) {
          setStatusMessage(readyStatusMessage);
        }
      } catch (error) {
        if (!disposed) {
          const message = getErrorMessage(error);
          setPageError(message);
          setStatusMessage(message);
        }
      } finally {
        if (!disposed) {
          setLoading(false);
        }
      }
    };

    void load();

    return () => {
      disposed = true;
    };
  }, [isNativeShell, skipInitialLoad, syncBootstrap, previewStatusMessage, readyStatusMessage]);

  return {
    bootstrap,
    setBootstrap,
    form,
    setForm,
    isDirty,
    updateForm,
    saveSettings,
    syncBootstrap,
    loading,
    saving,
    pageError,
    setPageError,
    statusMessage,
    setStatusMessage,
    autosaveVersionRef,
  };
}
