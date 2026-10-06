# 开发交接（HANDOFF）

> **维护规则（每次改动都必须做，别只写日志）**
>
> 1. 每完成**一轮**开发或修改，在文末「变更日志」**追加**一条：轮次、提交号、做了什么、验证到什么程度、遗留了什么。
> 2. 同时**更新**上面的「当前状态」「待办与已知问题」「易踩的坑」——状态区必须反映最新事实，不能只追加日志。
> 3. 只在**远程 CI 才能暴露**的坑（构建环境、签名、平台差异）尤其要写进「易踩的坑」，不要只留在提交信息里。
> 4. 本文件是给"下一个接手的人（或下一个 agent 会话）"看的：写事实、写命令、写链接，别写感想。

---

## 0. 当前状态（一句话）

桌面端（Windows/macOS/Linux）与 **Android 端**功能齐备，代码在 `main`；所有安装包与 APK 都由 **GitHub Actions 远程构建**，本机不需要也不应该打包。Android 目前产出**调试签名 APK**（可安装测试），正式签名所需的上传密钥（keystore）**尚未配置**，但工作流与签名流程已用一次性密钥实测通过。

- 版本：`3.3.1`（`src-tauri/tauri.conf.json` 与两个 `Cargo.toml`/`package.json` 需一起改）
- Android 验证基准：调试包构建 [run 37441422009](https://github.com/BevalZ/Eisenhower-matrix/actions/runs/37441422009) ✅；签名自检 [run 37443294373](https://github.com/BevalZ/Eisenhower-matrix/actions/runs/37443294373) ✅（release APK 19MB + AAB 7.7MB，均已签名）
- 合并进 `main` 后的首次 Android 构建：[run 37461559211](https://github.com/BevalZ/Eisenhower-matrix/actions/runs/37461559211) ✅（commit `0dd4531`，产物 `android-0dd4531…` 47.1MB），同一次推送的 `CI` 也通过

---

## 1. 项目速览

| 层 | 技术 | 位置 |
|---|---|---|
| 前端 | Vue 3 + TypeScript + Vite（无 UI 框架，手写 CSS） | `src/` |
| 后端 | Rust + Tauri 2.12 + rusqlite(SQLite) | `src-tauri/src/` |
| 平台 | Windows / macOS / Linux / **Android**（同一套代码） | — |
| 同步 | WebDAV（跨平台）+ Tailscale 直连（**仅桌面**） | `webdav.rs` / `sync.rs` |
| AI | TypeSafe AI（JevAI）或任意 OpenAI 兼容接口 | `jevai.rs` / `openai.rs` |

数据流：前端只通过 `invoke(...)` 调 Rust 命令（命令集中在 `lib.rs` 注册），状态集中在 `useTasks` 一个模块级单例里；**没有**任何前端直连数据库/网络的路径。

---

## 2. 开发环境：本机的坑（重要，每次新会话先读这段）

这个项目的约定是**不在本机打包**（用户本机没有 Android/Java 环境，Rust 工具链也偏旧），所有正式构建放 GitHub Actions。

本机（用户这台 Windows 机器）现状：

| 项 | 现状 / 应对 |
|---|---|
| Node | **不在 PATH**。用自带：`C:\Users\zhaoh\.dsh\dsh-runtimes\dsh-primary-runtime\dependencies\node\bin\node.exe` |
| Rust | `rustc 1.88`，但项目锁定的 `tauri 2.12` / `tauri-build 2.7` 要求 **1.90** → 直接 `cargo build/test` 会报 `rustc 1.88.0 is not supported`。本地验证加 `--ignore-rust-version`；**正式构建交给 CI**（CI 用 stable，已是 1.9x） |
| Java / Android SDK / NDK | **没有**。`keytool`、`sdkmanager`、`adb` 都不可用 → 不能在本地生成 keystore 或打 APK |
| `gh` CLI | **未安装** → 用 `scripts/ci.py`（见第 4 节） |
| GitHub 凭据 | 存在 Git Credential Manager：`git credential fill` 可取（用户 `BevalZ`）。该凭据可读写 repo，也**能读写 Actions Secrets** |
| 网络 | 命令行访问 GitHub 必须走代理 `http://127.0.0.1:7890`，且 `git` 要用 OpenSSL 后端（schannel 被拦）。本 checkout 的 `.git/config` 已设好：`http.proxy` + `http.sslBackend=openssl`；**换 clone 或新机器要重设**：<br>`git config http.proxy http://127.0.0.1:7890 && git config http.sslBackend openssl` |
| 沙箱 | 历史上 `workspace-write` 模式会导致：命令无法通过管道读子进程输出（`cargo`/`vitest` 报 EPERM）、无法写 `.git`（`commit`/`push` 报 Permission denied）。需要commit/push 或跑测试时把会话切到**完全访问**；本会话后段已是完全访问 |

---

## 3. CI/CD 全景

所有工作流都在 `.github/workflows/`：

| 工作流 | 触发 | 产物 | 备注 |
|---|---|---|---|
| `ci.yml` | push/PR 到 `main` | 无（跑检查） | 前端：`npm install`/`vue-tsc`/`npm test`/`npm run build`；Rust：`cargo check` + `cargo test`（ubuntu） |
| `build.yml` Test Build | push 到**非 main** 分支（忽略 `**.md`） | Windows 安装版+便携版（Artifacts，14 天） | 手动运行时可选 macOS/Linux |
| `release.yml` Release | push `v*` tag 或手动 | 各平台安装包 → GitHub Release | 用 `tauri-apps/tauri-action`；需要 `TAURI_SIGNING_PRIVATE_KEY` |
| `android.yml` Android | push `main` / `feat/**` / `ci/**`、`v*` tag、手动 | `app-universal-*.apk` / `.aab` → Artifacts `android-<sha>`；tag 时附加到 Release | 见下 |

`android.yml` 要点：

- runner 用**镜像自带的 Android SDK**（`/usr/local/lib/android/sdk`），`sdkmanager` 由脚本自己调用；**不要**再用 `android-actions/setup-android@v3`——它在本仓库跑许可协议交互时 `exit 1`（见第 8 节第 3 轮）。
- NDK 版本**自适应探测**（优先 27.0.12077973，再退到列表里最新可用版本），并写入 `NDK_HOME` / `ANDROID_NDK_HOME`。
- `gen/android` **不入库**（`.gitignore` 已忽略 `src-tauri/gen`），由 `npx tauri android init --ci --skip-targets-install` 在 CI 里生成。
- 只构建 **arm64（aarch64）**。要加 armv7/模拟器 x86_64：改 `android.yml` 里 `ABI_TARGET`、rust targets 列表，以及 `--target` 参数。
- 构建方式二选一（由「Decide the build flavour」步决定）：没有 `ANDROID_KEY_BASE64` → `--debug --apk`（可直接安装）；有 → `--apk --aab` 再用 `apksigner`/`jarsigner` 签名。
- **签名自检**：推一个名为 `ci/signing-selftest` 的分支，工作流会在 runner 上生成一次性密钥，走完整 release+签名流程（产物是测试签名，仅供验证流程）。验证完删分支即可。

配置正式签名（用户还没做）：在任意有 JDK 的机器上 `keytool -genkey -v -keystore upload-keystore.jks -keyalg RSA -keysize 2048 -validity 10000 -alias upload`，导出 base64，然后在仓库 Secrets 里加 `ANDROID_KEY_BASE64` / `ANDROID_KEY_ALIAS` / `ANDROID_STORE_PASSWORD` / `ANDROID_KEY_PASSWORD`（README「Android 端」一节有完整命令）。

---

## 4. 远程观察构建：`scripts/ci.py`

没有 `gh` CLI，就用这个（只用标准库 + git 已保存凭据 + 代理）：

```bash
python scripts/ci.py runs main android.yml        # 最近几次运行
python scripts/ci.py watch main android.yml       # 轮询到结束，失败时直接打印失败步骤日志
python scripts/ci.py steps <run_id>               # 每步结论
python scripts/ci.py artifacts <run_id>           # 产物名字/大小
python scripts/ci.py fetch <artifact_id> out.zip  # 下载产物（APK 等）
python scripts/ci.py log <job_id> "错误|error"     # 过滤日志
```

环境变量：`HTTPS_PROXY`（默认 `http://127.0.0.1:7890`）、`GITHUB_TOKEN`（默认取 GCM 凭据）、`GITHUB_REPO`（默认从 origin 推断）。

---

## 5. 代码地图与易踩的坑

### 前端 `src/`

| 文件 | 职责 |
|---|---|
| `App.vue` | 窗口判定（主窗口 vs 悬浮球）、看板/统计切换、全局快捷键、启动加载与主题 |
| `components/QuadrantBoard.vue` | 四象限看板、快速添加、键盘移动、接线拖拽 |
| `components/TaskCard.vue` | 卡片渲染、键盘操作、截止时间徽标；**`touch-action: none` 是触屏滚动手势实现的前提** |
| `components/TaskWizard.vue` | AI 对话式新建（五问 + 本地兜底估算） |
| `components/TaskEditDialog.vue` / `StatsPanel.vue` / `SettingsDialog.vue` / `FloatingBall.vue` / `ToastHost.vue` | 编辑 / 统计 / 设置（常规·数据·WebDAV·多端）/ 桌面悬浮球 / 提示条 |
| `composables/useTasks.ts` | **全局状态唯一入口**：所有 `invoke` 封装、乐观更新与回滚、象限分组的 computed |
| `composables/useBoardDrag.ts` | 指针拖拽 + 触屏手势（滑动滚动 / 长按后拖动）、ghost 动画 |
| `composables/usePlatform.ts` | 平台判定（`platform` 命令），手机端隐藏桌面专属 UI |
| `composables/useDueReminders.ts` | 分钟时钟 + 截止提醒（去重记录在 localStorage） |
| `ordering.ts` / `due.ts` / `wizard.ts` / `types.ts` | 纯逻辑：排序与落点、`touchIntent`、截止时间解析、向导选项、类型与象限元数据 |

坑：
- `useTasks().tasksByQuadrant(q)` 返回的是**内部数组**（只读，别原地改）；象限分组只在数据变化时算一次，别再在模板里反复 filter。
- 触屏：卡片是 `touch-action: none`，所以列表滚动由 `useBoardDrag` 手工实现；判断逻辑在 `ordering.ts::touchIntent`（有单测），改阈值先改那里。
- 乐观更新要成对写：先改本地、失败回滚（`useTasks` 里已有模式，新增写操作请照抄）。

### 后端 `src-tauri/src/`

| 文件 | 职责 |
|---|---|
| `lib.rs` | 所有 `#[tauri::command]`、`setup`（解析数据目录并打开数据库、迁移凭据、恢复监听）、`platform` 命令 |
| `db.rs` | SQLite 访问、迁移（`ensure_sync_columns` → `prune_tombstones`）、统计、按 uid 合并同步数据、导入导出校验 |
| `sync.rs` | Tailscale 状态读取、TCP 同步服务（`POST /v1/sync`，Bearer 密钥）、HTTP 请求解析 |
| `webdav.rs` | WebDAV 下载/上传与错误提示 |
| `jevai.rs` / `openai.rs` | 两家 AI 的分类调用与解析（共用 `classification()` 做偏置校准） |
| `secrets.rs` | 凭据存储：桌面走系统凭据库，**Android/iOS 走应用数据库并标记为明文** |
| `http.rs` | 共享 `reqwest::Client`（连接池/TLS 复用），WebDAV 用不跟随跳转的那个 |
| `models.rs` | 数据结构与校验阈值（象限 2.5、同步 schema 版本） |

坑：
- **数据目录**：桌面沿用 `dirs` 的历史路径（老数据不能丢），手机用 `app_data_dir`；改这里等于迁移用户数据，先想清楚。
- **迁移顺序**：`ensure_sync_columns` 里先补 uid/时间列 → 修重复 uid → 再建唯一索引；索引建失败会让 `Db::open` panic。
- **同步合并**：`merge_remote` 分 2000 条一个事务；`remote_schema < 1` 表示对端早于「截止时间」字段，**必须保留本地 `due_at`**。
- 新增平台相关代码请用 `#[cfg(target_os = "android")]` 之类的 cfg 分支，并确认桌面路径仍然编译（本地只能验证桌面）。

---

## 6. 怎么验证（照抄即可）

```bash
# 前端：类型检查 + 单测（Node 不在 PATH，用自带 node）
node node_modules/vue-tsc/bin/vue-tsc.js --noEmit
node node_modules/vitest/vitest.mjs run

# Rust 单测（本机工具链偏旧，必须加 --ignore-rust-version）
cd src-tauri && cargo test --offline --ignore-rust-version

# 远程：跑完整 CI（本机唯一能验证 Android 的方式）
python scripts/ci.py watch main android.yml
```

改 Android 相关代码时**本地无法编译验证**（没有 Android target/NDK），必须靠 CI：推分支 → `scripts/ci.py watch` → 看失败步骤日志。

---

## 7. 待办与已知问题

按优先级（P0 阻塞使用 → P3 优化）：

1. **P0 · Android 真机未验证**：没有人装到手机上跑过。重点看：能启动、看板可用、触屏「滑动滚动 / 长按 0.26s 拖动」手感、AI 分类与 WebDAV 在移动网络下是否正常。
2. **P1 · 正式签名未配置**：加了 4 个 Secrets 后同一条工作流自动产出签名 APK+AAB（流程已自检通过）。本机没有 JDK，无法本地生成 keystore。
3. **P2 · 已完成任务的墓碑会长期增长**：`prune_tombstones` 只清理**未完成**任务的墓碑（180 天），已完成墓碑保留是为了统计页的完成历史；同步包上限 16MB（约 7 万条），到顶会给出可操作提示。要彻底解决需要分页同步协议 + 完成历史的独立统计表。
4. **P2 · `Db::open` 失败仍然是 panic**：重复 uid 这个现实诱因已修（启动时自动重铸）。要做成错误弹窗，得在 `setup` 里弹原生对话框，而 `setup` 跑在事件循环启动前，blocking 对话框有死锁风险，需要单独设计。
5. **P3 · 锁文件未入库**：`package-lock.json`、`src-tauri/Cargo.lock` 目前未跟踪，CI 每次重新解析依赖（可复现性弱）。要固定版本就把它们提交。
6. **P3 · Android 只构建 arm64**：模拟器（x86_64）与老设备（armv7）需要时再开。
7. **P3 · Android 上没有系统凭据库**：API Key / WebDAV 密码明文存在应用私有数据库（设置页有明确提示）；后续可考虑 Android Keystore。
8. **P3 · 窄屏只调了手机竖屏**：平板/横屏未验证。
9. **P3 · `restore_from_webdav` 是整库覆盖并重排 id**（设计如此，UI 有二次确认）。
10. **P3 · `sync_all_peers` 串行同步**：设备多时较慢，但顺序同步避免了 SQLite 锁竞争，暂不改。

---

## 8. 变更日志（新的一轮追加在最上面）

### 第 4 轮 · 2026-10-06 · 交接文档与 CI 脚本

- 新增本文件 `HANDOFF.md` 与 `scripts/ci.py`（把上一轮临时用的 CI 观察脚本正式收进仓库，加上了网络重试）。
- README 增加指向本文件的入口。
- 未改任何业务代码；验证方式：`vue-tsc` / `vitest` / `cargo test` 结果与第 3 轮一致（文档与脚本改动不影响构建）。

### 第 3 轮 · 2026-10-06 · Android 端 + 远程打包（提交 `33aa17b` → `0dd4531`，已快进合并进 `main`）

- **代码适配**：数据库路径改由 Tauri 解析（桌面沿用原目录，老数据不受影响；手机用应用私有目录）；`keyring`/`dirs` 改为桌面专属依赖，Android 无凭据库时密钥存应用数据库并由设置页提示明文保存；手机端没有 `tailscale` 命令，多端直连返回明确提示，界面隐藏「多端」标签并引导用 WebDAV；新增 `platform` 命令，手机端隐藏悬浮球。
- **触屏与窄屏**：卡片保持 `touch-action: none`，滑动改为手势滚动列表，长按 0.26 秒后再拖动才是移动卡片（判定抽成可单测的 `ordering.ts::touchIntent`）；窄屏下 2×2 看板紧凑化、对话框占满宽度、安全区内边距、`dvh`。
- **CI**：新增 `.github/workflows/android.yml`；签名自检分支 `ci/signing-selftest`。
- **踩过的坑**：①`android-actions/setup-android@v3` 在本仓库许可协议交互 `exit 1` → 改用 runner 预装 SDK 的 `sdkmanager`；②NDK 版本写死有风险 → 改为探测；③产物下载走 Azure blob 时必须去掉 `Authorization` 头（否则 401）；④本机 git 必须用 `http.sslBackend=openssl` + 代理才能连 GitHub。
- **验证**：Rust 24/24、前端 38/38、`vue-tsc` 0 错误；Android 调试包构建成功（run 37441422009，产物含 `lib/arm64-v8a/libeisenhower_matrix_lib.so`、v2/v3 签名块）；签名自检成功（run 37443294373，release APK 19MB + AAB 7.7MB）；桌面 Test Build 仍通过（run 37441421969）。
- **遗留**：Android 真机未验证；正式签名待配置；只构建 arm64。

### 第 1–2 轮 · 2026-10-06 · 代码优化与同步/密钥问题修复（与第 3 轮同一个提交 `33aa17b` 入库）

> 注意：这两轮的改动当时未单独提交，最后和第 3 轮一起进了 `33aa17b`，该提交信息只写了 Android 部分。

- **第 1 轮（性能与健壮性）**：`get_stats` 由约 17 条 SQL 压到 3 条并加索引；`newest_by_uid` 从 O(n²) 改哈希索引；迁移先修重复 uid 再建唯一索引（否则 `Db::open` panic）；`tailscale status` 改 `tokio::process` + 5 秒超时 + `kill_on_drop`（原先阻塞 runtime，且状态命令跑在主线程）；一轮同步只读一次 Tailscale 状态；请求读取失败回 400 而不是直接断连；请求头结束符改增量查找；AI/WebDAV 共用 `reqwest::Client`；导出对话框与写盘挪到 `spawn_blocking`；前端象限分组改一次性 computed、`updateTask` 回滚补齐 `due_at`、启动加载用 `allSettled`、`useToast` 清理被挤出项、错误文本统一 `errorText()`。
- **第 2 轮（三项遗留修复）**：同步超 5000 条会永久失败的硬限制改为分块合并（2000/事务，安全上限 20 万）；未完成任务墓碑 180 天后清理（已完成墓碑保留以维持统计历史）；对端请求体上限 2MB→16MB 并给出可操作提示；`PeerSyncStatus` 不再下发同步密钥，改为 `secret_set` + 按需 `reveal_sync_secret`（保存空密钥=保留原值）；系统凭据库不可用时写入 `secrets_plaintext` 标记，设置页显示明文警告。
- **验证**：Rust 24/24（新增 3 个用例）、前端 38/38、`vue-tsc` 0 错误、`cargo check --all-targets` 通过。
