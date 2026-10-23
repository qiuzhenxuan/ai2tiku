<script setup lang="ts">
import AppIcon from "./AppIcon.vue";
import { ref, useId } from "vue";
import { useDialog } from "../composables/useDialog";

const props = defineProps<{ open: boolean; title: string; wide?: boolean; drawer?: boolean }>();
const emit = defineEmits<{ close: [] }>();
const panel = ref<HTMLElement | null>(null);
const titleId = useId();
const { layer } = useDialog(panel, () => emit("close"), () => props.open);
function leaving(element: Element) {
  (element as HTMLElement).inert = true;
  element.setAttribute('aria-hidden', 'true');
}
function entering(element: Element) {
  (element as HTMLElement).inert = false;
  element.removeAttribute('aria-hidden');
}
</script>

<template>
  <Teleport to="body">
  <Transition name="dialog" @before-enter="entering" @before-leave="leaving" @leave-cancelled="entering">
  <div v-if="open" class="modal-mask" :style="{ zIndex: layer }" :class="{ 'modal-mask--drawer': drawer }" @click.self="emit('close')">
    <div ref="panel" class="modal" :class="{ 'modal--drawer': drawer }" :style="wide && !drawer ? 'max-width: 720px' : ''" role="dialog" aria-modal="true" :aria-labelledby="titleId" tabindex="-1">
      <div class="modal__head">
        <span :id="titleId" class="modal__title">{{ title }}</span>
        <button
          class="btn btn--ghost btn--icon"
          type="button"
          style="margin-left: auto"
          aria-label="关闭"
          @click="emit('close')"
        >
          <AppIcon name="close" :size="15" />
        </button>
      </div>
      <div class="modal__body">
        <slot />
      </div>
      <div v-if="$slots.foot" class="modal__foot">
        <slot name="foot" />
      </div>
    </div>
  </div>
  </Transition>
  </Teleport>
</template>
