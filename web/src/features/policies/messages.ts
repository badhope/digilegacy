import { computed } from "vue";
import { useI18n } from "@/platform/i18n";
import policyZh from "../../../../content/i18n/policies/zh-CN.json";
import policyEn from "../../../../content/i18n/policies/en-US.json";

const dictionaries: Record<string, Record<string, string>> = {
  "zh-CN": policyZh as Record<string, string>,
  "en-US": policyEn as Record<string, string>,
};

/** Policy-only copy lives in its lazy route chunk, not the global entry bundle. */
export function usePolicyI18n() {
  const { locale } = useI18n();
  const messages = computed(() => dictionaries[locale.value]);

  function tp(key: string, vars: Record<string, string | number> = {}): string {
    const template = messages.value[key] ?? (policyZh as Record<string, string>)[key] ?? key;
    return Object.entries(vars).reduce(
      (text, [name, value]) => text.replaceAll(`{${name}}`, String(value)),
      template,
    );
  }

  return { tp };
}
