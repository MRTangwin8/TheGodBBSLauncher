# 隐私说明 / Privacy Policy

[中文](#中文) · [English](#english)

更新日期 / Last updated：2026-10-28

---

## 中文

神权论坛启动器（下称"启动器"）是一个开源的 Minecraft Java 版启动器。**开发者不运营任何服务器，不收集、不接收、不出售任何用户数据**，启动器里也没有统计、遥测或广告代码。

### 数据存在哪里

所有数据只保存在你自己的电脑上：

- **设置**：`settings.json`，在系统的应用数据目录里。
- **账号**：`accounts.json`。登录令牌经 AES-GCM 加密后保存，密钥放在系统钥匙串（Windows 凭据管理器、macOS 钥匙串、Linux Secret Service）里；钥匙串不可用时退回到配置目录里的密钥文件。
- **游戏文件**：游戏目录（默认在启动器所在目录的 `.minecraft`），包括存档、模组、截图、服务器列表。
- **实例配置、存档备份、下载缓存**：游戏目录或应用数据目录里。

卸载启动器不会删除游戏目录，你可以随时手动删除上面这些文件。

### 启动器会连接哪些服务

启动器只在你使用对应功能时，直接连接下面这些第三方服务；连接时这些服务能看到你的 IP 地址和启动器的版本号（请求头 `User-Agent: GodBBSLauncher/版本号`）。各服务如何处理数据，以它们自己的隐私政策为准。

| 用途 | 服务 | 什么时候连 |
|---|---|---|
| 下载游戏、版本列表、资源文件 | Mojang 官方服务器（`*.mojang.com`、`libraries.minecraft.net`、`resources.download.minecraft.net`）、BMCLAPI 镜像（`bmclapi2.bangbang93.com`） | 安装或启动游戏时；下载源可在设置里选择，自动模式会对两边测速 |
| 安装模组加载器 | Fabric、Quilt、Forge、NeoForge 的官方元数据和 Maven 仓库 | 安装加载器时 |
| 下载 Java | Mojang 官方 Java 运行时；官方没有的平台用 Azul（`api.azul.com`、`cdn.azul.com`） | 找不到合适的 Java、需要自动下载时 |
| 微软账号登录 | Microsoft（`login.microsoftonline.com`）、Xbox Live、Minecraft 服务（`api.minecraftservices.com`） | 添加或刷新微软账号、启动游戏时 |
| 外置登录 | 你自己填写的认证服务器（如 LittleSkin），以及 `authlib-injector.yushi.moe` | 只在你添加了外置登录账号时 |
| 皮肤显示 | Mojang（`sessionserver.mojang.com`、`textures.minecraft.net`）或你的外置登录服务器 | 显示账号皮肤时 |
| 资源中心、模组更新检查 | Modrinth（`api.modrinth.com`、`cdn.modrinth.com`）、CurseForge（填了 API Key 时用 `api.curseforge.com`，否则用 MCIM 镜像 `mod.mcimirror.top`）、苦力怕论坛资源站（`hub.klpbbs.com`） | 搜索、下载资源或检查更新时。检查更新会把本地模组文件的哈希值（SHA1 / CurseForge 指纹）发给这些平台，用来识别模组；不会上传文件本身 |
| 检查启动器更新、公告 | GitHub（`api.github.com`、`github.com`、`raw.githubusercontent.com`）、jsDelivr（`cdn.jsdelivr.net`） | 每次启动时各一次，可以在设置里关掉；应用内更新从本项目的 GitHub Releases 下载 |
| 崩溃分析规则更新 | 你在设置里填写的地址 | 只在你填了地址并点"立即更新"时 |
| 服务器列表 | 你添加的 Minecraft 服务器，以及系统的 DNS 服务器（查询 SRV 记录） | 打开「版本管理 → 服务器」时，查询每个服务器的延迟和在线人数 |

游戏本身启动后会连接 Mojang / Microsoft 和你进入的服务器，这部分由游戏负责，不经过启动器。

### 日志

启动器界面里显示的游戏日志会把访问令牌等敏感信息打码。Minecraft 和模组自己写到磁盘的日志不经过启动器处理，分享前请自己检查。

### 联系

有问题请在 [GitHub Issues](https://github.com/MRTangwin8/TheGodBBSLauncher/issues) 提出。

---

## English

TheGodBBSLauncher ("the launcher") is an open-source launcher for Minecraft: Java Edition. **The developer does not operate any servers and does not collect, receive or sell any user data.** The launcher contains no analytics, telemetry or advertising code.

### Where data is stored

All data stays on your own computer:

- **Settings**: `settings.json` in the system application data directory.
- **Accounts**: `accounts.json`. Login tokens are encrypted with AES-GCM; the key is kept in the system keychain (Windows Credential Manager, macOS Keychain, Linux Secret Service), falling back to a key file in the config directory if no keychain is available.
- **Game files**: the game directory (by default `.minecraft` next to the launcher), including worlds, mods, screenshots and the server list.
- **Instance settings, world backups, download caches**: in the game directory or the application data directory.

Uninstalling the launcher does not delete the game directory. You can delete any of these files at any time.

### Services the launcher connects to

The launcher connects directly to the following third-party services, only when you use the related feature. These services can see your IP address and the launcher version (`User-Agent: GodBBSLauncher/<version>`). Their own privacy policies apply.

| Purpose | Service | When |
|---|---|---|
| Game downloads, version lists, assets | Mojang (`*.mojang.com`, `libraries.minecraft.net`, `resources.download.minecraft.net`) and the BMCLAPI mirror (`bmclapi2.bangbang93.com`) | When installing or launching the game. The download source is selectable; automatic mode measures the speed of both |
| Mod loader installation | Official Fabric, Quilt, Forge and NeoForge metadata and Maven repositories | When installing a mod loader |
| Java downloads | Mojang Java runtimes; Azul (`api.azul.com`, `cdn.azul.com`) for platforms Mojang does not cover | When no suitable Java is installed |
| Microsoft account login | Microsoft (`login.microsoftonline.com`), Xbox Live, Minecraft services (`api.minecraftservices.com`) | When adding or refreshing a Microsoft account and when launching |
| Third-party (authlib-injector) login | The authentication server you enter (e.g. LittleSkin) and `authlib-injector.yushi.moe` | Only if you add such an account |
| Skin display | Mojang (`sessionserver.mojang.com`, `textures.minecraft.net`) or your authentication server | When showing account skins |
| Resource browser, mod update checks | Modrinth (`api.modrinth.com`, `cdn.modrinth.com`), CurseForge (`api.curseforge.com` with your own API key, otherwise the MCIM mirror `mod.mcimirror.top`), KLPBBS resource hub (`hub.klpbbs.com`) | When searching, downloading or checking for updates. Update checks send hashes of local mod files (SHA1 / CurseForge fingerprints) to identify them; the files themselves are never uploaded |
| Launcher updates, announcements | GitHub (`api.github.com`, `github.com`, `raw.githubusercontent.com`), jsDelivr (`cdn.jsdelivr.net`) | Once per start each; both can be turned off in settings. In-app updates are downloaded from this project's GitHub Releases |
| Crash rule updates | The URL you enter in settings | Only if you set a URL and click "update" |
| Server list | Minecraft servers you add, and your system DNS servers (SRV lookups) | When opening the Servers tab, to show latency and player counts |

Once started, the game itself connects to Mojang / Microsoft and to the servers you join; that traffic does not go through the launcher.

### Logs

Game logs shown inside the launcher have access tokens and similar secrets masked. Log files written to disk by Minecraft and mods are not processed by the launcher; please review them before sharing.

### Contact

Please open an issue on [GitHub](https://github.com/MRTangwin8/TheGodBBSLauncher/issues).
