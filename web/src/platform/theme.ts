import { ref } from "vue";

export type Theme = "light" | "dark";

const stored = typeof localStorage === "undefined" ? null : localStorage.getItem("digilegacy.theme");
const initial: Theme = stored === "dark" ? "dark" : "light";
const theme = ref<Theme>(initial);

function apply(value: Theme) {
  if (typeof document !== "undefined") {
    document.documentElement.dataset.theme = value;
  }
}

apply(initial);

export function setTheme(value: Theme) {
  theme.value = value;
  apply(value);
  if (typeof localStorage !== "undefined") {
    localStorage.setItem("digilegacy.theme", value);
  }
}

export function useTheme() {
  return {
    theme,
    setTheme,
    toggleTheme: () => setTheme(theme.value === "light" ? "dark" : "light"),
  };
}
