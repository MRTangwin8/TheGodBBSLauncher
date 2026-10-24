# 神权论坛启动器（MCBBS 纪念版）

跨平台（Windows / macOS / Linux）Minecraft Java 版启动器，界面致敬 MCBBS 经典论坛风格。

> 非官方粉丝纪念作品，与 MCBBS / Mojang Studios / Microsoft 无关。界面素材全部原创；本程序不分发任何 Minecraft 游戏文件，全部从官方源或 BMCLAPI 镜像下载。

## 当前进度

- [x] Tauri 2 + Vue 3 骨架、MCBBS 经典主题
- [x] 版本清单（官方 / BMCLAPI，自动测速）
- [x] version.json 解析：新旧参数格式、规则过滤、inheritsFrom 合并
- [x] 并发下载器：64 并发、断点续传、SHA1 校验、重试 + 镜像回退
- [x] 原版安装（client.jar / 库 / natives / 资源 / 日志配置）
- [ ] Java 管理、启动游戏、账号、模组加载器……见 PLAN

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
