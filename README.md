# IP Killswitch — 出口IP监测 + 进程管控

跨平台桌面工具（Windows / macOS / Linux），定时检测本机出口公网 IP，与允许列表比对，若不匹配则系统级弹窗 / 通知告警，并按策略结束指定进程。

## 功能

- **多检测源**：可配置多个返回出口 IP 的网站；HTML 与 text/plain 均自动解析（IPv4 / IPv6 内置正则，亦可自定义 regex）。
- **命中策略**：`任一命中` 或 `全部命中且 IP 一致`。
- **目标 IP 白名单**：多目标支持；UI 列出所有检测到的 IP 并标记是否匹配。
- **目标进程**：按 exe 名 / 进程名 / 路径子串匹配；支持多个；
  - kill 策略：`用户确认` / `自动 kill` / `仅通知`。
- **定时任务**：1m / 5m / 30m / 1h / 2h / 6h / 12h / 24h 预设，或直接填 6 段 cron 表达式（秒 分 时 日 月 周）。
- **手动检测**：仪表盘上可输入临时 URL + 目标 IP 直接执行，不会污染持久化配置。
- **失败重试**：每个检测源独立计数，默认 3 次（指数退避）。
- **系统托盘**：左键唤起主窗、菜单含 立即检测 / 暂停定时 / 退出；
- **关闭即最小化** + **退出二次确认**（均可在设置中关闭）。
- **开机自启**：可在设置中切换；自启动时以 `--minimized` 静默到托盘。
- **持久化日志**：tracing 每日滚动文件，UI 中展示最近 256KB。
- **单实例**：第二次启动会激活已有窗口而不是再开一份。

## 技术栈

- **桌面壳**：Tauri 2 (Rust)
- **前端**：React 18 + Vite 5 + Tailwind 3 + shadcn 风格内联组件
- **状态**：zustand
- **检测**：reqwest + 自定义重试 + 内置 IPv4/IPv6 正则
- **进程**：sysinfo 0.32
- **调度**：tokio interval + cron crate
- **日志**：tracing + tracing-appender (daily rolling)
- **托盘 / 通知 / 对话框 / 进程退出 / 自启动 / 单实例**：官方 Tauri v2 插件

## 目录结构

```
ip-killswitch/
├── package.json            # 前端依赖、npm 脚本
├── vite.config.ts          # 1420 端口 dev server
├── tailwind.config.js
├── postcss.config.js
├── tsconfig.json / tsconfig.node.json
├── index.html              # 已在预览面板可见
├── scripts/
│   └── gen-icons.mjs       # 生成占位 PNG / ICO / ICNS
├── src/                    # React 前端
│   ├── App.tsx
│   ├── main.tsx
│   ├── index.css
│   ├── api.ts              # Tauri invoke 绑定
│   ├── store.ts            # zustand store
│   ├── types.ts            # 与 Rust 端 serde 对齐
│   ├── lib/utils.ts        # cn、formatTime
│   ├── components/         # Dashboard / Providers / Processes / Schedule / Settings / Logs / Dialogs
│   └── components/ui/      # button / input / label / card / switch / tabs / dialog / badge / textarea
└── src-tauri/              # Rust 后端
    ├── Cargo.toml
    ├── tauri.conf.json
    ├── build.rs
    ├── capabilities/default.json
    ├── icons/              # 由 scripts/gen-icons.mjs 生成
    └── src/
        ├── main.rs
        ├── lib.rs          # Tauri Builder + 插件
        ├── commands.rs     # invoke handler 实现
        ├── config.rs       # JSON 持久化 + 类型
        ├── detector.rs     # HTTP + 解析 + 重试
        ├── processes.rs    # 列出 + kill
        ├── scheduler.rs    # 定时器（interval / cron）
        ├── tray.rs         # 托盘菜单
        ├── logger.rs       # tracing 文件
        └── state.rs        # AppState
```

## 安装与开发

```bash
# 1) 安装前端依赖
npm install

# 2) 生成占位图标（首次或替换图标时）
npm run gen:icons

# 3) 启动开发模式（Tauri + Vite HMR）
npm run tauri:dev
```

`npm run tauri:dev` 会在 **50001–59999** 之间随机挑一个可用端口启动 Vite，
并把端口注入到 Tauri 的 `--config` 覆盖里，避免多个项目并行开发时撞 1420。
`scripts/dev-server.js` 负责这一切，临时配置文件写到 `src-tauri/target/dev-server.conf.json`
（已 gitignored）。

如果想强制走固定 1420：`npm run tauri:dev:fixed`。

## 首次设置签名密钥（用于更新通道）

应用内置 Tauri Updater 插件，自动签名 + 验签每次发布的二进制。本仓库**不会**
提交私钥；首次 clone 后请按以下步骤一次性配置：

```bash
# 1) 生成密钥对（CLI 会交互式让你输入密码）
npx tauri signer generate -w tauri-signing-key.key

# 2) 把公钥 base64 写进 src-tauri/tauri.conf.json
npm run sync:pubkey

# 3) 把私钥与密码作为 GitHub Secret 配好（远端 release 工作流需要）
#    Settings → Secrets and variables → Actions：
#      TAURI_SIGNING_PRIVATE_KEY           ← cat tauri-signing-key.key
#      TAURI_SIGNING_PRIVATE_KEY_PASSWORD  ← 你刚才输入的密码
```

`tauri-signing-key.key` / `*.key.pub` 已被 `.gitignore` 覆盖，永远不会被提交。

## 构建发行包

### 本地构建

```bash
npm run tauri:build
```

产物路径：

- Windows：`src-tauri/target/release/bundle/{nsis,msi}/IP Killswitch_*.exe` 与 `*.msi`；`target/release/ip-killswitch.exe` 为免安装单文件。
- macOS：`.app` / `.dmg`（需在 macOS 上 build）。
- Linux：`.deb` / `.AppImage`（需在 Linux 上 build）。

启用了 `bundle.createUpdaterArtifacts`，每个 bundle 旁会同时输出 `*.sig` 签名文件，
更新通道使用。

### GitHub Release（自动化）

打一个 v 开头的 tag 即可触发 `.github/workflows/release.yml`：

```bash
git tag -a v1.0.0 -m "v1.0.0"
git push origin v1.0.0
```

工作流会：

1. 在 Windows / macOS (Apple Silicon + Intel) / Linux 上分别构建
2. 用 GitHub Secrets 里的 `TAURI_SIGNING_PRIVATE_KEY` 签名 updater bundle
3. 把所有 bundle + `.sig` 上传到对应 tag 的 Release
4. 生成 `latest.json` 并上传——这就是 in-app updater 读取的清单

发布默认为 **draft**，你在 GitHub UI 里点 Publish 后才正式对外可见 / 触发用户的更新检查。

## 版本号策略

- `package.json` / `src-tauri/Cargo.toml` / `src-tauri/tauri.conf.json` **三处版本必须一致**
- Release workflow 在打 tag 时会自动把三处同步到 tag 上的版本号（去掉 `v` 前缀）
- 本地手动更新版本就改这三个文件，然后 `git tag -a vX.Y.Z -m vX.Y.Z && git push --tags`
- 初始版本 **v1.0.0**

## 应用内更新

应用启动后 6 小时内自动检查一次（静默，无更新不打扰），用户也可以在窗口右上角点
「检查更新」按钮主动触发。检查走 `plugins.updater.endpoints` 配置的 URL：

```
https://github.com/ItBayMax/ip-killswitch/releases/latest/download/latest.json
```

下载的 bundle 必须通过 `pubkey` 签名校验，签名不对会直接拒绝安装——这意味着即使
有人劫持了 release 资产，没拿到私钥也没法假冒发布。

## 配置位置

| 平台    | 配置 / 日志路径                                                                              |
| ------- | ----------------------------------------------------------------------------------------- |
| Windows | `%APPDATA%\io.github.itbaymax.ipkillswitch\config.json` / `%LOCALAPPDATA%\io.github.itbaymax.ipkillswitch\logs\`           |
| macOS   | `~/Library/Application Support/io.github.itbaymax.ipkillswitch/config.json` / `~/Library/Logs/...`         |
| Linux   | `~/.config/io.github.itbaymax.ipkillswitch/config.json` / `~/.local/share/io.github.itbaymax.ipkillswitch/logs/`           |

## 设计细节

- **手动检测不会触发自动 kill**：仪表盘的临时检测纯粹是探针；只有计划任务 / 立即检测按钮在使用持久化配置时才会触发告警与进程处理。
- **`任一` 策略**：检测到的任一 IP 命中允许列表即视为合法。
- **`全部` 策略**：所有检测源必须成功且解析出同一 IP，且该 IP 在允许列表中。
- **失败重试**：每次失败之间指数退避（200ms × 2^n，封顶 16×）。
- **关闭主窗 vs 退出**：默认关闭按钮被拦截到托盘；设置中可改为弹出退出确认。
- **自启动静默**：自动启动时附带 `--minimized` 参数；主窗在 setup 阶段 hide 到托盘。

## 已知边界

- 占位图标是纯色圆盾，建议替换为真实品牌图：`npx @tauri-apps/cli icon ./your-source.png` 会覆盖 `src-tauri/icons/`。
- `confirm_exit=false` 时关闭事件直接退出，跳过弹窗；`close_to_tray=true` 与 `confirm_exit` 同时存在时优先最小化。
- 进程匹配在 Linux/macOS 区分大小写，在 Windows 上不区分。

## License

MIT
