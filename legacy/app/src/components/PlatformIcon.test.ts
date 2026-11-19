import { describe, expect, it } from 'vitest';
import { mount } from '@vue/test-utils';
import PlatformIcon from './PlatformIcon.vue';

describe('PlatformIcon', () => {
  it('已知平台渲染品牌图标（支付宝）', () => {
    const w = mount(PlatformIcon, { props: { name: '支付宝' } });
    expect(w.find('svg').exists()).toBe(true);
    expect(w.attributes('role')).toBe('img');
    expect(w.attributes('aria-label')).toBe('支付宝');
  });

  it('已知分类渲染语义图标（游戏）', () => {
    const w = mount(PlatformIcon, { props: { name: '游戏' } });
    expect(w.find('svg').exists()).toBe(true);
  });

  it('未知名称回退到通用盾形图标而不是空白', () => {
    const w = mount(PlatformIcon, { props: { name: '神秘平台XYZ' } });
    expect(w.find('svg').exists()).toBe(true);
  });

  it('支付宝/微信支付/B站等主流平台命中专用图标，不退化为通用盾形', () => {
    // 回归：旧映射表只覆盖 10 个名字，且与平台库真实 name 不一致
    // （「微信支付」「B站(bilibili)」都不在表里），资产列表里大半图标是盾牌。
    for (const name of ['支付宝', '微信支付', '微信', '微博', 'QQ', 'Steam', 'B站(bilibili)', '知乎', '网易云音乐']) {
      const w = mount(PlatformIcon, { props: { name } });
      expect(w.find('svg').exists(), `${name} 必须有图标`).toBe(true);
    }
  });

  it('支持自定义尺寸', () => {
    const w = mount(PlatformIcon, { props: { name: '微信', size: 32 } });
    expect(w.attributes('style')).toContain('32px');
  });
});
