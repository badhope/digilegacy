import { describe, expect, it, vi, beforeEach, afterEach } from 'vitest';
import { effectScope, nextTick, ref } from 'vue';
import {
  debounce,
  useDebouncedInput,
  useDebouncedRef,
  useThrottledRef,
} from './debounce';

beforeEach(() => {
  vi.useFakeTimers();
});

afterEach(() => {
  vi.useRealTimers();
});

describe('useDebouncedRef', () => {
  it('输入期间不更新，停止输入后才写一次', async () => {
    const scope = effectScope();
    const source = ref('');
    const out = scope.run(() => useDebouncedRef(source, 250))!;

    source.value = 'a';
    await nextTick();
    source.value = 'ab';
    await nextTick();
    source.value = 'abc';
    await nextTick();

    // 仍在连打，输出应停留在初值——否则每敲一个字都重算，搜索会卡顿
    expect(out.value).toBe('');

    vi.advanceTimersByTime(250);
    await nextTick();
    expect(out.value).toBe('abc');

    scope.stop();
  });

  it('作用域销毁后清掉待触发的定时器（不留悬挂回调）', async () => {
    const scope = effectScope();
    const source = ref('x');
    const out = scope.run(() => useDebouncedRef(source, 250))!;

    source.value = 'y';
    await nextTick();
    scope.stop();

    vi.advanceTimersByTime(1000);
    await nextTick();
    expect(out.value).toBe('x');
  });
});

describe('useThrottledRef', () => {
  it('首次立即生效，高频输入被合并，最后一次不丢', async () => {
    const scope = effectScope();
    const source = ref(0);
    const out = scope.run(() => useThrottledRef(source, 120))!;

    source.value = 1;
    await nextTick();
    // 第一次事件立即反馈：拖拽/滚动时用户要马上看到响应
    expect(out.value).toBe(1);

    source.value = 2;
    source.value = 3;
    source.value = 4;
    await nextTick();
    // 节流窗口内不刷新
    expect(out.value).toBe(1);

    vi.advanceTimersByTime(120);
    await nextTick();
    // 末次值必须落地，否则「松手后停在旧值」是典型节流 bug
    expect(out.value).toBe(4);

    scope.stop();
  });
});

describe('useDebouncedInput', () => {
  it('输入框回显即时、重计算滞后', async () => {
    const scope = effectScope();
    const { input, debounced } = scope.run(() => useDebouncedInput('', 200))!;

    input.value = 'digi';
    await nextTick();
    expect(input.value).toBe('digi');
    expect(debounced.value).toBe('');

    vi.advanceTimersByTime(200);
    await nextTick();
    expect(debounced.value).toBe('digi');

    scope.stop();
  });
});

describe('debounce', () => {
  it('只执行最后一次，且 cancel 后不再执行', () => {
    const fn = vi.fn();
    const wrapped = debounce(fn, 100);

    wrapped('a');
    wrapped('b');
    wrapped('c');
    expect(fn).not.toHaveBeenCalled();

    vi.advanceTimersByTime(100);
    expect(fn).toHaveBeenCalledOnce();
    expect(fn).toHaveBeenCalledWith('c');

    wrapped('d');
    wrapped.cancel();
    vi.advanceTimersByTime(500);
    expect(fn).toHaveBeenCalledOnce();
  });
});

describe('useThrottledRef 边界', () => {
  it('窗口内已排队时再触发不会重排定时器（合并为一次 flush）', async () => {
    const scope = effectScope();
    const source = ref(0);
    const out = scope.run(() => useThrottledRef(source, 100))!;

    source.value = 1;
    await nextTick();
    expect(out.value).toBe(1); // 首次立即生效

    // 进入节流窗口：两次触发只应排一个定时器，末值为 3
    source.value = 2;
    source.value = 3;
    await nextTick();
    expect(out.value).toBe(1);

    vi.advanceTimersByTime(100);
    await nextTick();
    expect(out.value).toBe(3);

    scope.stop();
  });

  it('窗口过去后又立即生效（不会永久卡在节流态）', async () => {
    const scope = effectScope();
    const source = ref('a');
    const out = scope.run(() => useThrottledRef(source, 100))!;

    source.value = 'b';
    await nextTick();
    expect(out.value).toBe('b');

    vi.advanceTimersByTime(150);
    await nextTick();

    source.value = 'c';
    await nextTick();
    // 距上次 flush 已超过 interval，应立刻生效而不是再等一个窗口
    expect(out.value).toBe('c');

    scope.stop();
  });

  it('销毁时清掉待触发的定时器', async () => {
    const scope = effectScope();
    const source = ref(0);
    const out = scope.run(() => useThrottledRef(source, 200))!;

    source.value = 1;
    await nextTick();
    source.value = 2; // 排队中
    await nextTick();
    scope.stop();

    vi.advanceTimersByTime(1000);
    await nextTick();
    // 已销毁：排队值不得再写入
    expect(out.value).toBe(1);
  });
});

describe('useThrottledRef 定时器与立即触发竞争', () => {
  it('已排队时如果窗口已经过去，取消排队并立即 flush 一次', async () => {
    const scope = effectScope();
    const source = ref(0);
    const out = scope.run(() => useThrottledRef(source, 100))!;

    // 第一次立即生效，last = now
    source.value = 1;
    await nextTick();
    expect(out.value).toBe(1);

    // 窗口内触发 → 排队（timer 已存在）
    source.value = 2;
    await nextTick();
    expect(out.value).toBe(1);

    // 把时钟推过 interval，让下一次触发走 wait <= 0 且 timer 仍在
    vi.setSystemTime(Date.now() + 200);
    source.value = 3;
    await nextTick();

    // 应立即生效（3），且原来排队的定时器被取消，不会稍后用 2 覆盖回去
    expect(out.value).toBe(3);
    vi.advanceTimersByTime(200);
    await nextTick();
    expect(out.value).toBe(3);

    scope.stop();
  });
});
