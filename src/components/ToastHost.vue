<script setup lang="ts">
import { useToast } from "../composables/useToast";

const { items } = useToast();
</script>

<template>
  <div class="toast-host">
    <TransitionGroup name="toast">
      <div
        v-for="t in items"
        :key="t.id"
        class="toast"
        :class="{ 'toast--ok': t.kind === 'ok', 'toast--err': t.kind === 'err' }"
      >
        {{ t.message }}
      </div>
    </TransitionGroup>
  </div>
</template>

<style scoped>
.toast-leave-active {
  transition: opacity 200ms ease, transform 200ms ease;
  position: absolute;
  right: 0;
  bottom: 0;
}

.toast-leave-to {
  opacity: 0;
  transform: translateX(14px);
}
</style>
