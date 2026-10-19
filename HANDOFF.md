# 交接

2026-10-15 起本文件只记项目级的账：现状、下一步、悬着。每次会话做了什么、用户怎么说的，由 cairn 的接续摘要记，新会话开局自动注入；更早的会话经过在 git 历史里（`git log -- HANDOFF.md`）。

## 现在在哪

- 范围：第 1 步 corral、第 2 步 dispatch 都已完成，范围到此为止（用户 10-05：遥测、Drover、插件系统砍掉，不迁）。之后的活 R1–R16 都已收尾（R11 并入 R12 做完；R16 是改走派活后的第一件，Codex 做、一次过）；R17（sandbox，fence 接 srt，DESIGN §8）进行中：R17a 本机小试验、R17b fence 本体和 `corral start --sandbox` 都已收尾，R17b 已部署（10-19），任务文件、审查和完成记录在 `docs/任务/`，设计在 `docs/DESIGN.md`（R10 远端安装 §3；R1 暂停 §6；R3 适配器、R6 amp、R7 读屏、R8 agy、R13 agy 派活修整都在 §7），公开格式在 `docs/CONTRACT.md`。
- 安装：
  - `~/.local/bin/corral`、`~/.local/bin/ranch`、`~/.local/bin/fence` → `~/.local/share/ranch/versions/7fbb76d/bin/`（10-19 用户部署，R17b：fence 和 `corral start --sandbox`；工作区干净，srt 0.0.80，fence 用的 node 是 `~/.nvm/versions/node/v24.13.1/bin/node`，这个版本不能卸；技能没变，没重装；没跑 `corral upgrade --all`，R17b 的 pen 改动只对新开的 sandbox agent 起作用，`~/.corral/.runtime/helper` 仍是 `f86d1c2`）。部署后起 `ranch/test-sandbox`（claude，`--sandbox dev`）核对过：工作目录能写，写主仓库被拦，访问 example.com 被代理拒，两条都进了 `fence denials`；测完已停。退回：`corral`、`ranch` 切回 `versions/9683ab4/bin/`，删掉 `~/.local/bin/fence`。
  - 上一版 9683ab4（10-17 用户部署，R9：pen 和提醒进程启动时关掉继承来的描述符；只改内部实现，技能没变，没重装；部署后起测试 agent 核对过，新 pen 只开着锁、日志、pty、socket，没有多余管道；没跑 `corral upgrade --all`，部署前起的 pen 还是旧版）。打包时主仓库有两个未跟踪的任务草稿（R14、R15），BUILD.txt 记成 `working-tree: modified`，程序和 9683ab4 的代码一致。退回：两个链接切回 `versions/59705a4/bin/`。
  - 9683ab4 之前是 59705a4（R13：agy 状态兜底、corral 技能里 agy 免确认、pen 临时信任工作目录，部署后真实 agy 验收通过；技能三处都是 `same`）。退回到它之前：两个链接切回 `versions/9b69e4d/bin/`，重跑 `corral install-skills`。再上一版 9b69e4d（R12：corral-dispatch 技能换成分工表和新分档表，`ranch dispatch route` 用 jev-1.14.0；corral 代码和 `f86d1c2` 一样，没跑 `corral upgrade --all`，`~/.corral/.runtime/helper` 仍是 `f86d1c2`）。退回：两个链接切回 `versions/8299e39/bin/`，再跑 `ranch dispatch install-skills`。旧版本目录都保留，不删。
  - corral 技能（`corral install-skills`）和 corral-dispatch 技能（`ranch dispatch install-skills`）都在 `~/.claude/skills/`、`~/.agents/skills/`、`~/.gemini/config/skills/` 下。
  - 用户的 Linux 实测机器（主机名不进仓库）：代码在 `~/Develop/ranch`，在 main 上（远端仓库设了 `receive.denyCurrentBranch updateInstead`，本机 `git push <host>:~/Develop/ranch main` 过去）。装的是 `versions/5de4583/`（R14，10-17 用户让主控经 ssh 部署；程序同 9683ab4）。代码已推到 R16 之后的 main，R16 只改测试，程序没变，没重新打包；那边全套测试过、修好的回显测试 40 遍 0 失败，两份技能都装了，pi、omp 实测过，没有在跑的 agent。退回：链接切回 `versions/53fe6fc/bin/`，重跑两份 `install-skills`。没装 cairn。经非交互 ssh 装技能要先把 `~/.local/bin` 加进 PATH，否则 corral 技能会写成“corral 没装”。那台机器上只用 pi、omp，任何情况下不跑 Claude Code、Codex，pi、omp 不委派活（用户 10-15，暂时的）。
  - 全局插件：amp 装在 `~/.config/amp/plugins/corral.ts`（10-14 用户同意后装）；agy 装在 `~/.gemini/config/plugins/corral/`（10-15 用户从 paddock Integrations 装，测过能用）。两个 `integration status` 都是 same。
- corral 和 corral-dispatch 技能开出去的 agent 带 `--env CAIRN_DISABLE=1`，不用 cairn（R2）；cairn 只留给用户直接对话的主控。开出去的 Codex 还带 `--disable memories`，不读跨项目的 Codex 记忆（用户 10-16）。
- 没有进行中的活，没有任务 worktree 和分支；`../ranch-worktrees/` 下只有编译目录 `.target/main`。
- 前端：Saddle（`dedf26a` 起）和 paddock 都只调用 `corral`、`ranch` 命令。paddock 已接完 R1（P5-33）、R4（P5-94）、R6（P5-98）、R8（P5-88），R10 之后接了远程机器上的 agent（P5-90 甲，10-16 合并），进度看 paddock 的 HANDOFF。
- 远程：`origin` = `github.com/firegnu/ranch`（public，不加许可证）。`.gitleaks.toml` 只放行 ureq 的公开测试私钥。

## 用户交代、一直沿用的

- Saddle 基本不用了，不跟新功能（10-07：“saddle我基本不用了”）。
- agy 很少用，接它主要是为了读屏，它的已知限制不主动追（10-14：“主要是想通过agy加进来读屏幕的能力，agy很少用。”）。
- 部署（切链接、`install-skills`、`corral upgrade --all`、装全局插件）由用户在普通终端里跑，主控事后核对。

## 下一步

1. R17c：到 5090 上验 Linux（只用 pi、omp；先查系统版本，bubblewrap、socat、ripgrep 要 sudo 装）。paddock 可以开始 P5-120（照 CONTRACT 接 fence 命令和 sandbox 字段）。10-17 起 ranch 的活由主控派出去（R15，AGENTS.md「开发方式（主控分派）」）。

## 悬着

- R12 的 sonnet 试用：分工表第 8 条（说不清的界面小活）先给 Claude Code sonnet / high 试 5 件；满 5 件后主控把这 5 件的「结果」行汇总给用户，由用户定以后怎么派，定之前第 8 条的活先问用户。
- paddock 10-16 提的两件可选的事，都改公开约定，「甲」层不依赖，做不做用户还没定：`corral ls` 带上 status 的字段（或 `status --all`），远端每轮刷新从两次调用减到一次；`corral --version` 带构建号，前端能看出远端比本机旧。
- agy 的已知限制（DESIGN §7「接入 agy」，不主动追）：
  - 子 agent 的会话会不会触发钩子、抢主会话，没实测；
  - 钩子 shell 里 `exec` 失败时不回 `{}`；
  - Antigravity IDE 会不会加载插件，没核对；
  - R13：agy 运行中自己保存设置（如点信任）会不会拿旧内容冲掉 corral 加删的条目，没实测；登录慢时首句抢跑没复现（10 次 0 次），送法没改。
- paddock 的 `docs/DESIGN.md` §3“Saddle 保底”和决策表还写着“只保证 Saddle 和运行时对得上、不兼容时写需求给 Saddle 适配”；ranch 这边已按用户 10-07“保底规矩放松吧，Saddle 不用再写影响了”改了 AGENTS.md 和 DESIGN §2，paddock 那边要不要跟着改，在 paddock 自己的任务里做。
- R17a 记下的 srt 边界（用户 10-18：“不需要严格限制。我只是在要通过做这个来彻底完全的了解一下sandbox。”，不当成 R17b 的门槛）：Codex 要开 `enableWeakerNetworkIsolation`（trustd，削弱网络隔离）；Claude Code 要读 `~/Library/Keychains`、用 `CLAUDE_CODE_TMPDIR`；各家状态目录放得较宽；worktree 提交成功但报 `packed-refs.lock` 被拒，原因没查清；macOS 系统日志的拦截记录没整理成统一取法。
- R17b 留下的：srt 的文件拦截日志会漏条（Codex、pi 写主仓库被拦没记进 `fence denials`，只在终端可见）；全套测试偶有 1 个失败、再跑就过（像原有 hook 协议测试的偶发超时，没认出名字）；corral 对 Codex 在同一轮里会短暂报 idle，派活时提醒会提前到（没查）。`corral start --help` 的用法行没列 `--sandbox`（CONTRACT 有）。
