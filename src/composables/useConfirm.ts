import { shallowRef } from "vue";
export interface Confirmation { title: string; message: string; confirmText?: string; danger?: boolean }
export const confirmation = shallowRef<Confirmation | null>(null);
let resolver: ((value: boolean) => void) | null = null;
let opener: HTMLElement | null = null;
export function settleConfirmation(value: boolean) {
  const resolve = resolver, target = opener; resolver = null; opener = null; confirmation.value = null; resolve?.(value);
  requestAnimationFrame(() => { if (target?.isConnected && !target.matches(':disabled') && !target.closest('[inert]')) target.focus(); });
}
export function useConfirm() {
  return (options: Confirmation) => {
    if (resolver) return Promise.resolve(false);
    opener = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    confirmation.value = options;
    return new Promise<boolean>(resolve => { resolver = resolve; });
  };
}
