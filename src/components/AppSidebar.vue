<script setup lang="ts">
import { nextTick, onMounted, onUnmounted, ref, watch } from "vue";
import { useRoute, useRouter } from "vue-router";
import { GROUP_LABELS, routes, type NavMeta } from "../router";
import AppIcon from "./AppIcon.vue";
import AppLogo from "./AppLogo.vue";
import RocketMark from "./RocketMark.vue";
import { useGearSpin } from "../composables/useGearSpin";

const props = defineProps<{ expanded: boolean }>();
const gear = useGearSpin();
const route = useRoute();
const router = useRouter();
const emit = defineEmits<{ toggle: [] }>();

function toggle() {
  hovered.value = "";
  focused.value = "";
  dismissed.value = true;
  emit("toggle");
}

const navGroups = (Object.keys(GROUP_LABELS) as NavMeta["group"][]).map((group) => ({
  group,
  label: GROUP_LABELS[group],
  items: routes
    .filter((r) => r.meta.group === group)
    .map((r) => ({ path: r.path, name: r.name, title: r.meta.title, desc: r.meta.desc, icon: r.meta.icon })),
}));

const version = __APP_VERSION__;
const hovered = ref("");
const focused = ref("");
const inputMode = ref<'pointer' | 'keyboard'>('pointer');
const dismissed = ref(false);
/** 展开时文字已可见，不再弹出说明卡片，避免重复 */
const isOpen = (name: string) => !props.expanded && !dismissed.value && (inputMode.value === 'keyboard' ? focused.value : hovered.value) === name;

/* 悬停意图：首次停留 220ms 才弹出；弹出后在相邻项间移动立即切换（类似 macOS 工具提示） */
let intentTimer: number | undefined;
let coolTimer: number | undefined;
let warm = false;

function enter(name: string, keyboard = false) {
  inputMode.value = keyboard ? 'keyboard' : 'pointer';
  dismissed.value = false;
  if (keyboard) { focused.value = name; return; }
  window.clearTimeout(intentTimer);
  window.clearTimeout(coolTimer);
  if (warm) { hovered.value = name; return; }
  intentTimer = window.setTimeout(() => { hovered.value = name; warm = true; }, 220);
}

function leave() {
  window.clearTimeout(intentTimer);
  hovered.value = "";
  coolTimer = window.setTimeout(() => { warm = false; }, 350);
}

/* ---------- 滑动选中指示器 ---------- */
const navEl = ref<HTMLElement | null>(null);
const indicator = ref({ y: 0, h: 0, visible: false });


function placeIndicator() {
  const active = navEl.value?.querySelector<HTMLElement>(".rail__item.is-active");
  if (!active || !navEl.value) { indicator.value.visible = false; return; }
  const navTop = navEl.value.getBoundingClientRect().top;
  const r = active.getBoundingClientRect();
  indicator.value = { y: r.top - navTop + navEl.value.scrollTop, h: r.height, visible: true };
}

watch(() => route.path, () => nextTick(placeIndicator));
watch(() => props.expanded, () => nextTick(placeIndicator));
const ro = typeof ResizeObserver !== "undefined" ? new ResizeObserver(() => placeIndicator()) : null;
onMounted(async () => {
  // 等路由解析完成、选中项渲染出来后再定位，否则首屏找不到选中项
  await router.isReady();
  await nextTick();
  placeIndicator();
  // 字体加载完成后行高可能变化，再校准一次
  void document.fonts?.ready.then(() => placeIndicator());
  if (navEl.value) ro?.observe(navEl.value);
  // 首次定位不做动画，之后的切换才滑动
  requestAnimationFrame(() => { indicatorAnimated.value = true; });
});
const indicatorAnimated = ref(false);
onUnmounted(() => { ro?.disconnect(); window.clearTimeout(intentTimer); window.clearTimeout(coolTimer); });

/** 快捷键提示：Ctrl + 序号 */
const shortcutOf = (name: string) => routes.findIndex((r) => r.name === name) + 1;


function focusItem(event: FocusEvent, name: string) {
  focused.value = name;
  if ((event.currentTarget as HTMLElement).matches(':focus-visible')) enter(name, true);
}

function pointerMove(name: string) {
  if (inputMode.value === 'keyboard') enter(name);
}

function moveFocus(event: KeyboardEvent) {
  const keys = ["ArrowDown", "ArrowUp", "Home", "End"];
  if (!keys.includes(event.key)) return;
  const nav = event.currentTarget as HTMLElement;
  const links = Array.from(nav.querySelectorAll<HTMLAnchorElement>(".rail__item"));
  const index = links.indexOf(document.activeElement as HTMLAnchorElement);
  if (index < 0) return;
  inputMode.value = 'keyboard';
  event.preventDefault();
  const next = event.key === "Home" ? 0 : event.key === "End" ? links.length - 1
    : (index + (event.key === "ArrowDown" ? 1 : -1) + links.length) % links.length;
  links[next]?.focus();
}
</script>

<template>
  <aside class="rail" :class="{ 'rail--expanded': expanded }" aria-label="应用菜单">
    <div class="rail__brand" title="ai2tiku · OCS 智能题库" aria-label="ai2tiku · 无限墨环">
      <span class="rail__brand-mark"><AppLogo /></span>
      <span class="rail__brand-copy" :aria-hidden="!expanded"><strong>ai2tiku</strong><small>AI 题库</small></span>
    </div>
    <button class="rail__toggle" type="button" :aria-expanded="expanded" aria-controls="sidebar-navigation"
      :aria-label="expanded ? '收起菜单' : '展开菜单'" :title="expanded ? '收起菜单' : '展开菜单'" @click="toggle">
      <span class="rail__glyph">
        <svg class="rail__toggle-symbol" width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" aria-hidden="true">
          <rect x="3" y="4" width="18" height="16" rx="2" />
          <path class="rail__toggle-divider" d="M9 4v16" />
          <path class="rail__toggle-arrow" :d="expanded ? 'm16 9-3 3 3 3' : 'm13 9 3 3-3 3'" />
        </svg>
      </span>
      <span class="rail__label" :aria-hidden="!expanded">收起菜单</span>
    </button>

    <nav id="sidebar-navigation" ref="navEl" class="rail__nav" aria-label="主导航" @keydown="moveFocus" @keydown.esc="dismissed = true">
      <span
        class="rail__indicator"
        :class="{ 'is-animated': indicatorAnimated, 'is-visible': indicator.visible }"
        :style="{ transform: `translateY(${indicator.y}px)`, height: `${indicator.h}px` }"
        aria-hidden="true"
      />
      <template v-for="g in navGroups" :key="g.group">
        <div class="rail__group" :aria-hidden="!expanded"><span>{{ g.label }}</span></div>
        <RouterLink
          v-for="item in g.items"
          :key="item.name"
          :to="item.path"
          custom
          v-slot="{ href, navigate, isActive, isExactActive }"
        >
          <div
            class="rail__wrap"
            :class="{ 'is-open': isOpen(item.name) }"
            @mouseenter="enter(item.name)"
            @mousemove="pointerMove(item.name)"
            @mouseleave="leave"
          >
            <a
              :href="href"
              class="rail__item"
              :class="[{ 'is-active': item.path === '/' ? isExactActive : isActive }, `rail__item--${item.icon}`]"
              :aria-label="item.title"
              :aria-current="(item.path === '/' ? isExactActive : isActive) ? 'page' : undefined"
              :aria-describedby="isOpen(item.name) ? `nav-tip-${item.name}` : undefined"
              @focus="focusItem($event, item.name)"
              @blur="focused = ''"
              @click="dismissed = true; item.name === 'settings' && gear.spin(); navigate($event)"
            >
              <span
                class="rail__glyph"
                :class="{ 'rail__glyph--spin': item.name === 'settings' }"
                :style="item.name === 'settings' ? { transform: `rotate(${gear.angle.value}deg)` } : undefined"
                aria-hidden="true"
              >
                <RocketMark v-if="item.name === 'setup'" />
                <AppIcon v-else :name="item.icon" :size="20" />
              </span>
              <span class="rail__label" :aria-hidden="!expanded">{{ item.title }}</span>
            </a>

            <div :id="`nav-tip-${item.name}`" class="rail__flyout" role="tooltip" :aria-hidden="!isOpen(item.name)">
              <div class="rail__flyout-inner">
                <div class="rail__flyout-title">{{ item.title }}<kbd>Ctrl {{ shortcutOf(item.name) }}</kbd></div>
                <div class="rail__flyout-desc">{{ item.desc }}</div>
              </div>
            </div>
          </div>
        </RouterLink>
      </template>
    </nav>

    <div class="rail__foot" :title="`版本 ${version}`">v{{ version }}</div>
  </aside>
</template>
