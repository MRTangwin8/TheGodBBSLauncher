<script setup lang="ts">
import { onMounted, ref, watch } from "vue";

// 用皮肤贴图画头像（脸 + 帽子层）或正面全身像。
// 贴图布局：64×64（1.8+，左右手脚分开）或老式 64×32（左侧手脚由右侧镜像）
const props = withDefaults(defineProps<{ src: string; slim?: boolean; mode?: "head" | "body"; size?: number }>(), {
  slim: false,
  mode: "head",
  size: 72,
});

const canvas = ref<HTMLCanvasElement | null>(null);

type Rect = [number, number, number, number];

/**
 * 老式 64×32 皮肤的帽子层经常是整块不透明（多为纯黑），原版游戏会把这种帽子层当作不存在
 * （源码里叫 "Notch transparency hack"），这里照做，否则头像会变成黑方块。
 */
function hatUsable(img: HTMLImageElement): boolean {
  if (img.height !== 32) return true;
  const probe = document.createElement("canvas");
  probe.width = 32;
  probe.height = 16;
  const pctx = probe.getContext("2d")!;
  pctx.drawImage(img, 32, 0, 32, 16, 0, 0, 32, 16);
  const data = pctx.getImageData(0, 0, 32, 16).data;
  for (let i = 3; i < data.length; i += 4) {
    if (data[i] < 128) return true;
  }
  return false;
}

function draw(img: HTMLImageElement) {
  const c = canvas.value;
  if (!c) return;
  const ctx = c.getContext("2d")!;
  ctx.imageSmoothingEnabled = false;
  ctx.clearRect(0, 0, c.width, c.height);
  const legacy = img.height === 32;
  const hat = hatUsable(img);
  const part = ([sx, sy, w, h]: Rect, dx: number, dy: number, mirror = false) => {
    if (mirror) {
      ctx.save();
      ctx.translate(dx + w, dy);
      ctx.scale(-1, 1);
      ctx.drawImage(img, sx, sy, w, h, 0, 0, w, h);
      ctx.restore();
    } else {
      ctx.drawImage(img, sx, sy, w, h, dx, dy, w, h);
    }
  };

  if (props.mode === "head") {
    part([8, 8, 8, 8], 0, 0);
    if (hat) part([40, 8, 8, 8], 0, 0);
    return;
  }
  // 正面全身：16×32
  const aw = props.slim ? 3 : 4;
  const ax = props.slim ? 1 : 0;
  part([8, 8, 8, 8], 4, 0); // 头
  part([20, 20, 8, 12], 4, 8); // 身体
  part([44, 20, aw, 12], ax, 8); // 右臂（画面左侧）
  part([4, 20, 4, 12], 4, 20); // 右腿
  if (legacy) {
    part([44, 20, aw, 12], 12, 8, true);
    part([4, 20, 4, 12], 8, 20, true);
  } else {
    part([36, 52, aw, 12], 12, 8); // 左臂
    part([20, 52, 4, 12], 8, 20); // 左腿
    // 外层（帽子、外套、袖子、裤子）
    part([20, 36, 8, 12], 4, 8);
    part([44, 36, aw, 12], ax, 8);
    part([52, 52, aw, 12], 12, 8);
    part([4, 36, 4, 12], 4, 20);
    part([4, 52, 4, 12], 8, 20);
  }
  if (hat) part([40, 8, 8, 8], 4, 0);
}

function load() {
  const img = new Image();
  img.onload = () => draw(img);
  img.src = props.src;
}

onMounted(load);
watch(() => [props.src, props.slim, props.mode], load);
</script>

<template>
  <canvas
    ref="canvas"
    :width="mode === 'head' ? 8 : 16"
    :height="mode === 'head' ? 8 : 32"
    class="skin"
    :class="mode"
    :style="mode === 'head' ? { width: `${size}px`, height: `${size}px` } : { width: `${size / 2}px`, height: `${size}px` }"
  />
</template>

<style scoped>
.skin { image-rendering: pixelated; display: block; }
.skin.head { border: 2px solid var(--border-strong); border-radius: 2px; }
.skin.body { filter: drop-shadow(0 4px 6px rgba(0, 0, 0, 0.35)); }
</style>
