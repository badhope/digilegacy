import { computed, ref } from "vue";
import type { Locale } from "./types";
import zhCN from "../../../content/i18n/zh-CN/ui.json";
import enUS from "../../../content/i18n/en-US/ui.json";

const dictionaries: Record<Locale, Record<string, string>> = {
  "zh-CN": zhCN,
  "en-US": enUS,
};

const stored = typeof localStorage === "undefined" ? null : localStorage.getItem("digilegacy.locale");
const initial: Locale = stored === "en-US" ? "en-US" : "zh-CN";
const locale = ref<Locale>(initial);

function applyDocumentLocale(value: Locale) {
  if (typeof document !== "undefined") {
    document.documentElement.lang = value;
  }
}

applyDocumentLocale(initial);

export function t(key: string, vars: Record<string, string | number> = {}): string {
  const template = dictionaries[locale.value][key] ?? dictionaries["zh-CN"][key] ?? key;
  return Object.entries(vars).reduce(
    (text, [name, value]) => text.replaceAll(`{${name}}`, String(value)),
    template,
  );
}

export function setLocale(value: Locale) {
  locale.value = value;
  applyDocumentLocale(value);
  if (typeof localStorage !== "undefined") {
    localStorage.setItem("digilegacy.locale", value);
  }
}

export function useI18n() {
  return {
    locale,
    isEnglish: computed(() => locale.value === "en-US"),
    t,
    setLocale,
    toggleLocale: () => setLocale(locale.value === "zh-CN" ? "en-US" : "zh-CN"),
  };
}
