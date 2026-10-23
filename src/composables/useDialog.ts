import { nextTick, onUnmounted, ref, watch, type Ref } from "vue";

const stack: symbol[] = [];
const panels = new Map<symbol, Ref<HTMLElement | null>>();
let previousInert = false;
let previousOverflow = "";
const focusable = 'button:not(:disabled),a[href],input:not(:disabled),select:not(:disabled),textarea:not(:disabled),[tabindex="0"]';

export function useDialog(element: Ref<HTMLElement | null>, close: () => void, active: () => boolean) {
  const token = Symbol("dialog");
  const layer = ref(60);
  let previous: HTMLElement | null = null;
  let registered = false;
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
  async function activate() {
    if (registered) return;
    registered = true;
    previous = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    const root = document.getElementById("app");
    if (stack.length === 0) {
      previousInert = root?.inert ?? false; previousOverflow = document.body.style.overflow;
      if (root) root.inert = true;
      document.body.style.overflow = "hidden";
    }
    layer.value = 60 + stack.length * 2;
    stack.push(token); panels.set(token, element); document.addEventListener("keydown", keydown, true);
    await nextTick();
    if (registered && stack[stack.length - 1] === token) (element.value?.querySelector<HTMLElement>("[data-autofocus]") ?? element.value?.querySelector<HTMLElement>(focusable) ?? element.value)?.focus();
  }
  function deactivate() {
    if (!registered) return;
    registered = false;
    document.removeEventListener("keydown", keydown, true);
    const wasTop = stack[stack.length - 1] === token;
    const index = stack.indexOf(token); if (index >= 0) stack.splice(index, 1);
    panels.delete(token);
    if (stack.length === 0) {
      const root = document.getElementById("app"); if (root) root.inert = previousInert;
      document.body.style.overflow = previousOverflow;
    }
    const expectedTop = stack[stack.length - 1];
    const target = previous;
    if (wasTop) void nextTick(() => {
      if (registered || stack[stack.length - 1] !== expectedTop) return;
      const parent = panels.get(stack[stack.length - 1])?.value;
      if (target?.isConnected && target !== document.body && !target.matches(':disabled') && !target.closest('[inert]') && (!parent || parent.contains(target))) target.focus();
      else (parent?.querySelector<HTMLElement>(focusable) ?? parent)?.focus();
    });
  }
  watch(() => active() && !!element.value, visible => { if (visible) void activate(); else deactivate(); }, { flush: 'post' });
  onUnmounted(deactivate);
  return { layer };
}
