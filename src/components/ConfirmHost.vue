<script setup lang="ts">
import { watch } from "vue";
import { useRoute } from "vue-router";
import AppModal from "./AppModal.vue";
import { confirmation, settleConfirmation } from "../composables/useConfirm";
const route = useRoute();
watch(() => route.fullPath, () => settleConfirmation(false));
</script>
<template>
  <AppModal :open="!!confirmation" :title="confirmation?.title || ''" @close="settleConfirmation(false)">
    <p class="confirmation-message">{{ confirmation?.message }}</p>
    <template #foot>
      <button class="btn" type="button" data-autofocus @click="settleConfirmation(false)">取消</button>
      <button class="btn" :class="confirmation?.danger ? 'btn--danger' : 'btn--primary'" type="button" @click="settleConfirmation(true)">{{ confirmation?.confirmText || '确认' }}</button>
    </template>
  </AppModal>
</template>
