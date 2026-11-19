import { reactive } from "vue";

export const ui = reactive({
  agentOpen: false,
  paletteOpen: false,
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

export function lockConsole() {
  ui.locked = true;
  ui.agentOpen = false;
  ui.paletteOpen = false;
}

export function unlockConsole() {
  ui.locked = false;
}
