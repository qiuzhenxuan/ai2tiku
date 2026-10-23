<script setup lang="ts">
import AppIcon from "./AppIcon.vue";
import { ref, useId } from "vue";
import { useDialog } from "../composables/useDialog";

defineProps<{ title: string; wide?: boolean; drawer?: boolean }>();
const emit = defineEmits<{ close: [] }>();
const panel = ref<HTMLElement | null>(null);
const titleId = useId();
useDialog(panel, () => emit("close"));
</script>

<template>
  <Teleport to="body">
  <div class="modal-mask" :class="{ 'modal-mask--drawer': drawer }" @click.self="emit('close')">
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
  </Teleport>
</template>
