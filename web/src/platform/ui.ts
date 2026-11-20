import { reactive } from "vue";

export const ui = reactive({
  agentOpen: false,
  paletteOpen: false,
  securityReady: false,
  securityConfigured: false,
  locked: false,
});

export function openAgent() {
  ui.agentOpen = true;
}

export function closeAgent() {
  ui.agentOpen = false;
}

export function openPalette() {
  ui.paletteOpen = true;
}

export function closePalette() {
  ui.paletteOpen = false;
}

export function markLocked() {
  ui.locked = true;
  ui.agentOpen = false;
  ui.paletteOpen = false;
}
