import { describe, expect, it } from 'vitest';
import { mount } from '@vue/test-utils';

import AssetCompositionChart from './AssetCompositionChart.vue';

// jsdom 没有 canvas 2D，组件会只渲染图例——这正是我们要断言的降级路径；
// 真实浏览器里由 E2E 视觉基线覆盖 canvas 本身。
describe('AssetCompositionChart', () => {
  it('按三类数量渲染图例与百分比', () => {
    const w = mount(AssetCompositionChart, {
      props: { property: 2, personality: 1, mixed: 1 },
    });
    const items = w.findAll('.asset-chart-legend li');
    expect(items).toHaveLength(3);
    expect(items[0].text()).toContain('财产型');
    expect(items[0].text()).toContain('2 项');
    expect(items[0].text()).toContain('50%');
    expect(items[1].text()).toContain('25%');
  });

  it('全为 0 时显示空态文案，不渲染图例', () => {
    const w = mount(AssetCompositionChart, {
      props: { property: 0, personality: 0, mixed: 0 },
    });
    expect(w.findAll('.asset-chart-legend li')).toHaveLength(0);
    expect(w.text()).toContain('还没有资产数据');
  });

  it('无障碍标签包含三类数量与总数', () => {
    const w = mount(AssetCompositionChart, {
      props: { property: 3, personality: 2, mixed: 1 },
    });
    const label = w.find('[role="img"]').attributes('aria-label') || '';
    expect(label).toContain('财产型 3 项');
    expect(label).toContain('人格型 2 项');
    expect(label).toContain('混合型 1 项');
    expect(label).toContain('共 6 项');
  });

  it('数量变化后百分比重新计算', async () => {
    const w = mount(AssetCompositionChart, {
      props: { property: 1, personality: 1, mixed: 0 },
    });
    expect(w.findAll('.asset-chart-legend li')[0].text()).toContain('50%');
    await w.setProps({ property: 3, personality: 1, mixed: 0 });
    expect(w.findAll('.asset-chart-legend li')[0].text()).toContain('75%');
  });
});
