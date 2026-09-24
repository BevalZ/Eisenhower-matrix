# 四象限任务管理器

> 基于艾森豪威尔矩阵的 AI 驱动桌面任务管理工具

[![CI](https://github.com/BevalZ/Eisenhower-matrix/actions/workflows/ci.yml/badge.svg)](https://github.com/BevalZ/Eisenhower-matrix/actions)
[![Release](https://github.com/BevalZ/Eisenhower-matrix/actions/workflows/release.yml/badge.svg)](https://github.com/BevalZ/Eisenhower-matrix/releases)
[![Platforms](https://img.shields.io/badge/platform-Windows%20%7C%20macOS%20%7C%20Linux-blue)](https://github.com/BevalZ/Eisenhower-matrix/releases)
[![Tauri](https://img.shields.io/badge/Tauri-2.0-FFC131?logo=tauri&logoColor=white)](https://tauri.app)
[![Vue](https://img.shields.io/badge/Vue-3-4FC08D?logo=vue.js&logoColor=white)](https://vuejs.org)
[![Rust](https://img.shields.io/badge/Rust-stable-000000?logo=rust&logoColor=white)](https://www.rust-lang.org)
[![License](https://img.shields.io/badge/license-MIT-green)](./LICENSE)

> *"The key is not to prioritize what's on your schedule, but to schedule your priorities."*
> — Stephen Covey

---

## 功能特性

| 功能 | 说明 |
|------|------|
| 🤖 **AI 自动分类** | 对话式描述任务，JevAI 从重要性、紧急性两个维度评分，自动归入四象限并给出象限内优先级 |
| 📊 **四象限看板** | 经典 2×2 矩阵，支持拖拽跨象限移动、同象限内排序、一键完成任务 |
| 🌗 **双主题** | 浅色 / 深色一键切换，自动记忆偏好 |
| 📈 **数据统计** | 完成率、各象限分布、近 7 天完成趋势 |
| 📤 **导入导出** | JSON 备份导出与导入恢复，跨设备迁移 |
| ☁️ **WebDAV 同步** | 备份上传至任意 WebDAV 服务器（Nextcloud / ownCloud 等） |
| 🔁 **Tailscale 多端同步** | 同一 tailnet 内按任务直连合并，不经过额外服务器 |
| 💾 **本地存储** | SQLite 本地持久化，同步只发生在你自己的 Tailscale 网络 |
| ⚡ **轻量** | Tauri 构建，安装包 < 10 MB，启动秒开 |

## 四象限方法论

```
              重要 ↑
              ┌─────────────┬─────────────┐
              │  Q2        │  Q1        │
              │  重要不紧急  │  重要且紧急  │
              │  计划做      │  立即做      │
              ├─────────────┼─────────────┤
              │  Q4        │  Q3        │
              │  不重要不紧急 │  紧急不重要  │
              │  删除        │  委托/快做   │
              └─────────────┴─────────────┘
                    不紧急 ←——→ 紧急
```

| 象限 | 策略 | 典型任务 |
|------|------|----------|
| Q1 重要且紧急 | 立即做 | 截止日期、危机、紧急修复 |
| Q2 重要不紧急 | 计划做 | 长期目标、学习、健康、规划 |
| Q3 紧急不重要 | 委托/快做 | 临时打断、部分会议、杂事 |
| Q4 不重要不紧急 | 删除 | 无意义消磨、过度浏览 |

## 下载安装

前往 [Releases](../../releases) 页面下载对应平台的安装包：

| 平台 | 格式 | 文件名 |
|------|------|--------|
| Windows | NSIS 安装版 / 便携版 `.exe` | `Eisenhower Matrix_*_x64-setup.exe` / `*_x64_portable.exe` |
| macOS | `.dmg` | `Eisenhower-Matrix_*.dmg` |
| Linux | `.deb` / `.AppImage` | `eisenhower-matrix_*.deb` |

> **macOS 提示**：首次打开若提示"无法验证开发者"，请到「系统设置 → 隐私与安全性」点击「仍要打开」。

## 从源码构建

### 前置要求

- **Rust** ≥ 1.77（[rustup.rs](https://rustup.rs/)）
- **Node.js** ≥ 18
- **系统依赖**：
  - **Windows**: 安装 [WebView2 Runtime](https://developer.microsoft.com/microsoft-edge/webview2/)（Win10/11 通常已预装）
  - **macOS**: Xcode Command Line Tools（`xcode-select --install`）
  - **Linux**:
    ```bash
    # Ubuntu / Debian
    sudo apt install libwebkit2gtk-4.1-dev build-essential curl wget file \
      libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev
    ```

### 构建步骤

```bash
# 克隆
git clone https://github.com/BevalZ/Eisenhower-matrix.git
cd eisenhower-matrix

# 安装依赖
npm install

# 生成应用图标（需要一张 1024×1024 的 PNG）
npm run tauri icon app-icon.png

# 开发模式
npm run tauri dev

# 构建安装包
npm run tauri build
```

构建产物位于 `src-tauri/target/release/bundle/`。

## 配置

### JevAI API Key

1. 打开应用，点击右上角 ⚙ 设置图标
2. 前往 [dashboard.typesafe.ai](https://dashboard.typesafe.ai) 获取 API Key
3. 在「常规」标签页粘贴并保存
4. 点击「+ 新建任务」，对话描述任务即可获得 AI 自动分类

> 未配置时仍可正常使用，AI 分类步骤可手动选择象限。

### WebDAV 同步

在「WebDAV 同步」标签页填入：
- **服务器地址**：完整的备份文件路径，如 `https://dav.example.com/eisenhower/backup.json`
- **用户名 / 密码**：WebDAV 凭证

点击「上传到 WebDAV」备份，「从 WebDAV 恢复」还原。

### Tailscale 多端同步

适合几台已经加入同一个 Tailscale 网络的电脑互相同步任务。连接只走 Tailscale 的 `100.64.0.0/10` 地址，不扫描局域网，也不上传到应用服务器。

1. 每台设备安装并登录同一个 tailnet。
2. 打开设置的「多端」页，填写**相同的同步密钥**和**相同的端口**（默认 `47321`）。
3. 两台设备都点击「开始接受同步」。应用会监听本机 Tailscale 地址，并大约每 45 秒与在线设备自动对齐；也可以手动同步某一台或全部在线设备。
4. Tailscale ACL 需要允许设备之间访问该 TCP 端口。macOS 首次监听时，系统可能会询问是否允许应用接受网络连接。

合并规则是按任务的修改时间后写覆盖；时间相同则用设备 ID 打破平局。删除会保留墓碑，所以删除也能同步到其他设备。API Key 和 WebDAV 密码不会同步。

如果两台设备在第一次同步前各自已经有任务，这些旧任务会各自生成 uid，可能变成重复项。可以先删掉多余任务，或者先用 JSON 导出，把其中一台的数据导入另一台，再开始同步。

## 项目结构

```
├── src/                      # 前端 Vue 3 + TypeScript
│   ├── components/           # UI 组件
│   │   ├── QuadrantBoard.vue # 四象限看板
│   │   ├── TaskCard.vue      # 任务卡片
│   │   ├── TaskWizard.vue   # AI 对话创建
│   │   ├── StatsPanel.vue   # 统计面板
│   │   └── SettingsDialog.vue# 设置管理
│   ├── composables/          # 状态与 Tauri 桥接
│   ├── App.vue
│   └── main.ts
├── src-tauri/                # Rust 后端
│   ├── src/
│   │   ├── lib.rs            # Tauri 命令注册
│   │   ├── models.rs        # 数据模型与象限计算
│   │   ├── db.rs             # SQLite 数据层
│   │   ├── jevai.rs          # TypeSafe AI 客户端
│   │   ├── webdav.rs         # WebDAV 客户端
│   │   └── sync.rs           # Tailscale 多端同步
│   ├── Cargo.toml
│   └── tauri.conf.json
├── .github/workflows/       # CI/CD 自动打包
├── index.html
└── package.json
```

## 数据存储

| 平台 | 路径 |
|------|------|
| Linux | `~/.local/share/eisenhower-matrix/tasks.db` |
| macOS | `~/Library/Application Support/com.eisenhower.app/tasks.db` |
| Windows | `%APPDATA%\com.eisenhower.app\tasks.db` |

## 技术栈

- **框架**: Tauri 2.0
- **前端**: Vue 3 + TypeScript + Vite
- **后端**: Rust
- **数据库**: SQLite (rusqlite, bundled)
- **AI**: TypeSafe AI JevAI (`jev-latest`)
- **HTTP**: reqwest

## 贡献

欢迎提交 Issue 和 Pull Request。

1. Fork 本仓库
2. 创建特性分支 (`git checkout -b feature/amazing`)
3. 提交更改 (`git commit -m 'Add amazing feature'`)
4. 推送到分支 (`git push origin feature/amazing`)
5. 开启 Pull Request

## License

[MIT](./LICENSE)
