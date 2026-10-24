# 神权论坛启动器（MCBBS 纪念版）

跨平台（Windows / macOS / Linux）Minecraft Java 版启动器，界面致敬 MCBBS 经典论坛风格。

> 非官方粉丝纪念作品，与 MCBBS / Mojang Studios / Microsoft 无关。界面素材全部原创；本程序不分发任何 Minecraft 游戏文件，全部从官方源或 BMCLAPI 镜像下载。

## 当前进度

- [x] Tauri 2 + Vue 3 骨架、MCBBS 经典主题
- [x] 版本清单（官方 / BMCLAPI，自动测速）
- [x] version.json 解析：新旧参数格式、规则过滤、inheritsFrom 合并
- [x] 并发下载器：64 并发、断点续传、SHA1 校验、重试 + 镜像回退
- [x] 原版安装（client.jar / 库 / natives / 资源 / 日志配置）
- [x] Java 管理：扫描本机 Java，缺少时自动下载 Mojang 官方 runtime
- [x] 启动游戏：natives 解压、新旧参数格式、旧版资源映射、版本隔离、实时日志
- [x] 账号：微软正版（设备码）、离线、外置登录（authlib-injector），令牌 AES-GCM 加密、密钥存系统钥匙串
- [x] 模组加载器一键安装：Fabric / Quilt（meta profile）、Forge / NeoForge（本地执行安装器 processors，旧版 Forge 走 versionInfo）
- [x] 崩溃分析：JSON 规则库（中英文日志）+ 中文诊断，崩溃后自动分析，也可分析外部日志文件，规则可在线热更新
- [x] 资源中心：Modrinth / CurseForge 搜索模组、资源包、光影，自动解析前置，已装识别（哈希），启用/禁用/删除
- [x] 下载时兼容性检查：平台、依赖版本范围、Mixin 注入点是否存在于当前游戏 jar（含 Jar-in-Jar），不兼容的前置自动换成兼容版本
- [ ] 整合包导入导出、插件系统、自动更新……见 PLAN

## 开发

需要 Rust（rustup）、Node 20+；Windows 需要 MSVC Build Tools 和 WebView2，Linux 需要 webkit2gtk。

```bash
npm install
npm run tauri dev      # 桌面窗口
npm run dev            # 仅浏览器预览界面（假数据）
```

```bash
cd src-tauri
cargo test                                             # 单元测试
cargo run --example install -- 1.20.1 ./.minecraft     # 命令行安装原版（冒烟测试）
cargo run --example launch -- 1.20.1 ./.minecraft      # 离线启动到主菜单后自动退出
cargo run --example loader -- forge 1.20.1 ./.minecraft # 安装原版 + 加载器（fabric/quilt/forge/neoforge）
cargo run --example resources -- modrinth "sodium extra" <版本id> ./.minecraft # 搜索并安装模组（含前置）
cargo run --example compat -- ./.minecraft <版本id> mods/*.jar   # 检查模组兼容性
cargo run --example analyze -- crash-reports/crash-xxx.txt      # 分析崩溃报告
```

## 目录

```
src-tauri/src/
  engine/        和 Tauri 无关的核心逻辑（避免和 Rust 内置的 core crate 重名）
    meta/        版本清单、version.json、平台规则
    download/    并发下载器
    mirror/      官方 / BMCLAPI 源改写与测速
    install/     原版安装流程
  commands/      前端 IPC（command 请求响应，event 推进度）
  config/        全局设置
src/
  views/ components/ stores/ api/ themes/
```
