import { describe, expect, it } from 'vitest';

import { t } from '../i18n';
import { INTERVAL_OPTIONS, intervalLabel } from './dms';

// 两个页面共用同一份选项：任何一处单独改文案/数值都会让「档案页显示 60 天、
// DMS 页没有 60 这个选项」的历史缺陷复现。
describe('DMS 间隔常量', () => {
  it('选项值唯一且覆盖 7/14/30/60/90', () => {
    const values = INTERVAL_OPTIONS.map(o => o.value);
    expect(new Set(values).size).toBe(values.length);
    expect(values).toEqual([7, 14, 30, 60, 90]);
  });

  it('每个选项都有长短两种文案（经 i18n 解析后非空）', () => {
    for (const opt of INTERVAL_OPTIONS) {
      expect(opt.labelKey.length).toBeGreaterThan(0);
      expect(opt.shortKey.length).toBeGreaterThan(0);
      expect(t(opt.labelKey).length).toBeGreaterThan(0);
      expect(t(opt.shortKey).length).toBeGreaterThan(0);
    }
  });

  it('已知间隔返回对应文案，未知值回退为「每 N 天」', () => {
    expect(intervalLabel(30)).toBe('每月确认一次');
    expect(intervalLabel(30, true)).toBe('每月');
    expect(intervalLabel(45)).toBe('每 45 天');
    expect(intervalLabel(45, true)).toBe('每 45 天');
  });
});
