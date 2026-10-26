# 神权论坛启动器（MCBBS 纪念版）

一个跨平台（Windows / macOS / Linux）的 Minecraft Java 版启动器，界面致敬当年的 MCBBS（我的世界中文论坛）。

> 非官方粉丝纪念作品，与 MCBBS、Mojang Studios、Microsoft 均无关联。界面素材全部原创绘制，没有使用任何官方 Logo 或图片。本程序不分发任何 Minecraft 游戏文件，所有文件都从 Mojang 官方服务器或 BMCLAPI 镜像下载。

## 下载

到 [Releases](../../releases) 页面下载最新版本：

| 系统 | 推荐下载 | 说明 |
|---|---|---|
| Windows 10 / 11 | `*_x64-setup.exe` | 也提供 `.msi`。需要 WebView2，Windows 11 自带 |
| macOS（Apple 芯片） | `*_aarch64.dmg` | |
| macOS（Intel 芯片） | `*_x64.dmg` | 不清楚是哪种芯片：点左上角苹果菜单 →「关于本机」查看 |
| Linux | `*.AppImage` | 也提供 `.deb` / `.rpm`。需要 webkit2gtk 4.1 |

目前处于**预发布阶段**，安装包还没有代码签名：
- **Windows**：SmartScreen 可能拦截，点「更多信息 → 仍要运行」。
- **macOS**：提示"已损坏，无法打开"时，在终端执行 `xattr -cr /Applications/神权论坛启动器.app`。

## 功能

**游戏管理**
- 原版安装：版本列表来自官方源和 BMCLAPI，自动测速选最快的；下载支持 64 线程并发、断点续传、SHA1 校验，失败自动换源重试。
- 加载器一键安装：Fabric、Quilt、Forge、NeoForge。
- Java 管理：自动扫描本机 Java，按游戏版本选择合适的；没有合适的就自动下载 Mojang 官方运行时。
- 版本隔离：每个版本的存档、模组和设置可以互不影响。

**账号**
- 微软正版登录：在浏览器里完成授权，启动器全程接触不到你的密码。
- 离线账号。
- 外置登录：兼容 authlib-injector 的皮肤站，比如 LittleSkin。
- 显示真实皮肤。

**资源与整合包**
- 资源中心：从 Modrinth / CurseForge 搜索模组、资源包、光影，自动安装前置；已安装的资源能识别出来，可以启用、禁用、删除。
- 下载时兼容性检查：检查加载器平台、依赖版本范围，以及模组的 Mixin 注入点在当前游戏版本里是否存在；不兼容的前置会自动换成兼容版本。
- 整合包：在资源中心直接搜索 Modrinth / CurseForge 的整合包，选好版本一键下载导入；也可以去 MCBBS 纪念版找中文整合包。支持导入、导出 Modrinth（`.mrpack`）、CurseForge、MCBBS 三种格式，本地文件拖进窗口就能导入。
- 中文社区资源入口，详见 [资源来源说明](docs/RESOURCES.md)。

**崩溃分析**
- 游戏崩溃后自动分析日志，用中文告诉你可能的原因和解决办法；也可以分析外部日志文件。
- 规则库是 JSON 格式，可以在线更新。

**更新提醒**
- 启动时（可在「设置」里关闭）会向 GitHub 查询有没有新版本，有的话弹窗问你要不要去下载；可以选「以后再说」或「跳过此版本」，也能随时在「设置 → 关于」里手动检查。
- 点「立即更新」会在应用内下载新版本、校验签名、安装并自动重启（设置、账号、游戏都不受影响）；也可以选「前往下载」去发布页自己选安装包。Linux 用 deb / rpm 安装的版本请用后一种方式。

**外观**
- 三种布局：经典论坛（横幅 + 顶部导航）、木板侧栏、完全现代。
- 七套内置配色，外加自定义配色编辑器；深浅色可以跟随系统。
- 自定义背景图；圆角、透明度、毛玻璃、字体等都能细调。
- 外观可以导出成 `.godtheme` 主题文件分享给别人。

## 常见问题

**CurseForge 需要 API Key 吗？**
不需要。不填 Key 时，启动器通过 [MCIM](https://mod.mcimirror.top) 社区镜像访问 CurseForge。如果你有自己的 Key（可在 [console.curseforge.com](https://console.curseforge.com) 免费申请），可以在「设置」里填写，之后改为直连官方接口。有少数模组的作者关闭了第三方下载，这类模组无论填不填 Key 都只能去 CurseForge 网页手动下载。

**我的账号安全吗？**
- 微软账号的密码只在微软自己的网页里输入。外置登录的密码只用来换取令牌，不会保存。
- 登录令牌用 AES-256-GCM 加密后存在本地，加密密钥放在系统钥匙串里（Windows 凭据管理器 / macOS 钥匙串 / Linux Secret Service）。
- 外置登录服务器必须使用 https，否则启动器会拒绝登录，避免密码明文传输。

**检查更新会上传什么？**
只会向 GitHub 的公开接口请求发布列表，GitHub 能看到你的网络地址，除此之外不会上传任何信息。不想联网检查的话，在「设置 → 关于」里关掉「启动时自动检查更新」即可。

**数据存在哪里？**

| 内容 | Windows | macOS | Linux |
|---|---|---|---|
| 默认游戏目录 | `%APPDATA%\GodBBSLauncher\.minecraft` | `~/Library/Application Support/GodBBSLauncher/.minecraft` | `~/.local/share/GodBBSLauncher/.minecraft` |
| 设置与账号 | `%APPDATA%\co.mcbbs.godbbs` | `~/Library/Application Support/co.mcbbs.godbbs` | `~/.config/co.mcbbs.godbbs` |

游戏目录可以在「设置」里修改。

## 从源码构建

需要：
- Rust（通过 [rustup](https://rustup.rs) 安装）、Node.js 20 以上；
- Windows：MSVC Build Tools 和 WebView2；
- Linux：`libwebkit2gtk-4.1-dev` 等依赖，完整列表见 [Tauri 文档](https://tauri.app/start/prerequisites/)。

```bash
npm install
npm run tauri dev      # 启动桌面窗口（开发模式）
npm run tauri build    # 打包安装包
npm run dev            # 只在浏览器里预览界面，使用假数据
```

### 测试

```bash
npm run typecheck && npm test
cd src-tauri
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
```

`src-tauri/examples/` 下有一些命令行冒烟测试，不用打开界面就能验证核心流程：

```bash
cargo run --example install -- 1.20.1 ./.minecraft          # 安装原版
cargo run --example launch -- 1.20.1 ./.minecraft           # 离线启动到主菜单后自动退出
cargo run --example loader -- forge 1.20.1 ./.minecraft     # 安装原版 + 加载器（fabric / quilt / forge / neoforge）
cargo run --example resources -- modrinth "sodium" <版本id> ./.minecraft   # 搜索并安装模组（含前置）
cargo run --example compat -- ./.minecraft <版本id> mods/*.jar             # 检查模组兼容性
cargo run --example analyze -- crash-reports/crash-xxx.txt  # 分析崩溃报告
cargo run --example modpack -- import pack.mrpack ./.minecraft 我的整合包   # 导入整合包
```

### 自己发布修改版

微软登录使用的 Azure 应用 Client ID 属于本项目（`src-tauri/src/engine/auth/microsoft.rs` 中的 `CLIENT_ID`）。如果你要发布修改版，请[注册自己的 Azure 应用](https://learn.microsoft.com/entra/identity-platform/quickstart-register-app)，向 Mojang 申请 Minecraft API 权限，然后替换掉这个 ID。

## 参与贡献

欢迎提 Issue 和 Pull Request。

- 开始之前请先读 [AGENTS.md](AGENTS.md)，里面有架构说明、跨层约定和几条红线。
- 每个 PR 都会在三个平台上自动构建和测试，请保证 CI 通过。
- 界面文案使用简体中文；新增素材必须是原创的。

## 项目结构

```
src-tauri/src/
  engine/      核心逻辑，和 Tauri 无关：下载、安装、启动、账号、加载器、资源、整合包、崩溃分析、主题
  commands/    提供给前端调用的接口
  config/      全局设置
src/
  layouts/     三种布局外壳
  views/       页面
  components/  通用组件
  stores/      状态管理（Pinia）
  api/         前后端通信，以及浏览器预览用的假数据
  themes/      配色与布局样式
```

## 致谢

- [BMCLAPI](https://bmclapidoc.bangbang93.com)（bangbang93）：国内下载镜像
- [MCIM](https://mod.mcimirror.top)：Modrinth / CurseForge 镜像
- [authlib-injector](https://github.com/yushijinhun/authlib-injector)：外置登录
- [Modrinth](https://modrinth.com) 与 [CurseForge](https://www.curseforge.com)：资源平台
- [Tauri](https://tauri.app)、[Vue](https://vuejs.org)
- 以及所有曾经在 MCBBS 发过帖、回过帖的人

## 许可证

代码以 [MIT 许可证](LICENSE) 开源。"MCBBS""我的世界中文论坛"等名称不在许可范围内；Minecraft 是 Mojang Studios 的商标，本项目与其无关。
