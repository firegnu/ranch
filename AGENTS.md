# ranch

ranch 是 corral 等“每台机器、每个用户只能有一份”的运行时所在的仓库。现在有 corral；以后还有遥测的存储与命令、插件协议和 dispatch。两个前端 Saddle（TUI，保底版）和 paddock（GPUI 桌面应用）都只通过 ranch 装好的命令使用它，地位对等。

设计和当前决定以 `docs/DESIGN.md` 为准。实现中要改设计，先改那份文档，并在提交说明里写清楚。

## 先读

- `docs/DESIGN.md`：范围、与两个前端的关系、安装方式、兼容规矩、步骤。
- `HANDOFF.md`：现在在哪、下一步、悬着什么。
- 来历和用户原话在 paddock 仓库：`../paddock/docs/背景与决策记录.md` §6g–§6j，`../paddock/docs/DESIGN.md` §2、§3。

## 规矩

- **语言与依赖**：Rust stable；只用成熟、活跃维护的库，依赖版本由 ranch 自己决定，升级作为单独任务。实现、测试和辅助工具不用 Python，工具用 Rust 或纯数据文件（从 Saddle 迁入的 `crates/corral/tests/collectors.mjs` 是 Node 写的手动测试，照原样保留）。不依赖 ratatui、crossterm。
- **前端只用公开约定**：Saddle、paddock 只调用 ranch 装好的命令（如 PATH 上的 `corral`），读它的 JSON 输出；不在 Cargo 里引用 ranch 的包，不读 ranch 的内部状态目录。
- **改动要写明对两个前端的影响**（paddock DESIGN §3“Saddle 保底”）：
  - 只改内部实现：前端不用动。
  - 公开约定（命令、参数、JSON 输出、退出码、`~/.corral` 格式、技能内容）有兼容的新增：写说明交用户决定 Saddle 跟不跟。
  - 不兼容：先尽量改成兼容；做不到就先写好前端怎么适配，交用户/Saddle 主控，定好次序再合并。
  - 过渡期可能有旧版 corral 同时操作同一批 agent，兼容要双向。
- **不改别的仓库**：不改 Saddle 仓库；paddock 的改动在 paddock 自己的任务里做。前端要改的写成需求交用户。不碰旧的 Python corral 仓库（`../corral`）。
- **安装**：程序放进不可变版本目录 `~/.local/share/ranch/versions/<提交>/`，不覆盖、不删除仍被运行中的 agent 或 hook 引用的程序。打包只生成目录；切换 `~/.local/bin/corral`、安装技能、`corral upgrade` 都是单独的部署操作，在用户在场时做。
- **不要干扰用户正在用的 agent**：`corral ls` 里现有的 agent 都是用户的。可以 `corral ls/status/reply` 读；不要对它们 `stop`、`send`、`keys`、`upgrade`，也不要 attach 上去打字。需要真实 agent 实测时，自己开一个 `ranch/test-<名字>`（`--label role=test`），用完 `corral stop`。
- **不按名字批量杀进程**：不要用 `pkill -f corral` 这类命令。停自己起的进程用记下的 PID。
- **测试不依赖真实 agent 和用户状态**：用隔离的 HOME、CORRAL_HOME 和合成程序（如 `/bin/cat`）。
- **验证**：小改跑直接相关测试；跨模块改动和合并前跑 `cargo test --all-targets`、`cargo clippy --all-targets -- -D warnings`、`cargo fmt --check`。保留有价值的测试，不靠删测试、放宽断言或缩短超时来通过。
- **迁入的代码**：开头注明来源（Saddle 提交、原文件）。会编进程序或装到用户目录的资源文件不加注释，来源写在所在包的 README。
- **独立编译目录**：命令前加 `CARGO_TARGET_DIR=$HOME/Developer/personal_projs/ranch-worktrees/.target/<子目录>`，主仓库用 `main`，任务 worktree 用分支名。不和 Saddle、paddock 共用。清 worktree 时一并删掉它的子目录。

## 开发方式

- 由 paddock 主控（`paddock/main`）兼管、自己实现（用户 10-05）。
- 每件活先在 `docs/任务/` 写任务文件（做成什么、范围、怎么算做完），给用户看过再动手。验收照抄用户原话，不补验收点；主控觉得该加的，列出来问用户。
- 每件活一个分支，worktree 放 `../ranch-worktrees/<分支>`。
- 合并前自查，在任务文件末尾写「完成记录」：做了什么、验证了什么、拿主意的地方、没做的事。
- 合并：本地合并进 main，推送到 `origin`（`github.com/firegnu/ranch`，public，不加许可证）。推送前用 gitleaks、trufflehog 查，确认没有密钥、截图和私人数据。
- 收尾记号：一件活合并完、worktree、分支和编译子目录清干净后，在 main 上补一条空提交，首行写「收尾: 」加一句话说明这件活。
- 收尾之后更新 `HANDOFF.md`。设计和理由进 `docs/DESIGN.md`。
