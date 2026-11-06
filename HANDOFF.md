# HANDOFF：写给接手 PhotoSee 的下一个 AI

你好。我是 2026-11-05 把 PhotoSee 发成 v0.1.0 的那个 AI（Claude Opus 5.5）。这封信写给你，内容包括：项目是什么、现在走到哪一步、怎么构建测试打包发布（命令都实跑过）、为什么这样设计、我们踩过的坑、还没修的问题，以及不要做的事。
更细的规则在 [AGENTS.md](AGENTS.md)（文件行数上限、不加依赖、ownership），各包的历史记录在 [TASKS.md](TASKS.md)。

> 用户的全局工作原则，照做：
> - 先实测再下结论。实测和早先的假设矛盾时，如实写"以为是 A，实测是 B"。
> - 绝不能出现"后台失败，用户以为成功"。
> - 只按路径 `git add`，每个独立改动单独提交，commit message 写真实依据（数字、复现步骤），构建和测试全绿才提交。
> - 机器是 Windows，主 shell 是 PowerShell；git bash 在 `D:\PortableGit\bin\bash.exe`。

---

## 1. 项目是什么，现在什么状态

PhotoSee 是一个只用来看图的 Windows 看图软件。Rust 核心 `core/` 负责解码、缩略图和目录遍历；`server/` 用 axum 提供本机 HTTP 接口，只监听 127.0.0.1，带令牌；`web/` 是不用框架的 TypeScript 界面；`app/` 是 Tauri 2 外壳，含托盘和 NSIS 安装包。
整个项目 2026-11-04 一天之内从零做到能用。当时由一个"编排者"拆包，交给 sonnet 子代理并行干活，每个包的 brief 在 `docs/packages/`，结果都记在 TASKS.md。

**v0.1.0（2026-11-05）已公开发布：**
- 仓库 `yoyoset/photosee`，public。11-05 第一次推送，推送前用 `git log --all -p` 扫过，没有密钥，也没有私人路径（只有一处 `fixtures-private` 的路径说明）。
- Release：https://github.com/yoyoset/photosee/releases/tag/v0.1.0
  - `PhotoSee_0.1.0_x64-setup.exe`：4.15 MiB，sha256 `074b0e1e7299f58e8251471e400dca8877bb82761adb7cee573c4978e8825c15`
  - `PhotoSee-0.1.0-portable.zip`：5.84 MiB，sha256 `a08f53fc8aa717ac3cbac12baee102eccceba5080bc67a53567af88ac5ad109a`
  - 两个文件都从 GitHub 下回来核过哈希，下回来的绿色版跑过，能出图。
- README（中文，5 张截图）、MIT LICENSE、CHANGELOG 都已入库。
- squareuncle.com 下载页**还没上架**。上架属于 SUBLOG 仓库，排期见 `F:\my_ai\SUBLOG\video\weekly\2026-11-博客与产品排期.md`（这个文件不进 git）。

## 2. 命令（全部实跑过）

```powershell
powershell -File scripts/check.ps1          # fmt + clippy -D warnings + cargo test + tsc + eslint + vitest + vite build + gates
powershell -File scripts/check.ps1 -Rust    # 只跑 Rust
powershell -File scripts/check.ps1 -Web     # 只跑前端
powershell -File scripts/package.ps1        # 构建安装包和绿色版 zip，约 4–5 分钟（release 编译占约 4 分钟）
node app/nsis/smoke.mjs                     # 安装包冒烟：静默装到 %TEMP% → 打开 JPEG → 卸载 → 比对 HKCU 注册表
cargo run --release --manifest-path tools/gen-fixtures/Cargo.toml   # 生成 fixtures/generated/（gitignored）
```

**Docker（LAN 服务端，2026-11-05 起部署在 $LAN_HOST:8111）：**
```bash
bash scripts/docker-stage.sh   # 本机交叉编译 musl 静态二进制 + web/dist → target/docker（约 3 分钟）
tar -C target/docker -czf - . | ssh root@$LAN_HOST "rm -rf $DEPLOY_DIR/build && mkdir -p $DEPLOY_DIR/build && tar --no-same-owner -xzf - -C $DEPLOY_DIR/build && chmod -R a+rX $DEPLOY_DIR/build && chmod 755 $DEPLOY_DIR/build/photosee-server && cd $DEPLOY_DIR/build && docker build --build-arg VERSION=$(cat VERSION) -t photosee:local -t photosee:$(cat VERSION) . && cd $DEPLOY_DIR && docker compose up -d --force-recreate"
```
- 首次需要 `rustup target add x86_64-unknown-linux-musl`。镜像是 `FROM scratch`，13 上不用拉任何基础镜像。
- 13 是非特权 LXC：解 tar 必须带 `--no-same-owner`，否则 chown 报错退出。
- compose 文件在 13 的 `$DEPLOY_DIR/docker-compose.yml`（不进仓库，里面是私人路径），数据（设置、缩略图缓存）在 `$DEPLOY_DIR/data`，属主 65534。
- 从 git bash 给 Python/curl 传 `/disk1/...` 这类参数会被 MSYS 改写成 `C:/Program Files/Git/disk1/...`，服务端回 403。要加 `MSYS_NO_PATHCONV=1`。

- 产物在 `target\release\bundle\nsis\PhotoSee_<ver>_x64-setup.exe` 和 `target\release\bundle\PhotoSee-<ver>-portable.zip`。
- 版本号所有产物统一（用户 11-06 要求）。要在四处同时改：`Cargo.toml` 的 `[workspace.package]`、`app/tauri.conf.json`、`web/package.json`、`web/package-lock.json`（顶层和 `packages[""]` 两处）。`gates.mjs` 会比对，不一致 `check.ps1` 就失败。`package.ps1` 从 `tauri.conf.json` 读版本号给 zip 命名；`docker-stage.sh` 从 `Cargo.toml` 写 `VERSION`，镜像打 `photosee:<ver>` 标签和 label；`/session` 和设置面板底部显示同一个号。tools/ 和 spikes/ 不发布，版本号不管。
- 11-05 的基线：Rust 247 passed / 9 ignored，web 466 passed，bundle 39,923 B gzip。

**发布流程（v0.1.0 就是这么走的）：**
1. `check.ps1` 全绿 → `package.ps1` → `smoke.mjs` 全 PASS。
2. 解压绿色版到一个新目录，跑一次，确认能出图。
3. `& "C:\Program Files\GitHub CLI\gh.exe" release create v<ver> <exe> <zip> --repo yoyoset/photosee --title "PhotoSee <ver>" --notes-file <说明> --target main`。gh 要写全路径，因为老终端的 PATH 可能没刷新；账号 yoyoset 已登录。
4. 用 `curl -L https://github.com/yoyoset/photosee/releases/latest/download/<文件名>` 下回来，核对 sha256，再跑一次。
5. 更新 CHANGELOG.md、TASKS.md 和排期文件。

**README 截图的做法：** 目录树会把真实盘符下的私人文件夹名拍进去，所以要按下面的步骤来。
1. 用 PIL 生成示例风景图，放进 scratchpad。
2. 用 `subst P: <示例目录>` 挂一个虚拟盘。
3. 解压**绿色版**来截图：它的数据在自己的 `data\` 里，不碰用户的 `%LOCALAPPDATA%\PhotoSee`。
4. 启动时设 `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=<端口>`，再用 CDP 的 `Page.captureScreenshot` 截图。驱动的写法照搬 `app/measure.mjs` 的 `attach()`。
5. 截完 `subst P: /D`。
注意：Claude Code 不允许删盘符根目录下的文件夹，所以示例图别放在 `F:\xxx`。

## 3. 关键决策和为什么

1. **传图走内嵌的 axum（127.0.0.1 + 随机令牌），不走 Tauri 自定义协议。** M0 实测，自定义协议端到端慢约 2 倍，纯传输慢最多 6.7 倍。见 `docs/m0-report.md`。
2. **服务端缩放是必须的。** 2400 万像素原图直接交给 WebView 解码绘制，中位数要 102 ms（p90 221 ms）；缩到屏幕尺寸后，中位数 31.8 ms。
3. **后台常驻默认关闭**，可以在托盘里打开。默认是关窗口就退出。最初的设计是常驻，用户反感"看不见的常驻进程"，11-04 改成了需要手动打开（commit 026ef01）。
4. **打开时直接进浏览模式（目录树 + 缩略图）**，不直接进单张看图。用户原话："专业一点，不要搞得跟个玩具一样"（11-04）。
5. **安装包只写"打开方式"（OpenWithProgids、Capabilities），绝不改默认程序和 UserChoice。** 每次发版都用 `smoke.mjs` 验证：卸载后注册表要和安装前一致。
6. **扩展名列表在 `core/src/format.rs` 和 `app/nsis/hooks.nsh` 各有一份**，因为 NSIS 读不了 Rust。`package.ps1` 打包前会比对两份，不一致就失败。加格式时两处都要改。
7. **AVIF 只透传，core 不解码。** `image` 的 `avif-native` 依赖 dav1d 的 C 构建，见 `docs/non-goals.md`。
8. **JPEG 在 Windows 上用 WIC 解码（DCT 缩放）和编码**（C2 包）。2400 万像素的转码时间从 583 ms 降到约 250–330 ms，但仍然超过 150 ms 的预算。
9. **"不做什么"写在 `docs/non-goals.md`**：编辑、相册、删除文件、云盘、视频、插件、主题、语言设置。用户提的新需求一律从"不做"开始讨论。

## 4. 踩过的坑（现象 → 原因 → 解决）

1. **C0 以为要装 MSVC Build Tools**，因为 `cl` 不在 PATH 里。实测 rustc 和 cc-rs 会通过 vswhere 找到 VS 18 Insiders，PowerShell 和 git bash 下都能找到，不用装。
2. **用 rAF 量"切换到上屏"的时间什么也没量到。** rAF 在 swap 后约 0.5 ms 就触发，那时像素还没合成，所以改成用浏览器的 paint timestamp。
3. **内存数字虚高。** 把每个进程的工作集加起来，会把 WebView2 共享的 DLL 重复算 7 次；改用 **private** 工作集（就是任务管理器里显示的那个）。
4. **`requestFullscreen` 需要用户手势。** 启动时自动全屏没有手势，所以由宿主路由来做，放在 guard 后面。见 `app/src/host.rs` 的注释。
5. **窗口隐藏后 `MainWindowHandle` 会变**，变成单实例插件的辅助窗口。测量脚本要先记下 handle，再直接问那个 handle。
6. **按住方向键快速翻页时，屏幕会停在错的图上**（ImageGlass #1754 那类问题）。按住时只显示缩略图，松手静止 `KEY_REPEAT_SETTLE_MS` 后才解码原图。
7. **加载指示条在 SMB 上不停闪**（150 ms 就出现）。改成延迟 450 ms 才出现，出现后至少显示 500 ms，依据是 Nielsen 的 0.1 s 界限。
8. **`app/measure.mjs startup` 现在跑不通**：它假设关窗口后进程还活着，后台常驻默认关闭（026ef01）以后进程直接退出，脚本在 `closeWindow` 处报错。11-05 测冷启动时我在 scratchpad 另写了一个简化脚本：每次用新的 `data\`（或保留），复制一个新文件名，读 `PHOTOSEE_PERF_OUT` 里的 `paint_thumb` 和 `paint_img`。**这个坑还没修**，修法是测之前打开常驻，或者只测冷启动。
9. **用 bash heredoc 加 Python 往 PowerShell 脚本里写 `'app\tauri.conf.json'` 时，`\t` 变成了制表符。** 改 Windows 路径要用 Edit 工具，写完用 `grep -c $'\t'` 查一遍。
10. **PowerShell 5.1 的 `Compress-Archive` 可能写出反斜杠路径。** 打 zip 要用 `%SystemRoot%\System32\tar.exe`（bsdtar），不能用 PATH 上 Git 带的 GNU tar。
11. **"并行开很多 Claude 子代理"太费 token**（用户 11-04 说的）。之后一次只开一个 worker，编排者自己提交。

## 5. 已知但还没处理的问题

- **RAW 只在合成容器上测过**，没有拿真实相机文件测。用户的样本还没给，Magpie 里有一条待办。真实样本放 `fixtures-private/`，设 `PHOTOSEE_PRIVATE_FIXTURES` 后测试会读它。
- HEIC 依赖系统的 HEIF/HEVC 扩展。Store 商品 ID 是 `9PMMSR1CGPWG` 和 `9NMZLZ57R3T7`，**没有在真机上点过那张卡片**确认能跳转。
- 2400 万像素 JPEG 转码约 250–330 ms，超过 150 ms 预算，熵解码本身就要约 110 ms。可以考虑桌面回环下原生 JPEG 改走透传，但那要改交付规则，得走 contract。
- 冷启动 1.2–1.45 s 出缩略图（11-05，绿色版，11 次），比 W2-APP 当时记的 1.03–1.23 s 慢。原因没查，测法也不同：那次是 `target\release\photosee.exe` 加固定文件，这次是绿色版，而且每次都换新文件。
- SMB/NAS 没在真实 NAS 上测过速度。`tools/bench-smb` 已经写好，还缺 NAS 路径。
- 读屏软件（NVDA、讲述人）没有完整走过一遍；触控板惯性也没在真机上测过。
- 单张预览模式下，胶片条的当前项居中，所以开头那段是空的，看着有点偏。这是按规格做的，没人提过意见。
- 安装包没有代码签名。
- macOS/Linux 外壳没做（`places()` 在非 Windows 上返回空）。
- `dev.photosee.app` 这个 identifier 以 `.app` 结尾，Tauri 会警告（和 macOS bundle 冲突），Windows 上没影响。

- **Docker 多用户（2026-11-05/06）**：设 `PHOTOSEE_USERS=/data/users.json` 后进入账号模式，这时 `PHOTOSEE_ROOTS` 只是「可挂载的硬盘」（volumes）。第一次打开网页，由第一个访问者创建管理员（名字 + PIN）。之后管理员在「用户管理」里建账号、重设 PIN、删账号。每个人在「我的文件夹」里从硬盘挑自己要看的文件夹挂上，**谁一开始都没有整块盘**（用户 11-06 明确要求）。
  - 隔离在服务端：每人一个 `AppState`（自己的 roots、设置、ui-state、上次看到的页 `reading`），见 `server/src/accounts.rs`、`sessions.rs`、`multi.rs`、`library.rs`、`admin.rs`。
  - 存储：`users.json` 是普通列表文件，每次改动原子重写，不用数据库；登录状态存在 `data/sessions.json`。登录一直有效，直到点「切换用户」，重新部署不会掉线。
  - 安全：PIN 明文存放（理由见 accounts.rs 头注释），连错 5 次锁 60 秒。挑文件夹时谁都能浏览整个硬盘的目录名，所以账号是分开书架，不是保密（见 non-goals）。
  - 已挂载的文件夹被移走或删掉时，跳过并打日志，不会让服务起不来。
  - 不设 `PHOTOSEE_USERS` 就是原来的开放模式。`PHOTOSEE_PASSWORD` 仍然会拒绝启动。
- 还没做：会话中途被登出（比如 users.json 删了这个人）时，页面不会自动跳到登录页，要刷新一次才会出现。
- Docker 镜像用 musl 的默认内存分配器，多线程转码可能比 glibc 慢，没有专门对比测过。13 上 1920×1080 转码实测约 190 ms。
- `.cbz` 的限制见 `docs/non-goals.md`：不支持 CBR/CB7/ZIP64，GBK 文件名会显示成乱码，安装包没把 .cbz 加进「打开方式」。

## 6. 不要做的事

- 不要 `git add -A` / `git add .`。
- 不要加新依赖，也不要放宽 300 行/文件、50 行/函数的限制（见 AGENTS.md）。真要加依赖，先写进 `docs/deps.md` 说明理由。
- 不要让安装包改默认程序或 UserChoice。
- 不要用 `taskkill /IM`、`Stop-Process -Name` 按名字杀进程，会把别人的 cargo/node 也杀掉。只按 PID 杀自己启动的进程。
- 不要用用户的真实相册截图或做测试数据。
- 不要往 README 写没实测过的数字。写的时候注明在哪台机器、用什么方法测的。
