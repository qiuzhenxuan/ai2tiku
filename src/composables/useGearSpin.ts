import { onUnmounted, ref } from "vue";
import { useToast } from "./useToast";

/**
 * 彩蛋：连续点击设置齿轮会越转越快。
 * - 每次点击给一次角速度冲量，带摩擦慢慢停下；
 * - 停下前被弹簧拉回最近的齿位（每 45°），会来回晃几下；
 * - 连点 5 下起每下反向，“转来转去”；连点 10 下弹出提示。
 */
const COMBO_WINDOW = 650;      // 两次点击间隔小于此值算连击（ms）
const FRICTION = 2.4;          // 角速度衰减系数（越大停得越快）
const SNAP = 45;               // 齿位间隔（度）
const SPRING = 55;             // 回到齿位的弹簧刚度
const DAMPING = 7;             // 回弹阻尼（越小晃得越久）

export function useGearSpin() {
  const toast = useToast();
  const angle = ref(0);
  let velocity = 0;            // 度 / 秒
  let combo = 0;
  let lastClick = 0;
  let direction = 1;
  let raf = 0;
  let last = 0;

  const reduced = () => window.matchMedia?.("(prefers-reduced-motion: reduce)").matches;

  function step(now: number) {
    const dt = Math.min(0.05, (now - last) / 1000);
    last = now;
    const target = Math.round(angle.value / SNAP) * SNAP;
    if (Math.abs(velocity) > 120) {
      // 自由旋转：只有摩擦
      velocity *= Math.exp(-FRICTION * dt);
    } else {
      // 慢下来后被拉回最近的齿位，产生来回晃动
      velocity += ((target - angle.value) * SPRING - velocity * DAMPING) * dt;
    }
    angle.value += velocity * dt;
    if (Math.abs(target - angle.value) < 0.15 && Math.abs(velocity) < 2) {
      // 归一化到 0–360，避免数值无限增长
      angle.value = ((target % 360) + 360) % 360;
      velocity = 0;
      raf = 0;
      return;
    }
    raf = requestAnimationFrame(step);
  }

  function spin() {
    if (reduced()) return;
    const now = performance.now();
    combo = now - lastClick < COMBO_WINDOW ? combo + 1 : 1;
    lastClick = now;
    if (combo >= 5) direction = -direction;
    // 保留部分已有转速（反向时只留 35%，让换向更利落），再加一次冲量，连击越多冲量越大
    const carry = Math.abs(velocity) * (combo >= 5 ? 0.35 : 1);
    velocity = direction * (carry + 520 + combo * 140);
    if (combo === 10) toast.info("齿轮：别转了，我晕了 @_@");
    if (!raf) {
      last = now;
      raf = requestAnimationFrame(step);
    }
  }

  onUnmounted(() => cancelAnimationFrame(raf));
  return { angle, spin };
}
