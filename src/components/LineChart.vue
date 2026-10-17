<script setup lang="ts">
/** 轻量 SVG 折线图，颜色全部取自 CSS 变量，日夜主题自动适配 */
const props = withDefaults(
  defineProps<{
    points: { label: string; value: number }[];
    height?: number;
    unit?: string;
  }>(),
  { height: 168, unit: "" },
);

const W = 640;
const PAD_L = 34;
const PAD_R = 10;
const PAD_T = 12;
const PAD_B = 24;

function build() {
  const pts = props.points ?? [];
  const h = props.height;
  const innerW = W - PAD_L - PAD_R;
  const innerH = h - PAD_T - PAD_B;
  const max = Math.max(1, ...pts.map((p) => p.value));
  // 让峰值留一点顶部余量，视觉上更舒服
  const top = Math.ceil(max * 1.15) || 1;
  const step = pts.length > 1 ? innerW / (pts.length - 1) : innerW;

  const coords = pts.map((p, i) => ({
    x: PAD_L + i * step,
    y: PAD_T + innerH - (p.value / top) * innerH,
    ...p,
  }));

  const line = coords.map((c) => `${c.x.toFixed(1)},${c.y.toFixed(1)}`).join(" ");
  const area =
    coords.length > 0
      ? `${PAD_L},${PAD_T + innerH} ${line} ${(PAD_L + innerW).toFixed(1)},${PAD_T + innerH}`
      : "";

  const gridLines = [0, 0.5, 1].map((r) => ({
    y: PAD_T + innerH * (1 - r),
    value: Math.round(top * r),
  }));

  // x 轴只显示首、中、尾三个标签，保持简洁
  const labelIdx = pts.length > 2 ? [0, Math.floor(pts.length / 2), pts.length - 1] : [0];

  return { coords, line, area, gridLines, innerH, labelIdx, h };
}

const geo = build();
</script>

<template>
  <svg class="chart" :viewBox="`0 0 ${W} ${geo.h}`" :height="geo.h" preserveAspectRatio="none">
    <line
      v-for="g in geo.gridLines"
      :key="g.y"
      class="chart__grid"
      :x1="PAD_L"
      :x2="W - PAD_R"
      :y1="g.y"
      :y2="g.y"
    />
    <text
      v-for="g in geo.gridLines"
      :key="`t${g.y}`"
      class="chart__axis"
      :x="PAD_L - 6"
      :y="g.y + 3"
      text-anchor="end"
    >
      {{ g.value }}
    </text>

    <polygon v-if="geo.area" class="chart__area" :points="geo.area" />
    <polyline v-if="geo.coords.length > 1" class="chart__line" :points="geo.line" />

    <circle
      v-for="(c, i) in geo.coords"
      v-show="c.value > 0"
      :key="i"
      class="chart__dot"
      :cx="c.x"
      :cy="c.y"
      r="2.8"
    >
      <title>{{ c.label }}：{{ c.value }}{{ unit }}</title>
    </circle>

    <text
      v-for="i in geo.labelIdx"
      :key="`l${i}`"
      class="chart__axis"
      :x="geo.coords[i]?.x ?? 0"
      :y="geo.h - 7"
      text-anchor="middle"
    >
      {{ geo.coords[i]?.label ?? "" }}
    </text>
  </svg>
</template>
