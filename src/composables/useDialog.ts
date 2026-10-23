import { nextTick, onMounted, onUnmounted, type Ref } from "vue";

const stack: symbol[] = [];
const panels = new Map<symbol, Ref<HTMLElement | null>>();
let previousInert = false;
let previousOverflow = "";
const focusable = 'button:not(:disabled),a[href],input:not(:disabled),select:not(:disabled),textarea:not(:disabled),[tabindex="0"]';

export function useDialog(element: Ref<HTMLElement | null>, close: () => void) {
  const token = Symbol("dialog");
  let previous: HTMLElement | null = null;
  function keydown(event: KeyboardEvent) {
    if (stack[stack.length - 1] !== token || !element.value) return;
    if (event.key === "Escape") { event.preventDefault(); event.stopImmediatePropagation(); close(); }
    if (event.key !== "Tab") return;
    const controls = Array.from(element.value.querySelectorAll<HTMLElement>(focusable)).filter(el => el.getClientRects().length > 0);
    const first = controls[0], last = controls[controls.length - 1];
    if (!first) { event.preventDefault(); element.value.focus(); }
    else if (event.shiftKey && (document.activeElement === first || !element.value.contains(document.activeElement))) { event.preventDefault(); last?.focus(); }
    else if (!event.shiftKey && (document.activeElement === last || !element.value.contains(document.activeElement))) { event.preventDefault(); first.focus(); }
  }
  onMounted(async () => {
    previous = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    const root = document.getElementById("app");
    if (stack.length === 0) {
      previousInert = root?.inert ?? false; previousOverflow = document.body.style.overflow;
      if (root) root.inert = true;
      document.body.style.overflow = "hidden";
    }
    stack.push(token); panels.set(token, element); document.addEventListener("keydown", keydown, true);
    await nextTick();
    if (stack[stack.length - 1] === token) (element.value?.querySelector<HTMLElement>("[data-autofocus]") ?? element.value?.querySelector<HTMLElement>(focusable) ?? element.value)?.focus();
  });
  onUnmounted(() => {
    document.removeEventListener("keydown", keydown, true);
    const wasTop = stack[stack.length - 1] === token;
    const index = stack.indexOf(token); if (index >= 0) stack.splice(index, 1);
    panels.delete(token);
    if (stack.length === 0) {
      const root = document.getElementById("app"); if (root) root.inert = previousInert;
      document.body.style.overflow = previousOverflow;
    }
    if (wasTop) void nextTick(() => {
      const parent = panels.get(stack[stack.length - 1])?.value;
      if (previous?.isConnected && previous !== document.body && (!parent || parent.contains(previous))) previous.focus();
      else (parent?.querySelector<HTMLElement>(focusable) ?? parent)?.focus();
    });
  });
}
