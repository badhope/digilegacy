import { describe, expect, it } from 'vitest';
import { mount } from '@vue/test-utils';
import CollapseGroup from './CollapseGroup.vue';

// CollapseGroup 是报告/助手页的可折叠容器。它的 v-model 契约一旦错位，
// 父组件的展开状态会与已展开内容不同步（点一次没反应）。
describe('CollapseGroup', () => {
  it('默认收起：不渲染插槽内容', () => {
    const w = mount(CollapseGroup, {
      props: { title: '继承流程' },
      slots: { default: '<p class="inner">内容</p>' },
    });

    expect(w.find('.inner').exists()).toBe(false);
    expect(w.find('.collapse-head').attributes('aria-expanded')).toBe('false');
  });

  it('defaultOpen 初始展开', () => {
    const w = mount(CollapseGroup, {
      props: { title: 't', defaultOpen: true },
      slots: { default: '<p class="inner">内容</p>' },
    });

    expect(w.find('.inner').exists()).toBe(true);
    expect(w.find('.collapse-head').attributes('aria-expanded')).toBe('true');
  });

  it('点击切换内部状态并发出 update:modelValue', async () => {
    const w = mount(CollapseGroup, {
      props: { title: 't' },
      slots: { default: '<p class="inner">内容</p>' },
    });

    await w.find('.collapse-head').trigger('click');
    expect(w.emitted('update:modelValue')?.[0]).toEqual([true]);
    expect(w.find('.inner').exists()).toBe(true);

    await w.find('.collapse-head').trigger('click');
    expect(w.emitted('update:modelValue')?.[1]).toEqual([false]);
    expect(w.find('.inner').exists()).toBe(false);
  });

  it('受控模式下外部 modelValue 变化同步到视图', async () => {
    const w = mount(CollapseGroup, {
      props: { title: 't', modelValue: false },
      slots: { default: '<p class="inner">内容</p>' },
    });
    expect(w.find('.inner').exists()).toBe(false);

    await w.setProps({ modelValue: true });

    expect(w.find('.inner').exists()).toBe(true);
  });

  it('count 为 0 时仍渲染徽标（0 是有效计数，不能被当成空值）', () => {
    const w = mount(CollapseGroup, { props: { title: 't', count: 0 } });
    expect(w.find('.collapse-count').text()).toBe('0');
  });

  it('不传 count 时不渲染徽标', () => {
    const w = mount(CollapseGroup, { props: { title: 't' } });
    expect(w.find('.collapse-count').exists()).toBe(false);
  });
});
