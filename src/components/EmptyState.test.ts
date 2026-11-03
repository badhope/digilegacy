import { describe, expect, it } from 'vitest';
import { mount } from '@vue/test-utils';

import EmptyState from './EmptyState.vue';

// 空态插画的 SVG 分支：不同 kind 渲染不同图形，未知 kind 走兜底环。
// 图形本身不展示文字，这里断言「不抛错 + 兜底分支可达」。
describe('EmptyState', () => {
  it('kind=assets 渲染清单图形', () => {
    const w = mount(EmptyState, { props: { kind: 'assets' } });
    expect(w.find('svg').exists()).toBe(true);
    expect(w.find('svg').attributes('aria-hidden')).toBe('true');
  });

  it('kind=search 渲染搜索图形', () => {
    const w = mount(EmptyState, { props: { kind: 'search' } });
    expect(w.find('svg').exists()).toBe(true);
  });

  it('kind=report 渲染报告图形', () => {
    const w = mount(EmptyState, { props: { kind: 'report' } });
    expect(w.find('svg').exists()).toBe(true);
  });

  it('未知 kind 走兜底品牌环分支', () => {
    const w = mount(EmptyState, { props: { kind: 'mystery' as never } });
    expect(w.find('svg').exists()).toBe(true);
  });
});
