<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref, useId } from 'vue'
const props = withDefaults(defineProps<{ title: string; wide?: boolean; closeable?: boolean }>(), { closeable: true })
const emit = defineEmits<{ close: [] }>()
const titleId = useId()
const panel = ref<HTMLElement>()
let previous: HTMLElement | null = null
let oldOverflow = ''
function onKey(event: KeyboardEvent) {
  if (event.key === 'Escape' && props.closeable) { event.preventDefault(); emit('close') }
  if (event.key !== 'Tab' || !panel.value) return
  const nodes = [...panel.value.querySelectorAll<HTMLElement>('button:not(:disabled),a[href],input:not(:disabled),select:not(:disabled),textarea:not(:disabled),[tabindex="0"]')].filter(e => e.getClientRects().length)
  if (!nodes.length) { event.preventDefault(); panel.value.focus(); return }
  const first = nodes[0]!, last = nodes[nodes.length - 1]!
  if (event.shiftKey && (document.activeElement === first || document.activeElement === panel.value)) { event.preventDefault(); last.focus() }
  else if (!event.shiftKey && (document.activeElement === last || !panel.value.contains(document.activeElement))) { event.preventDefault(); first.focus() }
}
onMounted(() => {
  previous = document.activeElement as HTMLElement
  oldOverflow = document.body.style.overflow
  document.body.style.overflow = 'hidden'
  document.getElementById('workspace')?.setAttribute('inert', '')
  document.addEventListener('keydown', onKey)
  requestAnimationFrame(() => (panel.value?.querySelector<HTMLElement>('[autofocus]') || panel.value)?.focus())
})
onBeforeUnmount(() => {
  document.removeEventListener('keydown', onKey)
  document.body.style.overflow = oldOverflow
  document.getElementById('workspace')?.removeAttribute('inert')
  if (previous?.isConnected) previous.focus()
})
</script>
<template>
  <Teleport to="body">
    <div class="modal-layer" @mousedown.self="props.closeable && emit('close')">
      <section ref="panel" class="modal-panel" :class="{ wide }" role="dialog" aria-modal="true" :aria-labelledby="titleId" tabindex="-1">
        <header class="modal-heading"><h2 :id="titleId">{{ title }}</h2><button v-if="closeable" class="icon-button" type="button" aria-label="关闭弹窗" @click="emit('close')">×</button></header>
        <slot />
      </section>
    </div>
  </Teleport>
</template>
