/** DMS 检查间隔的单一事实来源：两个页面都从这里取，避免选项集合漂移。 */
import { t } from '../i18n';

export interface IntervalOption {
  value: number;
  labelKey: string;
  shortKey: string;
}

export const INTERVAL_OPTIONS: IntervalOption[] = [
  { value: 7, labelKey: 'dms.interval7', shortKey: 'dms.interval7Short' },
  { value: 14, labelKey: 'dms.interval14', shortKey: 'dms.interval14Short' },
  { value: 30, labelKey: 'dms.interval30', shortKey: 'dms.interval30Short' },
  { value: 60, labelKey: 'dms.interval60', shortKey: 'dms.interval60Short' },
  { value: 90, labelKey: 'dms.interval90', shortKey: 'dms.interval90Short' },
];

export function intervalLabel(days: number, short = false): string {
  const hit = INTERVAL_OPTIONS.find(o => o.value === days);
  if (hit) return t(short ? hit.shortKey : hit.labelKey);
  return t('dms.intervalCustom', { days });
}
