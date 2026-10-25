# AGENTS.md：神权论坛启动器协作手册

写给并行开发本项目的 Agent 和人看：读完它就能接手任意模块，并且和别人不打架。
项目简介和命令行示例见 [README.md](README.md)；当前状态、已知限制和路线图见 [docs/STATUS.md](docs/STATUS.md)。

---

## 1. 一句话概览

跨平台 Minecraft Java 版启动器，技术栈是 **Tauri 2 + Vue 3 + TypeScript + Rust**，界面致敬 MCBBS 论坛。

- **Rust 后端**：分两层。`engine/` 是和 Tauri 无关的纯逻辑，可以写单测和 example；`commands/` 是暴露给前端的 IPC 薄封装。
- **Vue 前端**：只通过 `src/api/index.ts` 和后端通信。浏览器里没有 Tauri，这时自动改用 `src/api/mock.ts` 的假数据，所以 `npm run dev` 能单独预览界面。

## 2. 红线（任何改动都必须遵守）

1. **不分发游戏文件**：所有 Minecraft 文件只从 Mojang 官方源或 BMCLAPI 镜像下载，仓库和安装包里不放任何游戏资源。
2. **素材全部原创**：图标（`PixelIcon` / `ModernIcon`）、背景、Logo 一律用 CSS 或 SVG 自己画。不用 MCBBS 官方 Logo 或图片，也不用其他启动器（HMCL、PCL、BakaXL 等）的素材和名字。界面文案里不提其他启动器。
3. **凭据安全**：
   - 账号令牌经 AES-GCM 加密存在 `accounts.json`，密钥在系统钥匙串，见 `engine/auth/vault.rs`。
   - 不要打印令牌，不要把令牌写进日志，也不要把令牌传给前端。
   - CurseForge Key 由用户自己在设置里填写，不要写进代码或提交。
4. **快捷键屏蔽**：`src/shortcuts.ts` 屏蔽了 F12、Ctrl+P、Ctrl+R 等浏览器行为，新增功能不要依赖这些按键。
5. **界面文案用简体中文**，可以带一点论坛味（"神权""版主提示"），但要说清楚。
6. **只在维护者明确要求时提交**。提交信息用中文，一行概括，例如 `资源中心：……`。

## 3. 目录与职责

```
src-tauri/src/
  lib.rs              注册插件、AppState、所有 command（★ 冲突热点）
  config/mod.rs       全局 Settings（★ 冲突热点：加字段要同步前端）
  commands/           IPC 层，每个文件对应一个领域
    settings / versions / accounts / loaders / java / game / resources / modpack / crash
  engine/             纯逻辑，不依赖 Tauri
    meta/             版本清单、version.json 解析、平台规则、inheritsFrom 合并
    download/         并发下载器（断点续传、SHA1、重试、镜像回退）
    mirror/           官方 / BMCLAPI 改写与测速
    install/          原版安装、已安装版本列表（InstalledVersion）
    instance.rs       每个版本的实例配置（游玩时长、最后游玩、整合包来源）
    java/             扫描本机 Java、下载 Mojang runtime
    launch/           拼启动参数、natives、log4j 日志解析
    loader/           Fabric / Quilt（meta profile）、Forge / NeoForge（安装器 processors）
    auth/             微软设备码、离线、外置登录（authlib-injector）、皮肤、加密存储
    resources/        Modrinth / CurseForge 搜索、依赖解析、兼容性检查（classfile / Mixin）
    modpack/          mrpack / CurseForge / MCBBS 整合包导入导出
    crash/            崩溃分析：rules.json 规则库、测试样例（fixtures）
  examples/           命令行冒烟测试（install / launch / loader / resources / compat / analyze / modpack）

src/
  main.ts             挂载应用，引入主题 CSS，安装快捷键屏蔽
  App.vue             布局切换器 + 全局浮层（进度、整合包导入、拖放导入）
  appearance.ts       布局 / 配色 / 深浅色 / 背景图 → <html data-layout data-palette data-theme>
  router.ts           navItems：导航项和路由的唯一来源（★ 冲突热点）
  shortcuts.ts        屏蔽浏览器快捷键
  api/
    index.ts          所有 IPC 调用（★ 冲突热点）
    types.ts          前后端共享的类型（★ 冲突热点，必须和 Rust 的 serde 结构一致）
    mock.ts           浏览器预览用的假数据（★ 冲突热点）
  layouts/            三种外壳：ClassicShell（默认）/ SidebarShell / ModernShell
  views/              每个路由一个页面
  components/         通用组件；home/ 下是首页的两种形态（ForumHome / LaunchHome / QuickLaunch）
  composables/        useLauncher：首页启动逻辑，两种首页共用
  stores/             Pinia：settings / versions / accounts / game / modpack
  themes/
    mcbbs-classic.css 基础变量（默认 MCBBS 配色）和全局组件样式（.box、.btn、.tag……）
    palettes.css      其余配色：grass / nether / end / ocean / cherry / stone，各有深浅两套
    layouts.css       按布局覆盖变量：classic 直角不透明，modern 中性亚克力
```

## 4. 跨层约定

### 新增一个后端能力（command）
1. 逻辑写在 `engine/<领域>/`，能测就写 `#[cfg(test)]`，必要时加一个 `examples/`。
2. 在 `commands/<领域>.rs` 加 `#[tauri::command]`。参数和返回值用 `#[serde(rename_all = "camelCase")]`。错误统一转成 `String`，内容写中文、给出可操作的提示。
3. 在 `lib.rs` 的 `generate_handler!` 里注册。
4. 前端三处同步：
   - `api/types.ts` 加类型；
   - `api/index.ts` 加 `call<T>("snake_name", { camelArgs })`；
   - `api/mock.ts` 加假数据，否则浏览器预览会报错。
5. 需要新权限时（窗口、对话框等）改 `src-tauri/capabilities/default.json`。

### 长任务与进度
- 用事件推送：`install-progress`、`game-log`、`game-exit`。前端用 `api/index.ts` 导出的 `onInstallProgress` / `onGameLog` / `onGameExit` 订阅；新事件照此加一个导出。
- 下载任务带 `jobId`，进度显示在 `ProgressFloat`。

### Settings 加字段（四处都要改）
1. `src-tauri/src/config/mod.rs`：字段要带 `#[serde(default)]` 或默认值函数，这样旧的 settings.json 才能读。
2. `src/api/types.ts` 的 `Settings`。
3. `src/api/mock.ts` 的默认设置。
4. 用到它的界面。保存用 `useSettingsStore().save({...settings, ...patch})`；即时生效的开关要先本地赋值再保存，参考 `ThemeView.patch`。

### 新页面
在 `router.ts` 的 `navItems` 加一项，三种布局的导航会自动出现。`icon` 要在 `PixelIcon` 和 `ModernIcon` 里都有同名图标。

### 外观系统
- 颜色只用 CSS 变量，不写死色值（迷你预览图除外）。常用变量：
  - 表面：`--bg-box`、`--bg-box-solid`、`--bg-box-alt`、`--bg-title`、`--bg-input`
  - 文字：`--text`、`--text-muted`、`--link`
  - 强调色：`--grass`、`--grass-light`、`--grass-dark`、`--gold`、`--red`
  - 木头边框：`--wood`、`--wood-dark`、`--wood-light`；标题色：`--brown-dark`
  - 圆角：`--radius`、`--radius-sm`
- 新配色：在 `palettes.css` 加 `:root[data-palette="x"]` 和 `[data-theme="dark"]` 两套，再把名字加进三处：`appearance.ts` 的 `PALETTES`、`ThemeView.vue` 的 `palettes`、`engine/theme.rs` 的 `PALETTES`。
- 自定义配色（`palette = "custom"`）：
  - 只存四个基础色（`Settings.customPalette`）；
  - `colors.ts` 推算整套变量，包括深色版本；
  - `appearance.ts` 把结果写成 `<html>` 上的内联变量，优先级高于所有样式表；
  - 现代布局只写强调色。
- 外观细调（`Settings.tweaks`）也走内联变量：`--radius`、`--bg-box`、`--blur`、`--bg-filter`、`--bg-dim`、`--font-size`、`--font`；关掉动画时设 `data-motion="off"`。
- 主题文件 `.godtheme`：
  - 格式由 `engine/theme.rs` 定义，是 JSON，背景图用 base64 内嵌；
  - 导入时清洗所有字段，因为主题文件来自别人，不可信；
  - 新增可导出的外观字段时，要同时改 `ThemeFile`、`apply` 和清洗函数。
- 新布局：新建 `layouts/XxxShell.vue`，然后：
  - 加进 `appearance.ts` 的 `LAYOUTS`、`App.vue` 的映射、`ThemeView.vue` 的 `layouts`；
  - 改 `types.ts` 里 `Settings.layout` 的联合类型；
  - 需要的话在 `layouts.css` 加覆盖。
- 布局外壳负责四件事：
  - 背景，用 `<BackgroundLayer kind=…>`；
  - 标题栏：窗口没有系统边框，拖动区加 `data-tauri-drag-region`（只有带这个属性的元素本身能拖），右上角放 `<WindowControls>`；
  - 导航；
  - `<RouterView>`：必须包一层 `<div :key="r.path" class="page">` 再放进 `<Transition mode="out-in">`，否则多根页面会卡住不切换。
- 背景层 `z-index:-1`，所以底色只能放在 `html` 上，`body` 不能有背景。

## 5. 并行分工：怎样不打架

各领域彼此独立，可以同时开工：

| 领域 | 主要文件 | 备注 |
|---|---|---|
| 下载 / 安装 / 镜像 | `engine/download, mirror, install, meta` | 影响面大，改完跑 `cargo test` 和 `examples/install` |
| 启动 / Java | `engine/launch, java`，`commands/game.rs, java.rs` | 用 `examples/launch` 冒烟 |
| 加载器 | `engine/loader`，`commands/loaders.rs` | 用 `examples/loader` |
| 账号 / 皮肤 | `engine/auth`，`commands/accounts.rs`，`AccountView`，`AccountAvatar`，`SkinView` | 注意红线 3 |
| 资源中心 | `engine/resources`，`commands/resources.rs`，`ResourcesView`，`ResourceDialog` | |
| 整合包 | `engine/modpack`，`commands/modpack.rs`，`Modpack*Dialog` | |
| 崩溃分析 | `engine/crash`（`rules.json` + fixtures），`CrashPanel` | 新规则要配一个 fixture 测试 |
| 外观 / 布局 | `layouts/`，`themes/`，`appearance.ts`，`ThemeView`，`BackgroundLayer`，图标组件 | 纯前端 |
| 首页 | `components/home/`，`composables/useLauncher.ts` | |

**★ 冲突热点**：`lib.rs`、`config/mod.rs`、`api/index.ts`、`api/types.ts`、`api/mock.ts`、`router.ts`、`main.ts`。
- 只**追加**，不要重排或格式化别人的行。
- 每次改动尽量小，提交前先 rebase 或合并最新代码。
- 两个 Agent 都要往热点文件加东西时，按领域分段追加（各自放在本领域已有条目的末尾）。

## 6. 验证清单（提交前）

```bash
npm run typecheck                      # 前端类型检查（vue-tsc），必须零错误
cd src-tauri && cargo clippy && cargo test
```
- 改了界面：
  - 用 `npm run dev` 打开 http://localhost:1420 检查，三种布局 × 深浅色都看一眼（在主题中心切换）。
  - 浏览器预览里背景的 transition 可能被节流，截图会是旧画面。可以先注入 `*{transition:none!important}`，或者直接查 DOM。
- 改了后端：在 `npm run tauri dev` 的真实窗口里跑一遍对应流程。

### 打包与发布
- `main` 上每次推送都会在三个平台上构建，产物在 Actions 里保留 14 天。推 `v*` 标签（如 `v0.1.0`）会额外创建一个 GitHub Release，类型为预发布。
- 产品名是中文，打包时有几个坑：
  - Windows MSI 必须设 `wix.language = zh-CN`，否则 WiX 默认的 1252 代码页装不下中文；
  - Linux 的 rpm 包名不能以中文开头，所以 `tauri.linux.conf.json` 把 productName 换成英文，菜单里显示的中文名由 `linux/app.desktop` 提供；
  - Release 附件名不支持中文，workflow 会把文件名里的中文产品名换成 `GodBBSLauncher`。
- 版本号要同时改 `tauri.conf.json`、`Cargo.toml`、`package.json` 三处，再打同名标签。

### Windows 环境注意事项
- Git Bash 里可能找不到 cargo，先 `export PATH="$HOME/.cargo/bin:$PATH"`。
- 杀毒或索引会锁文件，导致 cargo 偶尔报 `os error 32` 或 `can't find crate`。重试即可，也可以用 `-j 2` 降低并发。
- 重新编译前先关掉正在运行的 `godbbs-launcher.exe`，否则 exe 被占用。
- 测试启动游戏后记得关掉自己拉起的 java 进程，别留在用户机器上。
- 写含中文的文件优先用编辑工具；Bash heredoc 里的 Python 可能把中文搞乱，用 `python -X utf8` 跑脚本文件更稳。

## 7. 当前状态（2026-10-25）

已完成（详见 README 进度）：
- 原版、Java、账号、启动；
- 加载器、崩溃分析；
- 资源中心（含下载时兼容性检查）、整合包导入导出。

界面：
- 无边框窗口和自绘标题栏，快捷键屏蔽，真实皮肤显示；
- 主题中心：
  - 三种布局：经典论坛（默认）、木板侧栏（可收起）、完全现代；
  - 七套配色加自定义配色编辑器，深浅色可跟随系统，自定义背景图；
  - 预设主题和外观细调：圆角、透明度、毛玻璃、背景模糊和压暗、字号、字体、动画；
  - `.godtheme` 主题导入导出，主题文件可以直接拖进窗口；
- 首页：经典布局带快速启动，侧栏和现代布局用启动器式首页；
- 游玩时长统计、记住上次启动的版本。

待办候选（以 PLAN.md 为准）：插件系统、自动更新、实例存档管理、服务器列表等。
