// 输入节流原语：搜索框这类「每次按键都会重算」的交互必须有它。
//
// 为什么自己写而不是装 lodash：这里只需要 30 行、一个 Vue ref 的生命周期管理，
// 引入整包 lodash（或 lodash.debounce）换来的只是 import 一行。判断标准是
// 「依赖带来的维护面 vs. 真正省下的代码量」，此处后者几乎为零。
//
// 两个原语的区别，选错会让交互手感变差：
//   useDebouncedRef —— 用户**停止输入**之后才更新（搜索、校验）。连打 5 个字只算 1 次。
//   useThrottledRef —— 固定频率更新（拖拽、滚动、实时预览）。保证有反馈但不刷屏。

import { onScopeDispose, ref, watch, type Ref } from 'vue';

/** 用户停止输入 `delay` 毫秒后才把值写出去。 */
export function useDebouncedRef<T>(source: Ref<T>, delay = 250): Ref<T> {
  const output = ref(source.value) as Ref<T>;
  let timer: ReturnType<typeof setTimeout> | undefined;

  watch(
    source,
    (next) => {
      if (timer !== undefined) clearTimeout(timer);
      timer = setTimeout(() => {
        timer = undefined;
        output.value = next;
      }, delay);
    },
    { flush: 'sync' }
  );

  onScopeDispose(() => {
    if (timer !== undefined) clearTimeout(timer);
  });

  return output;
}

/** 每 `interval` 毫秒最多写一次；第一次立即写，末次必写（不丢最后一次输入）。 */
export function useThrottledRef<T>(source: Ref<T>, interval = 120): Ref<T> {
  const output = ref(source.value) as Ref<T>;
  let last = 0;
  let timer: ReturnType<typeof setTimeout> | undefined;
  let pending: T | undefined;
  let hasPending = false;

  const flush = () => {
    timer = undefined;
    if (hasPending) {
      hasPending = false;
      last = Date.now();
      output.value = pending as T;
    }
  };

  watch(
    source,
    (next) => {
      const now = Date.now();
      const wait = interval - (now - last);
      pending = next;
      hasPending = true;
      if (wait <= 0) {
        if (timer !== undefined) {
          clearTimeout(timer);
          timer = undefined;
        }
        flush();
      } else if (timer === undefined) {
        timer = setTimeout(flush, wait);
      }
    },
    { flush: 'sync' }
  );

  onScopeDispose(() => {
    if (timer !== undefined) clearTimeout(timer);
  });

  return output;
}

/** 文本框用的防抖值：写 input 立即回显，读 debounced 值做重计算。 */
export function useDebouncedInput(initial = '', delay = 250) {
  const input = ref(initial);
  const debounced = useDebouncedRef(input, delay);
  return { input, debounced };
}

/**
 * 给任意函数加防抖（不是 ref 场景，例如「窗口 resize 后重排」）。
 * 返回的包装函数带 `cancel()`，卸载时务必调用，否则定时器会拖住已销毁组件。
 */
export function debounce<A extends unknown[]>(
  fn: (...args: A) => void,
  delay = 250
): ((...args: A) => void) & { cancel: () => void } {
  let timer: ReturnType<typeof setTimeout> | undefined;
  const wrapped = (...args: A) => {
    if (timer !== undefined) clearTimeout(timer);
    timer = setTimeout(() => {
      timer = undefined;
      fn(...args);
    }, delay);
  };
  wrapped.cancel = () => {
    if (timer !== undefined) clearTimeout(timer);
    timer = undefined;
  };
  return wrapped;
}
