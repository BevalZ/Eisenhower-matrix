# 开发交接（HANDOFF）

> **维护规则（每次改动都必须做，别只写日志）**
>
> 1. 每完成**一轮**开发或修改，在文末「变更日志」**追加**一条：轮次、提交号、做了什么、验证到什么程度、遗留了什么。
> 2. 同时**更新**上面的「当前状态」「待办与已知问题」「易踩的坑」——状态区必须反映最新事实，不能只追加日志。
> 3. 只在**远程 CI 才能暴露**的坑（构建环境、签名、平台差异）尤其要写进「易踩的坑」，不要只留在提交信息里。
> 4. 本文件是给"下一个接手的人（或下一个 agent 会话）"看的：写事实、写命令、写链接，别写感想。

---

## 0. 当前状态（一句话）

桌面端（Windows/macOS/Linux）与 **Android 端**功能齐备，代码在 `main`；所有安装包与 APK 都由 **GitHub Actions 远程构建**，本机不需要也不应该打包。Android **已配置正式签名**（上传密钥在仓库 Secrets 里），每次构建产出签名 APK + AAB。

- 版本：`3.4.1`（`src-tauri/tauri.conf.json`、`src-tauri/Cargo.toml`、`package.json` 三处必须一起改，改完打 tag 发布）
- 最新发布：[Release v3.4.0](https://github.com/BevalZ/Eisenhower-matrix/releases/tag/v3.4.0) ✅（tag 指向 `03d4083`；8 个资产，Android 为**正式签名**的 18.0MB APK + 7.6MB AAB）
- Android 签名：密钥库备份在 `D:\Github_repos\Hydens\android-signing\`（PKCS12 + 口令说明 + 生成脚本），证书 SHA-256 `A3:58:FA:49:…:62:A9`；**这份备份需要你自己再存一份**，丢了无法用同一签名更新已分发的应用
- Android 验证基准：签名构建 [run 37555424774](https://github.com/BevalZ/Eisenhower-matrix/actions/runs/37555424774) ✅（PKCS12 识别 + 指纹与本地一致）；无密钥时的调试包构建 [run 37441422009](https://github.com/BevalZ/Eisenhower-matrix/actions/runs/37441422009) ✅；一次性密钥自检 [run 37443294373](https://github.com/BevalZ/Eisenhower-matrix/actions/runs/37443294373) ✅
- `main` 当前基线（commit `c9dfec0`）：`Android` 工作流会走 release + 签名路径；`CI` [run 37462398442](https://github.com/BevalZ/Eisenhower-matrix/actions/runs/37462398442) ✅ 为最近一次全绿记录

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
2. **已完成 · Android 正式签名**：上传密钥已生成并写入 4 个 Secrets（见第 6 轮），构建默认产出签名 APK+AAB。**待你做的事**：把 `D:\Github_repos\Hydens\android-signing\`（密钥库 + 口令说明）另存到安全位置；要轮换密钥时按 README「正式签名」一节操作，注意换密钥后已安装用户需卸载重装。
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

### 第 7 轮 · 2026-10-06 · 修复真机「停在悬浮球界面」+ 全新应用图标（版本 3.4.1）

- **真机问题**：安卓装上后打开停在悬浮球界面、进不去看板。原因：配置里有两个窗口，Android 会把第二个（`floating-ball`）也建成 activity 并推到前台，而前端按 `window.label === "floating-ball"` 就渲染球。三层修复：
  1. 新增 `src-tauri/tauri.android.conf.json` 平台覆盖配置，Android 只保留 `main` 窗口（Tauri 会按平台自动合并 `tauri.<platform>.conf.json`）；
  2. `lib.rs` 的 `setup` 里在 Android/iOS 上主动 `close()` 该窗口兜底；
  3. `App.vue` 用 UA 同步判断，手机端永不渲染球（避免 platform 命令返回前先闪一下）。
- **应用图标**：接入用户提供的分平台 SVG，放进 `src-tauri/icons-src/`（`icon-manifest.json` 指明 `default`=windows.svg、`android_bg`/`android_fg` 为自适应图标两层、`android_fg_scale=100`）。生成了全新 `src-tauri/icons/**`：通用/Windows 用小圆角方形、macOS 的 `icon.icns` 与 `ios/**` 用全出血版、Android 含 `mipmap-anydpi-v26/ic_launcher.xml` + 各密度前景/背景层；`android.yml` 在 `tauri android init` 之后加了一步 `tauri icon src-tauri/icons-src/icon-manifest.json` 把图标渲染进生成的 Android 工程。
- **验证**：像素采样确认四个象限配色与用户配色一致（左上粉橙 / 右上天蓝 / 左下薄荷绿 / 右下淡紫）；Android 分层尺寸 108/162/216/324/432 与官方要求一致；`cargo check` 通过（移动端那段代码用「临时去掉 cfg 编译一次再还原」的方式验证过，因为本机没有 Android target）；`vue-tsc` 0 错误；`vitest` 38/38。
- **发布**：版本 3.4.1，`release.yml` 文案改为「本次更新」（修复 + 图标 + 功能一览）。
- **未解决/说明**：Android 12+ 的系统启动画面（短暂显示应用图标 + 主题底色）是系统行为，无法完全移除；如需进一步弱化观感，要改 `gen/android` 的主题文件，而 `gen/android` 不入库，需在 CI 里打补丁（未做，等需求确认）。

### 第 6 轮 · 2026-10-06 · 配置 Android 正式签名（提交 `c9dfec0`）

- **密钥已生成并配置**：本机 Python（`cryptography` + `pynacl`，不需要 JDK）生成 PKCS12 上传密钥，别名 `upload`，有效期 10000 天；4 个 Secrets（`ANDROID_KEY_BASE64` / `ANDROID_KEY_ALIAS` / `ANDROID_STORE_PASSWORD` / `ANDROID_KEY_PASSWORD`）已写入仓库。密钥与口令只落在本机备份目录 `D:\Github_repos\Hydens\android-signing\`（`eisenhower-upload.p12` + `凭据与说明.txt` + 生成脚本），**没有**打印到对话或日志里。
- **证书指纹（可公开）**：`A3:58:FA:49:0A:FC:44:7D:F0:FC:35:50:1C:64:D4:C8:B3:79:11:86:A7:3A:F5:D3:9E:A6:2C:48:8E:91:62:A9`
- **工作流加固**：签名步骤先探测密钥库格式（JKS / PKCS12）再按扩展名改名（apksigner 靠扩展名判类型），签名后执行 `apksigner verify --print-certs` + `jarsigner -verify`，并把密钥库条目列表与证书指纹打进日志，方便核对；密钥库读不出来会给出明确报错。
- **端到端验证**：推 `ci/verify-signing` 分支触发真实签名构建（[run 37555424774](https://github.com/BevalZ/Eisenhower-matrix/actions/runs/37555424774) ✅）：`Build release APK + AAB` 与 `Sign APK and AAB` 均 success，日志里 `Keystore type: PKCS12`、证书主题 `CN=Eisenhower Matrix Upload Key`、以及与上文完全一致的 SHA-256 指纹；产物 `Eisenhower-Matrix_c9dfec0_android-arm64-release.apk` 18.0MB + `..._android.aab` 7.6MB。验证完把工作流改动快进合并到 `main`（`c9dfec0`）并删掉临时分支。
- **v3.4.0 Release 资产已更新**：用签名版（18.0MB APK + 7.6MB AAB，上传后 GitHub 返回的 sha256 与本地一致）替换掉原来的 174MB 调试包，并同步改了 Release 说明里的 Android 文件名与签名描述。注意：APK 是用**同一份应用代码**构建的（与 tag 的差异只在 CI 工作流文件），所以没有重新打版本号。
- **后续行为**：Secrets 存在时，`Android` 工作流默认走 release + 签名路径（不再产出 174MB 的调试包）；`release.yml` 的说明文案也已同步为「正式签名」。

### 第 5 轮 · 2026-10-06 · 发布 `v3.4.0`

- 版本号 3.3.1 → 3.4.0，三处文件同步修改。
- `release.yml` 的 Release 说明重写为本轮 feat：Android 端、同步容量修复（>5000 条不再永久失联）、密钥安全（同步密钥按需读取 + 明文存储提示）、性能（统计 SQL 17→3、同步去重 O(n²)→哈希、共享 HTTP 客户端）、启动稳定性（重复 uid 修复）、触屏与窄屏；安装表格补上 Android 与「当前为调试签名」的说明。
- `android.yml`：产物改名为 `Eisenhower-Matrix_<版本|短SHA>_android-arm64-<debug|release>.apk` 与 `..._android.aab`；「附加到 Release」**不再自己 create**（会与 tauri-action 抢同一个 release），改为最多等 10 分钟等 `release.yml` 建好再 `gh release upload`，超时只在 Artifacts 保留并给 warning。
- 发布方式：`git push origin main` → `git tag v3.4.0` → `git push origin v3.4.0`，`Release`（桌面四平台）与 `Android`（APK）两个工作流并行；Android 的 APK 会等到 Release 建好后挂上去。
- **发布结果（已验证）**：[Release v3.4.0](https://github.com/BevalZ/Eisenhower-matrix/releases/tag/v3.4.0) 已发布（非 draft），共 7 个资产：`Eisenhower-Matrix_3.4.0_android-arm64-debug.apk`（174MB，调试签名）、Windows `x64-setup.exe` 3.8MB + `v3.4.0_x64_portable.exe` 14.7MB、macOS `aarch64.dmg` 5.3MB / `x64.dmg` 5.5MB、Linux `amd64.deb` 7.1MB / `amd64.AppImage` 81.4MB。`Release` 工作流四个平台全部 success，`Android` 工作流 success 且成功等到 Release 后再上传（新逻辑生效，没有与 tauri-action 抢 release）。release 正文校验通过（新功能 / 安装 / 首次使用三段齐全，含 Android 安装说明）。
- **注意**：调试签名 APK 有 174MB（未剥离调试符号）；配置签名密钥后走 release 构建约 19MB，且可用于上架。
- 顺带修复：`scripts/ci.py` 的重试集合补上 `IncompleteRead`/半截 JSON（本轮被代理截断过一次响应）。

### 第 4 轮 · 2026-10-06 · 交接文档与 CI 脚本（提交 `d44d265`、`28b8933`）

- 新增本文件 `HANDOFF.md` 与 `scripts/ci.py`（把上一轮临时用的 CI 观察脚本正式收进仓库，加了网络重试）。
- README 增加指向本文件的入口与 `scripts/ci.py` 用法；`.gitignore` 忽略 `__pycache__/`；`android.yml` 的 `paths-ignore` 增加 `scripts/**`、`.gitignore`（纯脚本/文档改动的提交不再跑 APK 构建）。
- 未改任何业务代码；验证：`vue-tsc` / `vitest` / `cargo test` 结果与第 3 轮一致，且 `main` 上 Android [37462398269](https://github.com/BevalZ/Eisenhower-matrix/actions/runs/37462398269) 与 CI [37462398442](https://github.com/BevalZ/Eisenhower-matrix/actions/runs/37462398442) 两个工作流均通过（产物 `android-28b8933…` 47.1MB）。

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
