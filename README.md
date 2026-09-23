# 四象限任务管理器 (Eisenhower Matrix)

一款基于 **艾森豪威尔矩阵** 的桌面任务管理应用，使用 **Tauri + Rust + Vue 3** 构建。
集成 [TypeSafe AI (JevAI)](https://typesafe.ai)，通过 AI 自动判断任务的重要性与紧急性，将任务精准归入四象限中的具体位置。

> "The key is not to prioritize what's on your schedule, but to schedule your priorities." — Stephen Covey

## 功能特性

- **🤖 AI 自动象限分类** — 对话式描述任务，JevAI 从重要性、紧急性两个维度评分，自动映射四象限并给出象限内优先级
- **📊 四象限看板** — 经典 2×2 矩阵视图，支持拖拽跨象限移动、同象限内排序
- **✅ 任务完成** — 一键标记完成，已完成任务置灰划线，可批量清除
- **📈 数据统计** — 总览面板：完成率、各象限分布、近 7 天完成趋势
- **🌗 双主题** — 浅色 / 深色主题一键切换，自动记忆偏好
- **📤 导入导出** — JSON 备份导出与导入恢复，支持跨设备迁移
- **☁️ WebDAV 同步** — 支持将任务备份上传到任意 WebDAV 服务器（Nextcloud / ownCloud 等），一键云端恢复
- **💾 本地数据持久化** — SQLite 本地存储，数据不上云，隐私安全
- **⚡ 轻量桌面应用** — Tauri 构建，安装包小、启动快、内存占用低

## 四象限说明

| 象限 | 名称 | 策略 |
|------|------|------|
| Q1 | 重要且紧急 | 立即做（危机、截止任务） |
| Q2 | 重要不紧急 | 计划做（长期目标、学习、健康） |
| Q3 | 紧急不重要 | 委托或快做（临时打断、部分会议） |
| Q4 | 不重要不紧急 | 删除或少做（无意义消磨） |

## 技术栈

- **前端**: Vue 3 + TypeScript + Vite
- **后端**: Rust (Tauri 2.0)
- **数据库**: SQLite (bundled via rusqlite)
- **AI API**: TypeSafe AI JevAI (`jev-latest` model)
- **HTTP**: reqwest

## 前置要求

开发前请安装以下工具：

1. **Rust** — 安装 [rustup](https://rustup.rs/)
2. **Node.js** ≥ 18 和 npm
3. **系统依赖**（Linux 构建需要）:
   ```bash
   # Ubuntu/Debian
   sudo apt install libwebkit2gtk-4.1-dev build-essential curl wget file libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev
   ```
   - macOS: Xcode Command Line Tools
   - Windows: WebView2 (Win10/11 通常已预装)

## 快速开始

```bash
# 1. 克隆仓库
git clone https://github.com/yourname/eisenhower-matrix.git
cd eisenhower-matrix

# 2. 安装前端依赖
npm install

# 3. （首次运行）生成应用图标
#    准备一张 1024x1024 的 PNG 放到项目根目录，然后：
npm run tauri icon app-icon.png

# 4. 开发模式运行
npm run tauri dev

# 5. 构建桌面安装包
npm run tauri build
```

构建产物位于 `src-tauri/target/release/bundle/`：
- **macOS**: `.dmg`
- **Windows**: `.msi` / `.exe`
- **Linux**: `.deb` / `.AppImage`

## 配置 JevAI

1. 打开应用，点击右上角 ⚙ 设置图标
2. 前往 [dashboard.typesafe.ai](https://dashboard.typesafe.ai) 获取 API Key
3. 粘贴到设置中并保存
4. 点击「+ 新建任务」，通过对话描述任务即可获得 AI 自动分类

> 未配置 API Key 时，应用仍可正常使用，只是 AI 分类步骤需要你手动选择象限。

## 项目结构

```
eisenhower-matrix/
├── src/                    # 前端 Vue 源码
│   ├── components/         # 组件（看板、任务卡片、向导、统计、设置）
│   ├── composables/        # 状态与 Tauri 命令桥接
│   ├── App.vue
│   └── main.ts
├── src-tauri/              # Rust 后端
│   ├── src/
│   │   ├── main.rs         # 入口
│   │   ├── lib.rs          # Tauri 命令注册
│   │   ├── models.rs       # 数据模型与象限计算
│   │   ├── db.rs           # SQLite 数据层
│   │   └── jevai.rs        # TypeSafe AI API 客户端
│   ├── Cargo.toml
│   └── tauri.conf.json
├── index.html
├── package.json
└── vite.config.ts
```

## 数据存储位置

应用数据存储在操作系统标准数据目录：
- **Linux**: `~/.local/share/eisenhower-matrix/tasks.db`
- **macOS**: `~/Library/Application Support/com.eisenhower.app/tasks.db`
- **Windows**: `%APPDATA%\com.eisenhower.app\tasks.db`

## License

[MIT](./LICENSE)
