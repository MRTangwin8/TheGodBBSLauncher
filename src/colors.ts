// 自定义配色：由四个基础色推算出整套 CSS 变量（含深色版本）
import type { CustomPalette } from "./api/types";

type Rgb = [number, number, number];

function parse(hex: string): Rgb {
  const m = /^#?([0-9a-f]{6})$/i.exec(hex.trim());
  const n = m ? parseInt(m[1], 16) : 0;
  return [(n >> 16) & 255, (n >> 8) & 255, n & 255];
}

function toHex([r, g, b]: Rgb): string {
  return "#" + [r, g, b].map((v) => Math.round(Math.min(255, Math.max(0, v))).toString(16).padStart(2, "0")).join("");
}

/** a 和 b 混合，t=0 全是 a，t=1 全是 b */
export function mix(a: string, b: string, t: number): string {
  const x = parse(a);
  const y = parse(b);
  return toHex([0, 1, 2].map((i) => x[i] + (y[i] - x[i]) * t) as Rgb);
}

function rgba(hex: string, alpha: number): string {
  const [r, g, b] = parse(hex);
  return `rgba(${r}, ${g}, ${b}, ${alpha})`;
}

function luminance(hex: string): number {
  const lin = parse(hex).map((v) => {
    const c = v / 255;
    return c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
  });
  return 0.2126 * lin[0] + 0.7152 * lin[1] + 0.0722 * lin[2];
}

/** WCAG 对比度，1~21；正文至少 4.5 才算看得清 */
export function contrast(a: string, b: string): number {
  const [x, y] = [luminance(a), luminance(b)].sort((m, n) => n - m);
  return (x + 0.05) / (y + 0.05);
}

/** 强调色相关的变量；现代布局只用这几个 */
export function accentVars(p: CustomPalette, dark: boolean): Record<string, string> {
  const a = p.accent;
  return {
    "--grass": a,
    "--grass-light": mix(a, "#ffffff", dark ? 0.2 : 0.25),
    "--grass-dark": mix(a, "#000000", dark ? 0.35 : 0.3),
    "--grass-top": a,
    "--link": dark ? mix(a, "#ffffff", 0.4) : mix(a, p.text, 0.35),
  };
}

/** 完整的配色变量 */
export function paletteVars(p: CustomPalette, dark: boolean): Record<string, string> {
  const { surface: s, text: t, wood: w } = p;
  if (!dark) {
    return {
      ...accentVars(p, false),
      "--sky-top": mix("#8ec5f0", p.accent, 0.15),
      "--sky-bottom": mix("#dff0fb", s, 0.4),
      "--bg-box-solid": s,
      "--bg-box": rgba(s, 0.9),
      "--bg-box-alt": rgba(mix(s, w, 0.08), 0.75),
      "--bg-title": rgba(mix(s, w, 0.2), 0.9),
      "--bg-input": mix(s, "#ffffff", 0.6),
      "--border": rgba(w, 0.35),
      "--border-strong": mix(w, s, 0.3),
      "--text": t,
      "--text-muted": mix(t, s, 0.45),
      "--brown": w,
      "--brown-dark": mix(w, t, 0.4),
      "--wood": w,
      "--wood-dark": mix(w, "#000000", 0.3),
      "--wood-light": mix(w, "#ffffff", 0.2),
      "--page-bg": mix(s, w, 0.22),
      "--page-dot": mix(s, w, 0.3),
    };
  }
  // 深色：底色用木头色压暗，文字用浅色的卡片底色
  const solid = mix(mix(s, w, 0.5), "#000000", 0.82);
  const text = mix(s, "#ffffff", 0.3);
  return {
    ...accentVars(p, true),
    "--sky-top": mix("#0f1a2b", p.accent, 0.1),
    "--sky-bottom": mix("#24324a", w, 0.15),
    "--bg-box-solid": solid,
    "--bg-box": rgba(solid, 0.88),
    "--bg-box-alt": rgba(mix(solid, "#ffffff", 0.06), 0.75),
    "--bg-title": rgba(mix(solid, "#ffffff", 0.1), 0.92),
    "--bg-input": mix(solid, "#000000", 0.2),
    "--border": rgba(mix(w, "#ffffff", 0.25), 0.45),
    "--border-strong": mix(w, "#ffffff", 0.1),
    "--text": text,
    "--text-muted": mix(text, solid, 0.4),
    "--brown": mix(w, "#ffffff", 0.3),
    "--brown-dark": mix(w, "#ffffff", 0.65),
    "--wood": mix(w, "#000000", 0.45),
    "--wood-dark": mix(w, "#000000", 0.62),
    "--wood-light": mix(w, "#000000", 0.25),
    "--page-bg": mix(solid, "#000000", 0.3),
    "--page-dot": mix(solid, "#000000", 0.15),
  };
}
