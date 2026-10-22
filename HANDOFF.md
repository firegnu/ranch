# 交接

本文件只记照着执行的事实：本机和 5090 装的是哪一版、怎么退回，机器和环境的长期事实，还没派的草稿，已知没修的问题。只在这些变了时更新。

- 会话经过、待定的事、约定和经验由 cairn 记（接续摘要、项目的账、经验），开局自动注入；这里不重复。
- 设计和理由在 `docs/DESIGN.md`，公开格式在 `docs/CONTRACT.md`，每件活的经过在 `docs/任务/`。
- 2026-10-22 之前的版本（每版部署经过、R1–R40 的进度流水）在 git 历史里：`git log -p -- HANDOFF.md`。

## 本机安装

- 现装：`~/.local/bin/pony` 指 `versions/da45737`（10-22，R40a2）；`corral`、`ranch`、`fence` 三个链接指 `versions/2513b3a`（da45737、a6e594e 里这三个程序和它逐字节相同，没切）。退回：pony 链接切回 `versions/a6e594e/bin/pony`，再 `launchctl kickstart -k gui/$(id -u)/local.ranch.pony`。
- 上一版 `versions/a6e594e`（10-22，R40a）。再退回：pony 链接切回 `versions/2513b3a/bin/pony` 再 kickstart；新加进配置的绑定旧版也认。
- 旧版本目录都保留，不删。
- 技能：corral 技能（`corral install-skills`）和 corral-dispatch 技能（`ranch dispatch install-skills`）装在 `~/.claude/skills/`、`~/.agents/skills/`、`~/.gemini/config/skills/`；技能内容编进程序，改技能要先打新版本再重装。
- 全局插件：amp 在 `~/.config/amp/plugins/corral.ts`；agy 在 `~/.gemini/config/plugins/corral/`。两个 `integration status` 都是 same。
- pony：launchd 用户服务 `local.ranch.pony`（`~/Library/LaunchAgents/local.ranch.pony.plist`）；配置和凭证在 `~/.pony`（目录 0700、凭证 0600），直连 Slack 不用代理；工作区里只有用户一人；绑了哪些频道看 `pony status`。
- `~/.fence/local.toml`（10-19 用户同意、主控写）：放行 IP Guardian 的 4 个查 IP 网站，允许读 `~/.ip_guardian_bin`、`~/.ip_guardian_allowed`；不写的话墙里的 codex、claude 会被 IP Guardian 拦下（退出码 1）。

## 部署时要记得

- 部署由用户在普通终端里跑，主控事后核对。
- 共用的 `~/.corral/.runtime/helper` 指 `versions/aef2b4a/bin/corral`，切 bin 链接不会跟着换。要换时用一个 `/bin/cat` 测试 agent 单独 `corral upgrade`，不动用户的 agent。
- 部署前起的 pen 还是旧版，只对新开的 agent 生效；没跑过 `corral upgrade --all`。
- pony 每次换程序都要 `kickstart` 才生效；plist 定义变了要 `pony service install --yes`。
- fence 用的 node 是 `~/.nvm/versions/node/v24.13.1/bin/node`，这个版本不能卸；srt 0.0.80。
- 打包时主仓库有未跟踪的草稿，`BUILD.txt` 会记成 `working-tree: modified`，程序和提交一致。

## 5090（用户的 Linux 实测机器，主机名不进仓库）

- 代码在 `~/Develop/ranch`，在 main 上（远端设了 `receive.denyCurrentBranch updateInstead`，本机 `git push <host>:~/Develop/ranch main` 过去）。装的是 `versions/5de4583/`（R14，程序同 9683ab4）；代码已推到 R16 之后，R16 只改测试，没重新打包。退回：链接切回 `versions/53fe6fc/bin/`，重跑两份 `install-skills`。没装 cairn。
- 那台机器上只用 pi、omp，不跑 Claude Code、Codex，主控（Claude Code）经 ssh 去敲命令也不行（用户 10-19）；那边的活交给那台机器上的 pi、omp。
- 经非交互 ssh 装技能要先把 `~/.local/bin` 加进 PATH，否则 corral 技能会写成「corral 没装」。

## 仓库

- `origin` = `github.com/firegnu/ranch`（public，不加许可证）。`.gitleaks.toml` 只放行 ureq 的公开测试私钥。
- 前端 paddock 只调用 `corral`、`ranch`、`pony` 命令，进度看 paddock 的 HANDOFF。

## 还没派的草稿

- `docs/任务/R39-pony后续想法.md`：pony 后续各条，已按先后排好；排第一的是 `ranch deploy`，等用户定要不要先出设计。
- `docs/任务/R40b-pony-setup新机器一条命令装好.md`（未提交）：新机器上 `pony setup`，要给别的机器装 pony 时再做。
- `docs/任务/R17c-Linux上验sandbox.md`（未提交）：5090 上验 sandbox，暂缓（用户 10-19：「先不做R17c了，先把全部的在macos上跑通」）。
- `docs/任务/R17-agent放进sandbox.md`（未提交）：sandbox 的总任务书，R17a、R17b 已做完，剩 R17c。

## 已知没修的（不主动追，碰到再看）

- srt：文件拦截日志会漏条（Codex、pi 写主仓库被拦没进 `fence denials`，只在终端可见）；Codex 要开 `enableWeakerNetworkIsolation`；Claude Code 要读 `~/Library/Keychains`；worktree 提交成功但报 `packed-refs.lock` 被拒，原因没查清；Linux 上墙里的 unix socket 只能全拦（R17c 时再定）。
- `~/.cache` 是指向 HOME 外的符号链接时，墙里连临时目录都建不了（R20 审查；用户的不是链接）。
- R18：旧运行的慢 `child-sandbox` 设定请求遇到同名重开，复核和写入之间还剩很小的窗口（CONTRACT 写明了）。
- corral 对 Codex 在同一轮里会短暂报 idle，派活时提醒会提前到。
- 全套测试偶有 1 个失败、再跑就过（hook 协议测试的偶发超时；`raw_v2_events_keep_original_input_reply_and_retry_grace_with_v1` 也失败过一次）。
- `corral start --help` 的用法行没列 `--sandbox`（CONTRACT 有）。
- agy 的已知限制见 DESIGN §7「接入 agy」。
