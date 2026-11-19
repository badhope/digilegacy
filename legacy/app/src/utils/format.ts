// 跨视图共享的格式化与文案映射（单一事实来源）
//
// 提取自 DashboardView / AssetsView / ReportView / AssetDetailView 的重复实现。
// 文案与规则只在此处维护，避免出现「待设定 / 待定」之类的不一致漂移
// （曾因三视图各持一份副本导致同一枚举值显示两种文案的实际缺陷）。

import { locale, t } from '../i18n';

/** 金额格式化：中文按「万」，英文按 k/M/B；空值/0 显示「0」 */
export function formatMoney(n: number): string {
  if (!n) return '0';
  if (locale.value === 'en-US') {
    if (n >= 1_000_000_000) return `${(n / 1_000_000_000).toFixed(1)}B`;
    if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
    if (n >= 1_000) return `${(n / 1_000).toFixed(1)}k`;
    return n.toLocaleString('en-US');
  }
  if (n >= 10000) {
    return t('common.tenThousand', { value: (n / 10000).toFixed(1) });
  }
  return n.toLocaleString('zh-CN');
}

/** 资产类别文案 */
export function categoryLabel(cat: string): string {
  const map: Record<string, string> = {
    property: 'cat.property',
    personality: 'cat.personality',
    mixed: 'cat.mixed',
  };
  return t(map[cat] || cat);
}

/** 处置意愿文案（no_action 统一为「待设定」，与资产详情下拉选项「暂不设定」语义对应） */
export function disposalLabel(method: string): string {
  const map: Record<string, string> = {
    inherit: 'disposalLabel.inherit',
    liquidate: 'disposalLabel.liquidate',
    memorialize: 'disposalLabel.memorialize',
    authorize_view: 'disposalLabel.authorize_view',
    delete: 'disposalLabel.delete',
    donate: 'disposalLabel.donate',
    no_action: 'disposalLabel.no_action',
  };
  return t(map[method] || method);
}
