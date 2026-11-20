import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

const tokens = readFileSync(resolve(process.cwd(), "src/design/tokens.css"), "utf8");

function declarations(selector: string): Record<string, string> {
  const start = tokens.indexOf(selector);
  if (start < 0) throw new Error(`missing selector: ${selector}`);
  const open = tokens.indexOf("{", start);
  const close = tokens.indexOf("}", open);
  const body = tokens.slice(open + 1, close);
  return Object.fromEntries(
    Array.from(body.matchAll(/--([a-z-]+)\s*:\s*#([0-9a-fA-F]{6})/g)).map((match) => [
      match[1],
      `#${match[2]}`,
    ]),
  );
}

function luminance(hex: string): number {
  const channels = hex
    .slice(1)
    .match(/../g)!
    .map((part) => Number.parseInt(part, 16) / 255)
    .map((value) => (value <= 0.03928 ? value / 12.92 : ((value + 0.055) / 1.055) ** 2.4));
  return 0.2126 * channels[0] + 0.7152 * channels[1] + 0.0722 * channels[2];
}

function ratio(foreground: string, background: string): number {
  const first = luminance(foreground);
  const second = luminance(background);
  return (Math.max(first, second) + 0.05) / (Math.min(first, second) + 0.05);
}

const themes = {
  light: declarations(":root {"),
  dark: declarations(':root[data-theme="dark"] {'),
};

const textPairs: Array<[string, string]> = [
  ["ink", "bg"],
  ["ink-soft", "bg"],
  ["muted", "bg"],
  ["ink", "surface"],
  ["ink-soft", "surface"],
  ["muted", "surface"],
  ["muted", "surface-soft"],
  ["muted", "surface-strong"],
  ["muted-strong", "surface"],
  ["accent-strong", "accent-soft"],
  ["accent-ink", "accent"],
  ["clay", "clay-soft"],
  ["amber", "amber-soft"],
  ["danger", "danger-soft"],
  ["info", "info-soft"],
];

describe.each(Object.entries(themes))("%s theme contrast", (_name, theme) => {
  it.each(textPairs)("%s on %s meets WCAG AA", (foreground, background) => {
    const value = ratio(theme[foreground], theme[background]);
    expect(value, `${foreground} on ${background} = ${value.toFixed(2)}:1`).toBeGreaterThanOrEqual(4.5);
  });
});
