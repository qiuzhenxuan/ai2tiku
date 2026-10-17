<script setup lang="ts">
import { computed, onMounted, reactive, ref } from "vue";
import { api } from "../api";
import type { Question, QuestionInput } from "../api/types";
import AppIcon from "../components/AppIcon.vue";
import AppModal from "../components/AppModal.vue";
import { useToast } from "../composables/useToast";
import { formatDate, sourceLabel, typeLabel } from "../utils/format";

const toast = useToast();

const items = ref<Question[]>([]);
const total = ref(0);
const page = ref(1);
const pageSize = 15;
const keyword = ref("");
const source = ref("");
const loading = ref(false);

const modal = ref(false);
const form = reactive<QuestionInput>({ id: null, title: "", options: "", qtype: "single", answer: "" });

const totalPages = computed(() => Math.max(1, Math.ceil(total.value / pageSize)));

async function load() {
  loading.value = true;
  try {
    const res = await api.listQuestions(keyword.value.trim(), source.value, page.value, pageSize);
    items.value = res.items;
    total.value = res.total;
  } catch (e) {
    toast.fromError(e);
  } finally {
    loading.value = false;
  }
}

function search() {
  page.value = 1;
  load();
}

function go(delta: number) {
  const next = page.value + delta;
  if (next < 1 || next > totalPages.value) return;
  page.value = next;
  load();
}

function open(item?: Question) {
  Object.assign(form, {
    id: item?.id ?? null,
    title: item?.title ?? "",
    options: item?.options ?? "",
    qtype: item?.qtype ?? "single",
    answer: item?.answer ?? "",
  });
  modal.value = true;
}

async function submit() {
  try {
    await api.saveQuestion({ ...form });
    modal.value = false;
    toast.ok("题目已保存");
    await load();
  } catch (e) {
    toast.fromError(e);
  }
}

async function remove(item: Question) {
  if (!window.confirm("删除这道题？")) return;
  try {
    await api.deleteQuestion(item.id);
    toast.ok("已删除");
    await load();
  } catch (e) {
    toast.fromError(e);
  }
}

async function clearAll() {
  if (!window.confirm(`清空全部 ${total.value} 道题？该操作不可撤销。`)) return;
  try {
    await api.clearQuestions();
    toast.ok("题库已清空");
    page.value = 1;
    await load();
  } catch (e) {
    toast.fromError(e);
  }
}

onMounted(load);
</script>

<template>
  <div class="page">
    <div class="card">
      <div class="card__head">
        <AppIcon name="database" :size="15" />
        <div class="row grow" style="gap: 10px">
          <input
            v-model="keyword"
            class="input"
            style="max-width: 320px"
            placeholder="搜索题干关键词…"
            @keyup.enter="search"
          />
          <select v-model="source" class="select" style="max-width: 130px" @change="search">
            <option value="">全部来源</option>
            <option value="ai">AI 作答</option>
            <option value="manual">手动录入</option>
          </select>
          <button class="btn" type="button" @click="search">
            <AppIcon name="search" :size="14" />
            搜索
          </button>
        </div>
        <button class="btn btn--primary btn--sm" type="button" @click="open()">
          <AppIcon name="plus" :size="13" />
          新增题目
        </button>
      </div>

      <div v-if="items.length === 0" class="empty">
        <div class="empty__title">{{ loading ? "加载中…" : "没有匹配的题目" }}</div>
        AI 答过的题会自动缓存到这里，也可以手动录入
      </div>
      <template v-else>
        <div class="table-wrap">
          <table class="table">
            <thead>
              <tr>
                <th style="width: 38%">题干</th>
                <th style="width: 20%">答案</th>
                <th>题型</th>
                <th>来源</th>
                <th class="num">命中</th>
                <th class="num">更新时间</th>
                <th class="num">操作</th>
              </tr>
            </thead>
            <tbody>
              <tr v-for="q in items" :key="q.id">
                <td><div class="cell-clip">{{ q.title }}</div></td>
                <td><div class="mono cell-clip">{{ q.answer }}</div></td>
                <td class="faint small nowrap">{{ typeLabel(q.qtype) }}</td>
                <td>
                  <span class="badge" :class="q.source === 'ai' ? 'badge--ai' : 'badge--ok'">
                    {{ sourceLabel(q.source) }}
                  </span>
                </td>
                <td class="num">{{ q.hits }}</td>
                <td class="num faint small nowrap">{{ formatDate(q.updatedAt) }}</td>
                <td>
                  <div class="row" style="justify-content: flex-end">
                    <button class="btn btn--ghost btn--sm" type="button" @click="open(q)">
                      <AppIcon name="edit" :size="13" />
                    </button>
                    <button class="btn btn--ghost btn--sm" type="button" @click="remove(q)">
                      <AppIcon name="trash" :size="13" />
                    </button>
                  </div>
                </td>
              </tr>
            </tbody>
          </table>
        </div>

        <div class="pager">
          <span>共 {{ total }} 道 · 第 {{ page }} / {{ totalPages }} 页</span>
          <button class="btn btn--sm" type="button" :disabled="page <= 1" @click="go(-1)">上一页</button>
          <button class="btn btn--sm" type="button" :disabled="page >= totalPages" @click="go(1)">下一页</button>
          <button class="btn btn--danger btn--sm" type="button" @click="clearAll">清空题库</button>
        </div>
      </template>
    </div>

    <AppModal v-if="modal" title="题目" wide @close="modal = false">
      <div class="stack">
        <div class="field">
          <label class="field__label" for="q-type">题型</label>
          <select id="q-type" v-model="form.qtype" class="select">
            <option value="single">单选题</option>
            <option value="multiple">多选题</option>
            <option value="judgement">判断题</option>
            <option value="completion">填空题</option>
            <option value="unknown">未知</option>
          </select>
        </div>
        <div class="field">
          <label class="field__label" for="q-title">题干</label>
          <textarea id="q-title" v-model="form.title" class="textarea" rows="3" />
        </div>
        <div class="field">
          <label class="field__label" for="q-options">选项（每行一个）</label>
          <textarea id="q-options" v-model="form.options" class="textarea" rows="4" />
        </div>
        <div class="field">
          <label class="field__label" for="q-answer">答案</label>
          <textarea id="q-answer" v-model="form.answer" class="textarea" rows="2" />
          <span class="field__help">
            多选题请填选项字母并用 <code class="mono">#</code> 或直接填 JSON 数组，例如
            <code class="mono">["A","C"]</code>
          </span>
        </div>
      </div>
      <template #foot>
        <button class="btn" type="button" @click="modal = false">取消</button>
        <button class="btn btn--primary" type="button" @click="submit">保存</button>
      </template>
    </AppModal>
  </div>
</template>
