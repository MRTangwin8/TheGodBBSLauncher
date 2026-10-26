# 资源来源

更新日期：2026-10-25。

## 搜索与安装

- Modrinth：原生搜索、版本列表和依赖安装。
- CurseForge：设置里填写自己的 API Key 时访问官方 API，留空使用 MCIM 社区镜像。密钥不需要发送给开发代理，也不应写进源码或 GitHub。
- KLPBBS 中文：通过 `https://hub.klpbbs.com/api/v1/resources` 查询公开中文目录，当前公开读取无需密钥。保持真实的 Modrinth / CurseForge 来源和项目 ID，选择资源后仍通过对应上游接口查询版本、解析依赖与校验文件。它不是兼容全部 CF 接口的代理。

搜索目录不要求预先安装游戏。安装模组、资源包或光影时，仍须选择已安装的目标游戏版本。

KLPBBS 不会收到设置里的 CurseForge 密钥，也不会把 CF 密钥当成 KLPBBS 的 API Key 使用。

## 整合包

资源中心的「整合包」分类可以搜索 Modrinth、CurseForge 和 KLPBBS 中文目录里的整合包：

- 整合包自带游戏版本和加载器，所以不选安装目标，也不按已安装的版本筛选。
- 选择一个版本后，启动器把 `.mrpack` / `.zip` 下载到 `数据目录/cache/modpacks/`，再打开和拖进窗口相同的导入对话框（预览、起名、安装）。导入时会单独创建一个版本，不影响已有的游戏。
- CurseForge 的服务端包会被过滤掉；作者关闭第三方下载的版本没有下载地址，界面会提示去项目页面手动下载。
- 列表上方有「前往 MCBBS 纪念版寻找整合包」入口，用系统浏览器打开纪念版资源中心的整合包栏目。

## 社区资源

资源中心的「社区资源」页签有两个入口：

1. MCBBS 纪念版资源中心：`https://www.mcbbs.co/portal.php?v3page=resources&type=modpack`
2. 红石中继站：`https://www.mczwlt.net/`

桌面版统一调用系统默认浏览器。浏览器预览使用普通外部链接。没有“逛论坛”导航、MODHUB 网站入口或内置网页窗口，也没有抓取、伪装这两个社区的下载接口。

从网站取得整合包后，可使用已有的整合包导入功能；网站中的登录、网盘和作者分发限制由用户在浏览器中自行处理，不会静默下载或执行脚本。

## CF 下载与凭据

官方 2026-06-10 公告说明，`edge.forgecdn.net` 的直连文件下载从 2026-07-16 起要求有效 API Key。启动器已补齐资源安装、兼容性检查和整合包导入的下载鉴权。

- 只在受限的官方 HTTPS 文件端点发送密钥，不向 MCIM、KLPBBS、Modrinth 或其他跳转域发送。
- 鉴权下载逐跳构建请求，避免自定义密钥请求头被自动带到其他主机。
- Key 不放进下载 URL；错误与日志会屏蔽常见 API Key 字段。
- API 未提供文件下载地址时，不自行拼 CDN 地址。资源界面提供项目官网入口；整合包必需文件缺失或受限时明确停止导入。
- 当前 Key 随启动器设置保存在本机配置文件中，不是供所有用户共用的内置密钥。不要分享含有密钥的设置文件。

官方资料：

- `https://docs.curseforge.com/rest-api/`
- `https://blog.curseforge.com/introducing-api-key-authentication-for-curseforge-file-downloads/`
- `https://hub.klpbbs.com/api-docs`

## 验证

```sh
npm test
npm run build
cd src-tauri
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
cargo run --locked --example catalog -- sodium
```

目录冒烟测试只读取公开结果，不使用密钥，不下载或安装游戏。当前没有使用真实 CF Key 验收官方直连；社区网页入口不等于已实现社区资源的自动安装。
