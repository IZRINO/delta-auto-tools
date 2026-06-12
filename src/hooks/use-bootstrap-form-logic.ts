/**
 * useBootstrapForm 内部纯逻辑函数，可脱离 React 在 node 环境下测试。
 */

/** 计算表单是否有未保存的变更（往返比较） */
export function computeIsDirty<TForm>(
  form: TForm | null,
  bootstrapSettings: Record<string, unknown> | null,
  settingsToForm: (settings: Record<string, unknown>) => TForm,
  parseSettingsForm: (form: TForm) => Record<string, unknown>,
): boolean {
  if (!bootstrapSettings || !form) return false;
  try {
    const canonicalCurrent = settingsToForm(parseSettingsForm(form));
    const canonicalBootstrap = settingsToForm(bootstrapSettings);
    return JSON.stringify(canonicalCurrent) !== JSON.stringify(canonicalBootstrap);
  } catch {
    return true;
  }
}

/** 检查异步保存响应是否已过期 */
export function isStaleSave(
  pendingVersion: number | undefined,
  autosaveVersionRef: { current: number },
): boolean {
  return typeof pendingVersion === "number" && pendingVersion !== autosaveVersionRef.current;
}

/** 判断是否应完全同步 form */
export function shouldSyncFormFully(
  syncMode: string | undefined,
  syncForm: boolean | undefined,
  formIsNull: boolean,
): boolean {
  return syncMode === "full" || syncForm === true || formIsNull;
}
