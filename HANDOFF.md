# 交接

本文件只记照着执行的事实：本机和 5090 装的是哪一版、怎么退回，机器和环境的长期事实，还没派的草稿，已知没修的问题。只在这些变了时更新。

- 会话经过、待定的事、约定和经验由 cairn 记（接续摘要、项目的账、经验），开局自动注入；这里不重复。
- 设计和理由在 `docs/DESIGN.md`，公开格式在 `docs/CONTRACT.md`，每件活的经过在 `docs/任务/`。
- 2026-10-22 之前的版本（每版部署经过、R1–R40 的进度流水）在 git 历史里：`git log -p -- HANDOFF.md`。

## 本机安装

- 现装：四个链接（`corral`、`ranch`、`fence`、`pony`）都指 `versions/03c265e`（10-24，R50b，`corral terminal-colors`、没有客户端时回答 OSC 10/11；只有 corral 程序变了）。Slack 应用已打开交互（Interactivity）。共用 helper 指 `versions/dd02e36/bin/corral`（R47c 改了钩子，记 Bash 权限框的原始命令，10-23 已切；R48c、R48d 没改钩子行为，不用换）。退回：`~/.local/share/ranch/versions/03c265e/bin/ranch deploy 8a6b239`（会自动重启 pony）。部署记录在 `~/.local/share/ranch/deploys.jsonl`。
- 上一版 `8a6b239`（10-24，R48e）、`5abf1f8`（10-24，R48d，Kimi 进派活表、题目给 pony）、`0232198`（10-24，R48c，Kimi 基本接入；本机 `~/.kimi-code/config.toml` 装了 corral 的标记块）、再上一版 `dd4d661`（10-23，R47c + R47d）、`dd02e36`（R47c）、`a04f7dd`（R47a + R47b）。本机派活表写了 `kimi` 的段时，退到 R48d 之前要先删掉这些段（见 CONTRACT）。再往前退回用 `versions/03c265e/bin/ranch deploy <版本>`，它会按程序内容列出要做的事；helper 退到 R47c（`dd02e36`）之前时，pony 拿不到原始命令，只是不给 Bash 的「允许」按钮，一般不用退 helper；退到 R43（`2d63ee1`）之前的版本时，helper 要另外用旧版 corral 对一个 `/bin/cat` 测试 agent 做 `corral upgrade`。
- 旧版本目录都保留，不删。
- 技能：corral 技能（`corral install-skills`）和 corral-dispatch 技能（`ranch dispatch install-skills`）装在 `~/.claude/skills/`、`~/.agents/skills/`、`~/.gemini/config/skills/`；技能内容编进程序，改技能要先打新版本再重装。
- 全局插件：amp 在 `~/.config/amp/plugins/corral.ts`；agy 在 `~/.gemini/config/plugins/corral/`。两个 `integration status` 都是 same。
- pony：launchd 用户服务 `local.ranch.pony`（`~/Library/LaunchAgents/local.ranch.pony.plist`）；配置和凭证在 `~/.pony`（目录 0700、凭证 0600），直连 Slack 不用代理；工作区里只有用户一人；绑了哪些频道看 `pony status`。
- `~/.fence/local.toml`（10-19 用户同意、主控写）：放行 IP Guardian 的 4 个查 IP 网站，允许读 `~/.ip_guardian_bin`、`~/.ip_guardian_allowed`；不写的话墙里的 codex、claude 会被 IP Guardian 拦下（退出码 1）。

## 部署时要记得

- 部署由用户在普通终端里跑 `~/.local/share/ranch/versions/<新版本>/bin/ranch deploy <新版本>`；主控先跑 `--dry-run` 给用户看计划，事后核对链接、`deploys.jsonl`、`pony status`。
- 共用的 `~/.corral/.runtime/helper` 指 `versions/dd02e36/bin/corral`（10-23 切），`ranch deploy` 不会跟着换。改了钩子代码（`hooks.rs`）的版本要换：部署后在普通终端跑 `corral start ranch/test-helper --label role=test -- /bin/cat`、`corral upgrade ranch/test-helper`、`corral stop ranch/test-helper`，不动用户的 agent。
- 部署前起的 pen 还是旧版，只对新开的 agent 生效；没跑过 `corral upgrade --all`。
- pony 换程序要 `kickstart`、plist 变了要 `pony service install --yes`：`ranch deploy` 会自己判断、自己做。它在重装服务时如果被 Ctrl-C 打断，重跑不会自动把服务装回来，要手动跑目标版本的 `pony service install --yes`。
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

- `docs/任务/R48f-Kimi进sandbox.md`（未提交）：Kimi 阶段 3，进 fence 的 sandbox；要用真 Kimi 在墙里实测定最小放行。用户 10-24：先建草稿。
- `docs/任务/R48g-Kimi题目在Slack上回答.md`（未提交）：Kimi 阶段 4（可选），pony 里回答 Kimi 的题目、加按钮，连带处理后台提问漏掉、提问线程里「停」停不下来两条已知问题。用户 10-24：先建草稿。
- `docs/任务/R51-stop与ls同时跑丢退出记录.md`：R48e 审查发现，`stop` 和同时跑的 `ls` 竞争丢退出记录；审查给了修法，改的是所有 agent 共用的 `stop` 等待。用户 10-24：记草稿，以后再做。
- `docs/任务/R39-pony后续想法.md`：pony 后续各条，按档排好，各条写了什么时候再做；手机上答确认框那条已由 R46、R47a–R47d 做完。
- `docs/任务/R40b-pony-setup新机器一条命令装好.md`（未提交）：新机器上 `pony setup`，要给别的机器装 pony 时再做。
- `docs/任务/R17c-Linux上验sandbox.md`（未提交）：5090 上验 sandbox，暂缓（用户 10-19：「先不做R17c了，先把全部的在macos上跑通」）。
- `docs/任务/R17-agent放进sandbox.md`（未提交）：sandbox 的总任务书，R17a、R17b 已做完，剩 R17c。

## 已知没修的（不主动追，碰到再看）

- srt：文件拦截日志会漏条（Codex、pi 写主仓库被拦没进 `fence denials`，只在终端可见）；Codex 要开 `enableWeakerNetworkIsolation`；Claude Code 要读 `~/Library/Keychains`；worktree 提交成功但报 `packed-refs.lock` 被拒，原因没查清；Linux 上墙里的 unix socket 只能全拦（R17c 时再定）。
- `~/.cache` 是指向 HOME 外的符号链接时，墙里连临时目录都建不了（R20 审查；用户的不是链接）。
- R18：旧运行的慢 `child-sandbox` 设定请求遇到同名重开，复核和写入之间还剩很小的窗口（CONTRACT 写明了）。
- corral 对 Codex 在同一轮里会短暂报 idle，派活时提醒会提前到。
- 全套测试偶有 1 个失败、再跑就过（hook 协议测试的偶发超时；`raw_v2_events_keep_original_input_reply_and_retry_grace_with_v1` 也失败过一次）。
- `crates/corral/tests/lifecycle.rs` 跑完会在 `/tmp` 留一个空的测试目录（`/tmp/.tmp…/pens/test/cat`，R50b 审查发现，没有进程在用）。
- `corral start --help` 的用法行没列 `--sandbox`（CONTRACT 有）。
- agy 的已知限制见 DESIGN §7「接入 agy」。
- `corral stop` 和同时跑的 `corral ls` 竞争：`ls` 可能先删掉刚停的 agent 的 `exit.json`，`stop` 输出 `exit_code`、`stopped_by` 为 null（Kimi 的 `remaining_pids` 也丢）；所有 agent 都会碰上，开着 paddock 时约 4%。修法草稿见 R51。
- Kimi（R48d 审查，可以不改的）：Kimi 的后台提问（`AskUserQuestion` 带 `background:true`）提问框开着、模型照样接着跑，pony 多半看不到这道题（不会给错题，只是漏）；这一轮结束后才回答时，状态会从 idle 回到 working，等下一轮结束才恢复。阶段 4 做 Slack 回答时用真 Kimi 测。详见 `docs/任务/R48d-审查.md` 第 4 条。
