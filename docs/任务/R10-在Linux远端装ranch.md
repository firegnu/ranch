# 任务：ranch 在 Linux 远端能正式装上、用起来

2026-10-15 起草（paddock/main 以 ranch 主控身份），10-16 ranch/main 和 paddock/main 对过一遍后改定。
类型：功能变更
依据：paddock P5-90（在 paddock 里接远程机器上 corral 托管的 agent）要远端有能用的 corral。用户的远端多半是 Linux 或 VPS。本轮让 ranch 整包（corral、ranch、技能）在 Linux 上编得过、测得过、打得了包、照步骤装上，并在用户的一台 Linux 机器上实测。不做 paddock 那边的事，不改公开约定。
执行：主控自己做（要 ssh 到用户的机器上编译和实测，按下面的规矩只开 pi、omp，不交给别的 agent）。

## 用户的话
- 10-15：“远端一般不会常开，远端一般就是要么linux，要么是vps之类的”
- 10-15：“其实我觉得这种远程的场景还是挺重要的。”
- 10-15：“只安装corral吗？ranch不安装？”；“我不是说那台机器不派活，只是暂时不派。暂时不用不代表不做啊”
- 10-15，实测用的机器：“那上面有pi……你可以拿pi和omp做实验。但是也不要让他们委派任何活……不要在任何情况下运行那台机器上的claude code和codex”（用户说明是暂时的）。
- 10-16：“接下来我觉得要和paddock合作做远程agent接入的事情了”；“按照你的建议来，先和 paddock/main 对一下。”

## 现状（主控 10-15 查的）
- corral 写过 Linux 的分支：`crates/corral/src/freeze.rs` 不是 macOS 时读 `/proc`；`state.rs` 的 unix 套接字长度上限按平台取（macOS 104，其他 108）。但 ranch 没有 CI，没在 Linux 上编过、跑过测试。
- 打包工具（`tools/package`）用 `rustc -vV` 的 host 写进 `BUILD.txt`，没看到只认 macOS 的地方（动手时再看完）。
- 实测机器（用户的一台 Linux x86_64，登录 shell zsh）：有 cargo、git；没装 corral、ranch；pi、omp 只在交互登录 shell 的 PATH 里，ssh 直接跑命令时找不到。经公网复用连接一条命令约 0.9 秒。
- R9（macOS 上并发 start 卡住）不挡这件：Linux 上标准库建管道时同时设 close-on-exec。碰到卡住就记下，不在这件里修。

## 在哪里干活
- 本机：分支 `r10-linux`，worktree `../ranch-worktrees/r10-linux`。
- 实测机器：代码放 `~/Develop/ranch`。本机的改动用 `git push <host>:~/Develop/ranch r10-linux` 直接推过去，不经 GitHub，不推任务分支到 origin；合并后那边再取 main。

## 要做的
1. 在实测机器上跑 `cargo test --all-targets`、`cargo clippy --all-targets -- -D warnings`。
2. Linux 上编不过或测不过的，在 ranch 里修（Mac 上照样要过），每处写明原因。修不了、或者要改公开约定的，停下来问用户。
3. 在实测机器上用打包工具生成 `~/.local/share/ranch/versions/<提交>/`，核对 `BUILD.txt` 写的是 Linux 目标。
4. 部署（用户在场时做，和本机的部署同一个规矩）：链接 `~/.local/bin/corral`、`~/.local/bin/ranch`，跑 `corral install-skills` 和 `ranch dispatch install-skills`。装技能只写文件，不运行任何 agent。
5. 实测：只开 `ranch/test-*`（`--label role=test`），只用 pi、omp，用完 stop。都从本机经**非交互 ssh、用 corral 的绝对路径**调（paddock 就这么调）：
   - `start`：pi、omp 各开一个，`--cwd` 用临时目录。能起来，说明 corral 用 `-l -i` 重建登录环境在 Linux 上有效（pi、omp 不在非交互 ssh 的 PATH 里）。
   - 钩子：状态 starting → idle → working → idle；`send` 回 confirmed；`reply` 拿到原文；`wait` 正常返回。
   - `pause`/`resume`：真的冻住（走 `/proc` 那条分支），能恢复。
   - `attach`：本机 `ssh -tt -e none <host> <corral 绝对路径> attach <名字>` 看得到画面、能打字、Ctrl-] 能退；正常退出 ssh 后 `status` 的 `attached` 回到 0。
   - 套接字路径：用一个长一点的 agent 名字 start 一次（Linux 上限 108）。
   - `stop`；`corral integration status`、`ranch dispatch route` 能跑。
6. 在 DESIGN 的安装一节写上“在另一台机器（Linux）上装和升级”：取代码、打包、切链接、装技能，升级时再加 `corral upgrade --all`，由用户在场时做，前端不装、不升级远端。写明 corral 不需要在非交互 ssh 的 PATH 里，用绝对路径调即可；前端连上时找一次（先 `command -v corral`，找不到试 `$HOME/.local/bin/corral`），看 `corral --version` 的 contract 是不是 "1"，记下路径以后直接用。

## 怎么算做完
- 实测机器上 ranch 的标准检查（test、clippy）全过；Mac 上照旧全过。
- 实测机器上按 DESIGN 的步骤装好 corral、ranch 和技能，`corral --version` 的 contract 是 1。
- 第 5 条用 pi、omp 各走一遍，结果对；测试 agent 都停掉。

## 不要做
- 实测机器上任何情况下都不运行 Claude Code、Codex；pi、omp 不委派任何活（不让它们开 agent、不派任务）。用户说是暂时的，放开以用户明说为准。
- 主机名、地址、用户名不进仓库（仓库公开），写 `<host>`。
- 不动实测机器上别的东西（上面有别的服务和别人在用的数据）：不 `pkill`，不装系统包（要装先问用户），不改 shell 配置。ssh 命令一条一条发，那个地址会限制连得太快的连接。
- 不改公开约定（命令、参数、JSON、退出码、`~/.corral` 格式）；要改的停下来问。
- 不在这件里修 R9。
- paddock 的实测清单 4–5（拖窗口改大小、断网）不在这件里跑，R10 装好后由 paddock/main 请用户跑。

## 对 paddock 的影响
- 只加 Linux 支持和文档，不改公开约定，paddock 不用动；装好后 paddock 接着做 P5-90。
