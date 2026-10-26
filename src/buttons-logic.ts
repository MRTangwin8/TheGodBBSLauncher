// 按钮自定义里不依赖界面的纯逻辑：导航排序、快捷按钮校验。单独成文件，方便直接写测试。
// 校验规则和后端 config/buttons.rs、engine/launch/server.rs 保持一致；后端会再校验一遍，这里是为了让用户当场看到哪里填错了。

export const MAX_QUICK = 8;

/** 不能从导航栏隐藏的页面：隐藏了就进不去自定义和设置，用户没法自己改回来 */
export const ALWAYS_VISIBLE = ["/", "/theme", "/settings"];

/** 按用户设置的顺序排列导航项：设置里列出的在前，没列出的（比如以后新增的页面）按默认顺序排在后面；隐藏的去掉，但必留项除外 */
export function orderNav<T extends { path: string }>(items: T[], order: string[], hidden: string[]): T[] {
  const rank = new Map(order.map((path, i) => [path, i]));
  return items
    .map((item, i) => ({ item, key: rank.get(item.path) ?? order.length + i }))
    .sort((a, b) => a.key - b.key)
    .map((x) => x.item)
    .filter((item) => ALWAYS_VISIBLE.includes(item.path) || !hidden.includes(item.path));
}

/** 设置界面里要显示的完整顺序（含被隐藏的），和 orderNav 的排序规则一致 */
export function fullOrder(defaultPaths: string[], order: string[]): string[] {
  return orderNav(
    defaultPaths.map((path) => ({ path })),
    order,
    [],
  ).map((x) => x.path);
}

/** 把第 index 项上移（-1）或下移（1）一位，返回新数组；越界就原样返回 */
export function moveItem<T>(list: T[], index: number, delta: -1 | 1): T[] {
  const to = index + delta;
  if (index < 0 || index >= list.length || to < 0 || to >= list.length) return list;
  const next = [...list];
  [next[index], next[to]] = [next[to], next[index]];
  return next;
}

/** 服务器地址：主机名、IPv4 或 [IPv6]，可带端口。返回错误说明，合格返回 null；留空算合格（不进服务器） */
export function validateServer(input: string): string | null {
  const s = input.trim();
  if (!s) return null;
  if (s.length > 260) return "服务器地址太长";
  const m = /^(?:\[([0-9a-fA-F:.]+)\]|([A-Za-z0-9_.-]+))(?::(\d{1,5}))?$/.exec(s);
  if (!m) return "服务器地址格式不对，应为 主机名 或 主机名:端口，例如 mc.example.com:25565";
  const host = m[1] ?? m[2];
  if (m[1] ? !m[1].includes(":") : /^[-.]|[-.]$|\.\./.test(host)) return "服务器地址格式不对";
  if (m[3] !== undefined && (Number(m[3]) < 1 || Number(m[3]) > 65535)) return "端口要在 1 到 65535 之间";
  return null;
}

export type QuickActionDraft =
  | { type: "openFolder"; path: string }
  | { type: "openUrl"; url: string }
  | { type: "launch"; versionId: string; server?: string | null }
  | { type: "runProgram"; path: string };

export interface QuickDraft {
  id: string;
  label: string;
  icon: string;
  action: QuickActionDraft;
}

/** 校验一个快捷按钮，返回第一个问题；没问题返回 null */
export function validateQuick(b: QuickDraft): string | null {
  if (!b.label.trim()) return "给按钮起个名字";
  const a = b.action;
  switch (a.type) {
    case "openFolder":
      return a.path.trim() ? null : "选择要打开的文件夹";
    case "runProgram":
      return a.path.trim() ? null : "选择要运行的程序或脚本";
    case "openUrl": {
      try {
        const u = new URL(a.url.trim());
        if (!["http:", "https:"].includes(u.protocol)) return "网址要以 http:// 或 https:// 开头";
        if (u.username || u.password) return "网址里不要带账号密码";
        return null;
      } catch {
        return "网址格式不对，例如 https://www.mcbbs.co/";
      }
    }
    case "launch":
      if (!a.versionId.trim()) return "选择要启动的版本";
      return validateServer(a.server ?? "");
  }
}

/** 按钮动作的一句话说明，用在列表和悬停提示里 */
export function describeQuick(a: QuickActionDraft): string {
  switch (a.type) {
    case "openFolder":
      return `打开文件夹 ${a.path}`;
    case "openUrl":
      return `打开网址 ${a.url}`;
    case "runProgram":
      return `运行 ${a.path}`;
    case "launch":
      return a.server?.trim() ? `启动 ${a.versionId} 并进入 ${a.server.trim()}` : `启动 ${a.versionId}`;
  }
}

/** 给 id 用：同一台电脑上不会重复 */
export function newQuickId(existing: string[]): string {
  let id: string;
  do {
    id = `q${Date.now().toString(36)}${Math.floor(Math.random() * 1296).toString(36)}`;
  } while (existing.includes(id));
  return id;
}
