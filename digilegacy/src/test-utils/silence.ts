import { afterEach, beforeEach, vi } from 'vitest';

// 失败路径用例会故意触发 console.error（store/提醒模块的错误日志）。
// 这是被测行为的一部分，不是测试失败；但直接输出到 stderr 会被人和 CI 误读成「全红」。
//
// 做法：静音 console.error 但保留调用记录，需要验证「确实记了日志」的用例
// 可断言 spy 被调用过。

let spy: { mockRestore: () => void; mock: { calls: unknown[][] } } | null = null;

export function getConsoleErrorSpy() {
  return spy;
}

/** 在测试文件顶部调用一次即可（内部注册 beforeEach/afterEach）。 */
export function silenceExpectedErrors() {
  beforeEach(() => {
    spy = vi.spyOn(console, 'error').mockImplementation(() => {});
  });
  afterEach(() => {
    spy?.mockRestore();
    spy = null;
  });
}
