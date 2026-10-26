// 按钮自定义的界面侧：启动按钮的文字 / 样式 / 颜色、排好序的导航栏、启动提示音。
// 设置存在 settings.buttons 里（不进主题文件）；纯逻辑在 buttons-logic.ts。
import { computed } from "vue";
import type { Buttons } from "./api/types";
import { orderNav } from "./buttons-logic";
import { readableText } from "./colors";
import { navItems } from "./router";
import { useSettingsStore } from "./stores/settings";

export const DEFAULT_BUTTONS: Buttons = {
  launchLabel: "",
  launchStyle: "stone",
  launchColor: "",
  launchSound: false,
  launchShortcut: true,
  navOrder: [],
  navHidden: [],
  quick: [],
};

export function useButtons() {
  const store = useSettingsStore();
  const buttons = computed<Buttons>(() => ({ ...DEFAULT_BUTTONS, ...store.settings?.buttons }));

  /** 导航栏：按用户的顺序，去掉隐藏的（首页、主题中心、设置不能隐藏） */
  const nav = computed(() => orderNav(navItems, buttons.value.navOrder, buttons.value.navHidden));

  const launchLabel = computed(() => buttons.value.launchLabel.trim() || "开始游戏");
  /** 默认「石头 + 跟随配色」不加任何覆盖，样式见 themes/buttons.css */
  const launchClass = computed(() => ["ui-launch", `ui-${buttons.value.launchStyle}`, buttons.value.launchColor ? "ui-colored" : ""]);
  const launchStyle = computed(() => {
    const color = buttons.value.launchColor;
    if (!color) return undefined;
    const fg = readableText(color);
    return { "--launch-color": color, "--launch-fg": fg, "--launch-shadow": fg === "#ffffff" ? "2px 2px 0 rgba(0, 0, 0, 0.35)" : "none" };
  });

  return { buttons, nav, launchLabel, launchClass, launchStyle };
}

let audio: AudioContext | undefined;

/** 点启动时的一声提示音：用 Web Audio 合成两个短音，不带任何音频文件 */
export function playLaunchSound() {
  try {
    audio ??= new AudioContext();
    const now = audio.currentTime;
    [660, 990].forEach((freq, i) => {
      const osc = audio!.createOscillator();
      const gain = audio!.createGain();
      const start = now + i * 0.09;
      osc.type = "square";
      osc.frequency.value = freq;
      gain.gain.setValueAtTime(0.0001, start);
      gain.gain.exponentialRampToValueAtTime(0.05, start + 0.01);
      gain.gain.exponentialRampToValueAtTime(0.0001, start + 0.12);
      osc.connect(gain).connect(audio!.destination);
      osc.start(start);
      osc.stop(start + 0.13);
    });
  } catch {
    // 没有音频设备或被系统静音就算了，不影响启动
  }
}
