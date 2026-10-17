<script setup lang="ts">
import AppIcon from "./AppIcon.vue";

defineProps<{ title: string; wide?: boolean }>();
const emit = defineEmits<{ close: [] }>();
</script>

<template>
  <div class="modal-mask" @click.self="emit('close')">
    <div class="modal" :style="wide ? 'max-width: 720px' : ''" role="dialog" aria-modal="true">
      <div class="modal__head">
        <span class="modal__title">{{ title }}</span>
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
</template>
