// 统一反馈层：Toast 提示 + 模态确认。
//
// 替代原生 alert()/confirm()：Electron/Tauri 里原生对话框会弹出系统级窗口，
// 样式与产品完全割裂（且不可定制、不可访问性控制），在 Windows 上尤其突兀。
//
// 用法：
//   const fb = useFeedback();
//   fb.toast('已保存');                      // 成功（默认）
//   fb.toast('保存失败', 'error');
//   if (await fb.confirm({ title: '确认删除？', danger: true })) { ... }

import { reactive, readonly } from 'vue';
import { t } from '../i18n';

export type ToastKind = 'success' | 'error' | 'info';

export interface ToastItem {
  id: number;
  message: string;
  kind: ToastKind;
}

export interface ConfirmOptions {
  title: string;
  message?: string;
  confirmText?: string;
  cancelText?: string;
  /** 危险操作用红色主按钮 */
  danger?: boolean;
}

interface ConfirmState extends ConfirmOptions {
  open: boolean;
  resolve: ((ok: boolean) => void) | null;
}

const toasts = reactive<ToastItem[]>([]);
const confirmState = reactive<ConfirmState>({
  open: false,
  title: '',
  message: '',
  confirmText: t('common.confirm'),
  cancelText: t('common.cancel'),
  danger: false,
  resolve: null,
});

let seq = 0;

export function toast(message: string, kind: ToastKind = 'success', duration = 2600) {
  const id = ++seq;
  toasts.push({ id, message, kind });
  window.setTimeout(() => {
    const i = toasts.findIndex(t => t.id === id);
    if (i >= 0) toasts.splice(i, 1);
  }, duration);
}

export function dismissToast(id: number) {
  const i = toasts.findIndex(t => t.id === id);
  if (i >= 0) toasts.splice(i, 1);
}

export function confirmDialog(opts: ConfirmOptions): Promise<boolean> {
  // 同一时刻只允许一个确认框：旧的按取消处理，避免 Promise 悬挂
  confirmState.resolve?.(false);
  return new Promise<boolean>((resolve) => {
    confirmState.open = true;
    confirmState.title = opts.title;
    confirmState.message = opts.message ?? '';
    confirmState.confirmText = opts.confirmText ?? t('common.confirm');
    confirmState.cancelText = opts.cancelText ?? t('common.cancel');
    confirmState.danger = opts.danger ?? false;
    confirmState.resolve = resolve;
  });
}

/** 读取当前挂起的确认框（测试用：断言破坏性操作的文案与危险标记）。 */
export function lastConfirm(): ConfirmState | null {
  return confirmState.open ? { ...confirmState } : null;
}

export function settleConfirm(ok: boolean) {
  confirmState.open = false;
  const r = confirmState.resolve;
  confirmState.resolve = null;
  r?.(ok);
}

/** 仅供测试：清空全部 toast 与确认框状态。 */
export function __resetFeedbackForTests() {
  toasts.splice(0, toasts.length);
  confirmState.open = false;
  confirmState.resolve = null;
}

export function useFeedback() {
  return {
    toasts: readonly(toasts),
    confirmState: readonly(confirmState),
    toast,
    confirm: confirmDialog,
  };
}
