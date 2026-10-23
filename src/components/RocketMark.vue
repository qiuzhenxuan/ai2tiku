<script setup lang="ts">
import AppIcon from "./AppIcon.vue";

// Evenly spaced emissions with deterministic lanes. Negative delays fill the plume
// immediately; only transform/opacity animate, and hidden particles stay paused.
const particles = Array.from({ length: 72 }, (_, i) => ({
  '--dx': `${(((i * 37) % 71) / 35 - 1) * 3.1}px`,
  '--dy': `${11 + (i % 5) * 0.5}px`,
  '--delay': `${-(i / 72) * (440 + (i % 4) * 40)}ms`,
  '--life': `${440 + (i % 4) * 40}ms`,
  '--size': `${0.7 + (i % 3) * 0.18}px`,
}));
</script>

<template>
  <span class="rocket-mark" aria-hidden="true">
    <AppIcon name="rocket" :size="20" />
    <span class="rocket-mark__exhaust">
      <span class="rocket-mark__plume" />
      <span class="rocket-mark__core" />
      <i v-for="(style, i) in particles" :key="i" :style="style" />
    </span>
  </span>
</template>

<style>
.rocket-mark {
  position: relative;
  display: block;
  width: 20px;
  height: 20px;
  transform: translateY(0);
  transition: transform 1350ms cubic-bezier(.22, .61, .36, 1);
}
.rail__item--rocket:is(:hover, :focus-visible) .rocket-mark {
  transform: translateY(-4px);
}
.rocket-mark__exhaust {
  position: absolute;
  top: 79.1667%; /* Same (12, 19) nozzle origin as the upright 24x24 SVG. */
  left: 50%;
  width: 0;
  height: 0;
  opacity: 0;
  pointer-events: none;
  color: var(--text);
  transition: opacity 180ms ease-out;
}
.rail__item--rocket:is(:hover, :focus-visible) .rocket-mark__exhaust { opacity: 1; }
.rocket-mark__plume, .rocket-mark__core {
  position: absolute;
  top: 0;
  left: 0;
  transform: translateX(-50%);
  border-radius: 0 0 50% 50%;
  clip-path: polygon(25% 0, 75% 0, 100% 28%, 70% 65%, 50% 100%, 30% 65%, 0 28%);
  background: linear-gradient(to bottom, currentColor 0%, transparent 100%);
}
.rocket-mark__plume { width: 7px; height: 13px; opacity: .26; }
.rocket-mark__core { width: 3px; height: 11px; opacity: .8; }
.rocket-mark__exhaust i {
  position: absolute;
  top: 0;
  left: 0;
  width: var(--size);
  height: calc(var(--size) * 1.9);
  border-radius: 50%;
  background: currentColor;
  opacity: 0;
  animation: exhaust-particle var(--life) cubic-bezier(.35, 0, .75, 1) var(--delay) infinite;
  animation-play-state: paused;
}
.rail__item--rocket:is(:hover, :focus-visible) .rocket-mark__exhaust i { animation-play-state: running; }
@keyframes exhaust-particle {
  0% { transform: translate(-50%, 0) scale(.5); opacity: 0; }
  10% { opacity: .7; }
  50% { opacity: .38; }
  100% { transform: translate(calc(-50% + var(--dx)), var(--dy)) scale(.18); opacity: 0; }
}
@media (prefers-reduced-motion: reduce) {
  .rocket-mark { transition: none; transform: none !important; }
  .rocket-mark__exhaust { display: none; }
  .rocket-mark__exhaust i { animation: none; }
}
</style>
