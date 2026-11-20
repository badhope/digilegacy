import { nextTick, onBeforeUnmount, onMounted, watch, type Ref } from "vue";

const FOCUSABLE =
  'a[href], button:not([disabled]), input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])';

export function focusableElements(root: HTMLElement): HTMLElement[] {
  return Array.from(root.querySelectorAll<HTMLElement>(FOCUSABLE)).filter(
    (element) => element.getAttribute("aria-hidden") !== "true",
  );
}

export function trapTabKey(event: KeyboardEvent, root: HTMLElement | null): boolean {
  if (!root || event.key !== "Tab") return false;
  const items = focusableElements(root);
  if (!items.length) {
    event.preventDefault();
    root.focus();
    return true;
  }
  const active = document.activeElement;
  if (!root.contains(active)) {
    event.preventDefault();
    (event.shiftKey ? items[items.length - 1] : items[0]).focus();
    return true;
  }
  if (active === root) {
    event.preventDefault();
    (event.shiftKey ? items[items.length - 1] : items[0]).focus();
    return true;
  }
  if (event.shiftKey && active === items[0]) {
    event.preventDefault();
    items[items.length - 1].focus();
    return true;
  }
  if (!event.shiftKey && active === items[items.length - 1]) {
    event.preventDefault();
    items[0].focus();
    return true;
  }
  return false;
}

/**
 * Keep keyboard focus inside a modal and return it to the opener on close.
 * Components still own Escape handling because not every layer is dismissible.
 */
export function useDialogFocus(
  open: Ref<boolean>,
  panel: Ref<HTMLElement | null>,
  initial: "panel" | "first" = "panel",
  restoreFocus: Ref<boolean> | boolean = true,
): void {
  let previous: HTMLElement | null = null;

  function onKeydown(event: KeyboardEvent) {
    if (open.value) trapTabKey(event, panel.value);
  }

  watch(
    open,
    async (value) => {
      if (value) {
        previous = document.activeElement instanceof HTMLElement ? document.activeElement : null;
        await nextTick();
        const root = panel.value;
        if (!root) return;
        if (initial === "first") {
          (focusableElements(root)[0] ?? root).focus();
        } else {
          root.focus();
        }
      } else {
        await nextTick();
        const shouldRestore = typeof restoreFocus === "boolean" ? restoreFocus : restoreFocus.value;
        if (shouldRestore && previous?.isConnected) previous.focus();
        previous = null;
      }
    },
    { immediate: true },
  );

  onMounted(() => window.addEventListener("keydown", onKeydown));
  onBeforeUnmount(() => window.removeEventListener("keydown", onKeydown));
}
