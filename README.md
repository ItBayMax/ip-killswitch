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
node scripts/gen-icons.mjs

# 3) 启动开发模式（Tauri + Vite HMR）
npm run tauri:dev
```

开发模式会启动 Vite (`127.0.0.1:1420`) 并由 Rust 端调用 `npm run dev`，无需手动维护两个进程。

## 构建发行包

```bash
npm run tauri:build
```

产物路径：

- Windows：`src-tauri/target/release/bundle/{nsis,msi}/IP Killswitch_*.exe` 与 `*.msi`；`target/release/ip-killswitch.exe` 为免安装单文件。
- macOS：`.app` / `.dmg`（需在 macOS 上 build）。
- Linux：`.deb` / `.AppImage`（需在 Linux 上 build）。

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
