import { afterEach, describe, expect, it } from 'vitest';
import { categoryLabel, disposalLabel, formatMoney } from './format';
import { setLocale } from '../i18n';

// 该模块是三视图共享的单一事实来源，测试同时充当文案漂移回归网
// （曾因各视图持本地副本导致 no_action 在「待定/待设定」间漂移）。

afterEach(() => setLocale('zh-CN'));

describe('formatMoney', () => {
  it('空值与 0 显示「0」', () => {
    expect(formatMoney(0)).toBe('0');
    expect(formatMoney(NaN)).toBe('0');
  });

  it('万以下千分位', () => {
    expect(formatMoney(9999)).toBe('9,999');
  });

  it('万以上缩写为 x.x万', () => {
    expect(formatMoney(10000)).toBe('1.0万');
    expect(formatMoney(123456)).toBe('12.3万');
  });

  it('英文界面按 k/M/B 缩写，不再出现「万」', () => {
    setLocale('en-US');
    expect(formatMoney(999)).toBe('999');
    expect(formatMoney(123456)).toBe('123.5k');
    expect(formatMoney(2_500_000)).toBe('2.5M');
    expect(formatMoney(3_000_000_000)).toBe('3.0B');
  });
});

describe('categoryLabel', () => {
  it('三类枚举全映射', () => {
    expect(categoryLabel('property')).toBe('财产型');
    expect(categoryLabel('personality')).toBe('人格型');
    expect(categoryLabel('mixed')).toBe('混合型');
  });

  it('未知值原样返回', () => {
    expect(categoryLabel('unknown_cat')).toBe('unknown_cat');
  });
});

describe('disposalLabel', () => {
  it('六种主动处置全映射', () => {
    expect(disposalLabel('inherit')).toBe('指定继承');
    expect(disposalLabel('liquidate')).toBe('变现分配');
    expect(disposalLabel('memorialize')).toBe('保留纪念');
    expect(disposalLabel('authorize_view')).toBe('授权查阅');
    expect(disposalLabel('delete')).toBe('彻底删除');
    expect(disposalLabel('donate')).toBe('捐赠');
  });

  it('no_action 统一为「待设定」（文案漂移回归）', () => {
    expect(disposalLabel('no_action')).toBe('待设定');
  });

  it('未知值原样返回', () => {
    expect(disposalLabel('custom_x')).toBe('custom_x');
  });
});
