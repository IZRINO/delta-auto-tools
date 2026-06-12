import { useCallback, useRef, useState } from "react";

/** 热键录制 hook 选项 */
export interface UseHotkeyRecorderOptions {
  /** 将键盘事件格式化为热键字符串；返回 null 表示不可识别 */
  formatKey: (event: React.KeyboardEvent<HTMLButtonElement>) => string | null;
  /** 可选验证：返回 false 则拒绝此次录入 */
  validate?: (key: string, event: React.KeyboardEvent<HTMLButtonElement>) => boolean;
  /** 成功录入回调 */
  onCommit: (key: string) => void;
  /** 取消录制回调（参数为 beginRecording 时保存的草稿值） */
  onCancel: (draftValue: string) => void;
  /** 状态消息回调 */
  onStatusMessage: (message: string) => void;
  /** 录制激活时的消息（默认："正在录制热键，按下组合键后会自动更新；失焦会取消录制。"） */
  recordingActiveMessage?: string;
  /** 格式化返回 null 时的消息（默认："请按下一个可识别的主键，支持字母、数字、功能键与常用导航键。"） */
  keyRejectedMessage?: string;
  /** 录制取消时的消息（默认："已取消热键录制。"） */
  recordingCancelledMessage?: string;
  /** 成功录入消息，可以是静态字符串或根据 key 动态生成（默认：(key) => `新的热键已录制：${key}`） */
  keyRecordedMessage?: string | ((key: string) => string);
}

/** 热键录制 hook 返回值 */
export interface UseHotkeyRecorderReturn {
  /** 是否正在录制 */
  isRecording: boolean;
  /** 开始录制（传入当前值作为草稿） */
  beginRecording: (currentValue: string) => void;
  /** 键盘事件处理器（绑定到按钮 onKeyDown） */
  handleKeyDown: (event: React.KeyboardEvent<HTMLButtonElement>) => void;
  /** 失焦事件处理器（绑定到按钮 onBlur） */
  handleBlur: () => void;
}

/**
 * 提取三个页面共享的热键录制循环：
 * beginRecording → handleKeyDown(format→commit/cancel) → handleBlur(cancel→draft)。
 *
 * 调用方保留 recordingTarget / isRecording 等目标标识状态，
 * hook 仅管理 isRecording + draft + keydown/blur 事件处理。
 */
export function useHotkeyRecorder(options: UseHotkeyRecorderOptions): UseHotkeyRecorderReturn {
  const {
    formatKey,
    validate,
    onCommit,
    onCancel,
    onStatusMessage,
    recordingActiveMessage = "正在录制热键，按下组合键后会自动更新；失焦会取消录制。",
    keyRejectedMessage = "请按下一个可识别的主键，支持字母、数字、功能键与常用导航键。",
    recordingCancelledMessage = "已取消热键录制。",
    keyRecordedMessage = (key: string) => `新的热键已录制：${key}`,
  } = options;

  const [isRecording, setIsRecording] = useState(false);
  const draftRef = useRef("");

  // 保持回调 ref 最新，避免闭包过期
  const formatKeyRef = useRef(formatKey);
  const validateRef = useRef(validate);
  const onCommitRef = useRef(onCommit);
  const onCancelRef = useRef(onCancel);
  const onStatusMessageRef = useRef(onStatusMessage);
  const keyRecordedMessageRef = useRef(keyRecordedMessage);

  // 同步 ref — 这些回调可能来自页面级 useCallback，依赖变化时需要更新
  formatKeyRef.current = formatKey;
  validateRef.current = validate;
  onCommitRef.current = onCommit;
  onCancelRef.current = onCancel;
  onStatusMessageRef.current = onStatusMessage;
  keyRecordedMessageRef.current = keyRecordedMessage;

  const beginRecording = useCallback((currentValue: string) => {
    draftRef.current = currentValue;
    setIsRecording(true);
    onStatusMessageRef.current(recordingActiveMessage);
  }, [recordingActiveMessage]);

  const handleKeyDown = useCallback((event: React.KeyboardEvent<HTMLButtonElement>) => {
    if (!isRecording) {
      return;
    }

    if (event.key === "Tab") {
      return;
    }

    event.preventDefault();
    event.stopPropagation();


    const nextKey = formatKeyRef.current(event);
    if (!nextKey) {
      onStatusMessageRef.current(keyRejectedMessage);
      return;
    }

    if (validateRef.current && !validateRef.current(nextKey, event)) {
      return;
    }

    onCommitRef.current(nextKey);
    setIsRecording(false);

    const msg = typeof keyRecordedMessageRef.current === "function"
      ? keyRecordedMessageRef.current(nextKey)
      : keyRecordedMessageRef.current;
    onStatusMessageRef.current(msg);
  }, [isRecording, keyRejectedMessage]);

  const handleBlur = useCallback(() => {
    if (!isRecording) {
      return;
    }

    onCancelRef.current(draftRef.current);
    setIsRecording(false);
    onStatusMessageRef.current(recordingCancelledMessage);
  }, [isRecording, recordingCancelledMessage]);

  return { isRecording, beginRecording, handleKeyDown, handleBlur };
}
