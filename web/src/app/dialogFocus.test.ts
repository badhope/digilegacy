import { describe, expect, it } from "vitest";
import { focusableElements, trapTabKey } from "./dialogFocus";

function key(key: string, options: KeyboardEventInit = {}): KeyboardEvent {
  return new KeyboardEvent("keydown", { key, bubbles: true, cancelable: true, ...options });
}

describe("dialog focus", () => {
  it("collects focusable controls and ignores tabindex=-1", () => {
    document.body.innerHTML = `
      <div id="panel" tabindex="-1">
        <button id="first">First</button>
        <input id="middle" />
        <button id="last">Last</button>
        <span tabindex="-1">Skip</span>
      </div>`;
    const panel = document.getElementById("panel") as HTMLElement;
    expect(focusableElements(panel).map((element) => element.id)).toEqual(["first", "middle", "last"]);
  });

  it("wraps Tab from the last control to the first", () => {
    document.body.innerHTML = `
      <div id="panel">
        <button id="first">First</button>
        <button id="last">Last</button>
      </div>`;
    const panel = document.getElementById("panel") as HTMLElement;
    document.getElementById("last")?.focus();
    const event = key("Tab");
    expect(trapTabKey(event, panel)).toBe(true);
    expect(event.defaultPrevented).toBe(true);
    expect(document.activeElement?.id).toBe("first");
  });

  it("wraps Shift+Tab from the first control to the last", () => {
    document.body.innerHTML = `
      <div id="panel">
        <button id="first">First</button>
        <button id="last">Last</button>
      </div>`;
    const panel = document.getElementById("panel") as HTMLElement;
    document.getElementById("first")?.focus();
    const event = key("Tab", { shiftKey: true });
    expect(trapTabKey(event, panel)).toBe(true);
    expect(document.activeElement?.id).toBe("last");
  });

  it("pulls focus back into the panel when it sits outside", () => {
    document.body.innerHTML = `
      <button id="outside">Outside</button>
      <div id="panel"><button id="inside">Inside</button></div>`;
    const panel = document.getElementById("panel") as HTMLElement;
    document.getElementById("outside")?.focus();
    const event = key("Tab");
    expect(trapTabKey(event, panel)).toBe(true);
    expect(document.activeElement?.id).toBe("inside");
  });

  it("wraps Shift+Tab from the panel container to its last control", () => {
    document.body.innerHTML = `
      <div id="panel" tabindex="-1">
        <button id="first">First</button>
        <button id="last">Last</button>
      </div>`;
    const panel = document.getElementById("panel") as HTMLElement;
    panel.focus();
    const event = key("Tab", { shiftKey: true });
    expect(trapTabKey(event, panel)).toBe(true);
    expect(document.activeElement?.id).toBe("last");
  });
});
