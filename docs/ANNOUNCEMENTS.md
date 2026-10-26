# 公告栏

启动器首页的「公告栏」会显示这个仓库根目录下 [`announcements.json`](../announcements.json) 里的公告。维护者把公告提交并推送到 `main`，用户**下次启动**启动器时就能看到，不用发新版本。

启动器没有服务器，所以不是真正的"推送"，而是**启动时拉取一次**，运行期间不再拉取：

1. 先读 `https://raw.githubusercontent.com/MRTangwin8/TheGodBBSLauncher/main/announcements.json`；
2. 读不到（比如国内网络连不上 GitHub）就读 jsDelivr 的镜像 `https://cdn.jsdelivr.net/gh/MRTangwin8/TheGodBBSLauncher@main/announcements.json`；
3. 都读不到就显示上次缓存的公告；20 分钟内重复启动直接用缓存；
4. 用户可以在「设置 → 关于」里关掉「启动时获取公告」，或点「刷新公告」立即重拉。

## 怎么发公告

1. 编辑 `announcements.json`，往 `announcements` 数组里加一条（格式见下）。
2. 提交并推送到 `main`。**提交说明里加上 `[skip ci]`**，否则这次提交会白白触发一次三平台的构建：

   ```bash
   git add announcements.json
   git commit -m "公告：xxx [skip ci]"
   git push origin main
   ```

3. 等生效：GitHub 原始文件大约 5 分钟内更新；走 jsDelivr 镜像的用户可能要等更久（它缓存分支文件，最长约 12 小时），想马上生效可以访问 `https://purge.jsdelivr.net/gh/MRTangwin8/TheGodBBSLauncher@main/announcements.json` 清缓存。

不想显示了：把那一条删掉再推送；或者给它设一个 `expires` 到期日。

## 文件格式

```json
{
  "version": 1,
  "announcements": [
    {
      "id": "2026-10-27-welcome",
      "title": "欢迎使用神权论坛启动器",
      "body": "正文，纯文本。\n可以换行。",
      "date": "2026-10-27",
      "level": "info",
      "pinned": false,
      "link": "https://github.com/MRTangwin8/TheGodBBSLauncher/releases",
      "linkText": "查看更新日志",
      "expires": "2026-12-31",
      "minVersion": "0.3.0",
      "maxVersion": "0.3.2"
    }
  ]
}
```

| 字段 | 必填 | 说明 |
|---|---|---|
| `id` | 是 | 公告的唯一编号，只能用字母、数字、`-`、`_`、`.`，最长 64。用户点 × 关掉一条公告，记的就是这个 id；**改了 id 就会重新显示给所有人** |
| `title` | 是 | 标题，最长 60 个字 |
| `body` | 否 | 正文，纯文本，最长 800 个字，换行用 `\n` |
| `date` | 否 | 发布日期 `YYYY-MM-DD`，用于排序和显示 |
| `level` | 否 | `info`（默认）/ `notice` / `warn`；`warn` 会显示"注意"标签 |
| `pinned` | 否 | `true` 置顶，排在最前面 |
| `link` / `linkText` | 否 | 一个链接按钮，只允许 `http://` 或 `https://`，点了用系统浏览器打开；`linkText` 是按钮文字，最长 16 个字，默认"查看详情" |
| `expires` | 否 | 到期日 `YYYY-MM-DD`（按 UTC 算，当天还显示），之后不再显示 |
| `minVersion` / `maxVersion` | 否 | 只给某个版本范围内的用户看，两端都包含。比如"这个问题 0.3.3 修好了"，就写 `"maxVersion": "0.3.2"`。版本号写错了会被当成没有限制 |

排序：置顶的在前，其余按 `date` 从新到旧。

## 限制与安全

公告文件是**不可信的外部输入**，启动器按这些规则处理，写公告时也要遵守：

- 一次最多显示 **10 条**；整个文件不能超过 **64 KB**，超过就整个忽略；
- 标题、正文都按**纯文本**显示，写 `<b>` 之类的 HTML 不会生效，只会原样显示出来；
- 链接只允许 `http` / `https`，不能带账号密码；`javascript:`、`file:` 之类的会被丢掉；
- `id` 重复的只留第一条；`id` 不合法或标题为空的整条丢掉；
- 文件格式整体写错（不是合法 JSON）时，这次不显示任何公告，会沿用上次缓存的。

## 隐私

拉取公告会让 GitHub（或 jsDelivr）看到用户的网络地址，和检查更新一样。除此之外不会上传任何信息。用户可以在「设置 → 关于」里关掉。
