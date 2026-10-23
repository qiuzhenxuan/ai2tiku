<script setup lang="ts">
import { ref } from "vue";
import { api } from "../api";
import type { ImportPreview } from "../api/types";
import { useToast } from "../composables/useToast";
import { useConfirm } from "../composables/useConfirm";
import AppModal from "./AppModal.vue";
const emit = defineEmits<{ changed: [] }>();
const toast = useToast(), confirm = useConfirm();
const busy = ref(false), opened = ref(false), mode = ref<'merge' | 'replace'>('merge');
const preview = ref<ImportPreview | null>(null), content = ref(''), filename = ref('');
const backupPath = ref('');
async function exportFile() {
  if (busy.value) return;
  busy.value = true;
  try { const path = await api.exportQuestionFile(); if (path) toast.ok(`题库备份已导出：${path}`); }
  catch (error) { toast.fromError(error); } finally { busy.value = false; }
}
async function pick() {
  if (busy.value) return;
  busy.value = true;
  try {
    const selected = await api.pickQuestionFile(); if (!selected) return;
    const result = await api.previewQuestionImport(selected[1]);
    filename.value = selected[0]; content.value = selected[1]; preview.value = result;
    mode.value = 'merge'; opened.value = true;
  } catch (error) { toast.fromError(error); } finally { busy.value = false; }
}
async function refreshPreview() {
  busy.value = true;
  try { preview.value = await api.previewQuestionImport(content.value); }
  catch (error) { toast.fromError(error); } finally { busy.value = false; }
}
async function apply() {
  if (busy.value || !preview.value || preview.value.invalid) return;
  busy.value = true;
  try {
    if (!await confirm({ title: mode.value === 'replace' ? '恢复并替换整个题库？' : '确认导入题目？',
      message: mode.value === 'replace' ? `将用文件中的 ${preview.value.valid} 道题替换当前全部 ${preview.value.existing} 道题。执行前自动备份当前题库；供应商、API Key 和答题记录保持不变。` : `导入 ${preview.value.newCount} 道新题。已有题目及其答案全部保留，跳过 ${preview.value.duplicates} 道重复题与 ${preview.value.conflicts} 道冲突题。`,
      confirmText: mode.value === 'replace' ? '备份并恢复' : '确认导入', danger: mode.value === 'replace' })) return;
    const result = await api.applyQuestionImport(content.value, mode.value, preview.value.snapshot);
    backupPath.value = result.backupPath; opened.value = false; content.value = ''; preview.value = null;
    toast.ok(`已导入 ${result.imported} 道，跳过 ${result.skipped} 道，题库共 ${result.total} 道`); emit('changed');
  } catch (error) { toast.fromError(error); } finally { busy.value = false; }
}
</script>
<template>
  <div class="transfer-bar">
    <div><strong>题库备份与迁移</strong><p class="faint small">导出整个题库为 JSON；仅含题目，不含 API Key、设置和答题记录。</p></div>
    <div class="row"><button class="btn" type="button" :disabled="busy" @click="exportFile">备份 / 导出</button><button class="btn" type="button" :disabled="busy" @click="pick">导入 / 恢复</button></div>
    <p v-if="backupPath" class="transfer-backup small">恢复前的自动备份：<span class="mono">{{ backupPath }}</span></p>
  </div>
  <AppModal v-if="opened && preview" title="导入预览" wide @close="!busy && (opened = false)">
    <div class="stack">
      <p class="detail-text"><strong>{{ filename }}</strong><br><span class="faint small">文件 {{ preview.total }} 条 · 可恢复 {{ preview.valid }} 道 · 文件内重复 {{ preview.fileDuplicates }} 条</span></p>
      <p v-if="preview.fileConflicts" class="callout callout--warn">备份中有 {{ preview.fileConflicts }} 条同题冲突。整库恢复会保留备份中的原始数据；合并时同题仅保留第一条，请先核对答案。</p>
      <div class="import-counts"><div><strong>{{ preview.newCount }}</strong><span>新题目</span></div><div><strong>{{ preview.duplicates }}</strong><span>已有重复题</span></div><div><strong>{{ preview.conflicts }}</strong><span>与现有答案冲突</span></div><div><strong>{{ preview.invalid }}</strong><span>无效条目</span></div></div>
      <div v-if="preview.invalid" class="callout callout--warn"><div>文件存在无效条目，本次不会写入任何题目。修正文件后重新选择。<ul><li v-for="error in preview.errors" :key="error">{{ error }}</li></ul></div></div>
      <label class="import-mode"><input v-model="mode" type="radio" value="merge" :disabled="busy"><span><strong>合并导入</strong><small>只添加新题，保留现有题目，跳过重复和冲突。</small></span></label>
      <label class="import-mode"><input v-model="mode" type="radio" value="replace" :disabled="busy"><span><strong>恢复整个题库</strong><small>使用此文件替换全部题目；执行前自动保存当前题库备份。</small></span></label>
      <p class="field__help">支持本软件的版本化 JSON 备份，以及含 title、options、qtype、answer 的题目数组。恢复仅作用于题库。</p>
    </div>
    <template #foot><button class="btn" type="button" :disabled="busy" @click="refreshPreview">重新预览</button><button class="btn" type="button" :disabled="busy" @click="opened = false">取消</button><button class="btn btn--primary" type="button" :disabled="busy || preview.invalid > 0 || (mode === 'replace' && preview.valid === 0)" @click="apply">{{ busy ? '处理中…' : mode === 'replace' ? '恢复题库' : '导入新题' }}</button></template>
  </AppModal>
</template>
