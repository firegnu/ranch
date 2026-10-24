# 任务：R48g2 pony 在 Slack 上回答 Kimi 的提问框、批准框，「拒绝」「停」能停下 Kimi

2026-10-24，ranch/main 交给 ranch/dev-r48g2（Codex，碰要害：gpt-6-astra / xhigh）；交叉审查交 Claude Code（opus[1m] / xhigh）。
路由：重 / 交叉审查要 / 影响面：碰要害（路由：重（置信 0.82）、要、碰要害）
类型：功能变更
依据：照 R48g1 方案和用户的 5 条决定，让 pony 能在 Slack 上回答 Kimi 的提问框、批准框，「拒绝」「停」能停下 Kimi；corral 给 Kimi 补兼容新增的只读字段。不碰真实 Slack，不部署。
提示：围绕已确认的使用目标完成变更，优先沿用现有机制。
你是被委派的 agent：照本文件做，不要再开别的 agent（下面写明的测试 agent 除外）。

## 用户原话

- 10-24：「1: 我觉得要做到现在已经接入的agent的所有功能（但是可以分阶段做，有几个阶段你要给我列一下）……」。第 4 阶段是 Slack 回答和按钮。
- 10-24：「R48g 先做吧」；主控问三件事，用户都选推荐：提问框、批准框都做，和 Claude Code 对齐，顺带处理后台提问漏掉、提问线程里「停」停不下来；先出方案，用户定了再实现；真 Kimi 实测方案、实现各 5–10 轮短任务。
- 10-24，R48g1 方案（`docs/调研/R48g-Kimi提问和批准框.md`）的 5 件事，用户都选推荐：
  1. 前台提问：两次 Esc 关框并停下这一轮，再把答案当文字送（同 Claude Code）；选项按钮也走这条路。
  2. 后台提问：Slack 列题目、给按钮；等它空下来，在框里按键答，按完核对 `resolved`。
  3. 批准框「拒绝」和停在框上时的「停」：两次 Esc，拒绝并停下这一轮。
  4. corral 改动 §5.3 a–e 都做（e 回退选择器报 `blocked` 也做）。
  5. 非 Bash 的批准第一版在 Slack 上只给「拒绝」；「允许这一次」只给 Bash 命令框。

## 先读

- `docs/调研/R48g-Kimi提问和批准框.md`：全文，重点 §1.1（按键）、§2、§3.4、§4、§5（方案、防误按、corral 要补的、Slack 样子）、§6（要改的地方和测试）、「未知」「用户的决定」。
- `docs/任务/R45-*.md`、`R47a`、`R47b`、`R47c`、`R47d` 的任务文件和审查：Claude Code 那套怎么做的、防误按怎么核对。
- `docs/CONTRACT.md` 里 `status` 的 `questions`、`command`、Kimi 的部分；`docs/DESIGN.md` §7 Kimi、§10 pony。

## 在哪里干活

- worktree：`/Users/firegnu/Developer/personal_projs/ranch-worktrees/r48g2`，分支 `r48g2`（已从 main 建好）。只动 `crates/corral/`（Kimi 适配器和 `status` 合并，见报告 §6）、`crates/pony/`、`docs/DESIGN.md`、`docs/CONTRACT.md`、pony README，以及本任务文件末尾的记录。
- 编译目录：`CARGO_TARGET_DIR=$HOME/Developer/personal_projs/ranch-worktrees/.target/r48g2`。

## 要做的

照报告 §5、§6 和「用户的决定」做：

1. **corral**：`status` 对 Kimi 兼容新增 §5.3 a–e：Bash 批准框的 `command`；后台提问的 `questions`（带 `background:true`）和 `state_started`；最近一次框的结果 `resolved`；非 Bash 批准框的 `action`；回退选择器开着时报 `blocked`。只从主会话 wire 读，不改钩子，不需要换共用 helper。新旧 corral 交替读同一个 agent 要能用（照 R48d 升游标版本）。
2. **pony 在 Kimi 上**：
   - 前台提问：线程里写答案、选项按钮，都同 Claude Code，只是关框按两次 Esc；提醒末尾那句换成 Claude Code 那句；
   - 后台提问：照报告 §5.4 的样子列题目、给按钮；在干活时排着，空下来再在框里按键答，按完核对 `resolved`，对不上就照实说；
   - 批准框：Bash 命令框给「允许这一次」（按 `1`，命令用 `status.command` 核对）和「拒绝」；非 Bash 只给「拒绝」，显示 `action`；「本会话都允许」一类永远不给；
   - 「拒绝」「停」：Kimi 停在框上（提问、批准、后台提问）按两次 Esc，干活中按一次，空闲不按；
   - 防误按照报告 §5.2：按之前核对、按之后确认、用户已在电脑上答过、过期和连点；
   - 旧 corral 没有新字段时，退回「回电脑处理」，不按键。
3. 文档：CONTRACT、DESIGN、pony README 跟着写，写明对 paddock 的影响（公开约定只有兼容新增）。

## 怎么算做完

- 用户定的 5 条（上面「用户原话」最后一条）都做到。
- 「顺带处理后台提问漏掉、提问线程里『停』停不下来」：后台提问在 Slack 上有题目、能答；Kimi 提问框和批准框上回「停」能停下这一轮。
- Claude Code、Codex、amp、agy、pi、omp 在 pony 里的行为不变。
- 验证只做这些：
  - corral、pony 各自针对的测试，照报告 §6 列的用例（合成 wire、假 corral、内存聊天适配器，隔离 HOME、CORRAL_HOME、KIMI_CODE_HOME、PONY_HOME），先写测试确认现在失败；边角：两个框同时开、旧游标升级和新旧交替重放、旧 corral 没有新字段、命令截断、危险命令；
  - 真 Kimi 在测试 agent 上按 pony 会送的按键序列走一遍：前台提问线程回答、前台多题、后台提问空闲时按键答（加一次「它在干活时开着后台提问」）、Bash 批准允许、批准拒绝、提问框上「停」，记下每条怎么看出来的；
  - `cargo test --all-targets --no-fail-fast`、`cargo clippy --all-targets -- -D warnings`、`cargo fmt --check` 各一次。
  觉得不够，在回复里说，不要自己加。

## 真 Kimi 实测的边界

- 额度：5–10 轮短任务，超了先停下来问主控。
- 只在自己开的测试 agent `ranch/test-r48g2-<用途>` 上做（`--label role=test`，cwd 用 worktree 外的临时 git 仓库，隔离的 `CORRAL_HOME`，普通模式不加 `--auto`，可以加 `--env KIMI_CODE_TRUST_WORKSPACE=1`，不指定模型），用完 `corral stop`。用本分支编出的 corral。
- Kimi 用用户的真实 `~/.kimi-code`，不改配置和凭证，不读、不往仓库里抄凭证、token、`device_id`、`server.token` 的内容。只让 Kimi 做无害的事，批准框只在临时目录里的命令上允许。
- 不碰真实 Slack：不发消息、不点按钮，不改 `~/.pony`，不动 pony 服务。Slack 上的真机实测由主控在部署后和用户一起做。

## 不要做

- 不给 Kimi 的非 Bash 批准框、危险命令、「本会话都允许」一类任何「允许」按钮。
- 不改 Kimi 的钩子、corral 的共用代码里和 Kimi 无关的部分、fence、dispatch；不升级依赖。
- 不改 PATH 上装好的程序，不打包、不部署，不装技能，不切共用 helper。
- 不碰 `corral ls` 里已有的 agent（都是用户的），不 `stop`、`send`、`keys`、`upgrade`，也不 attach。
- 不按项目名或路径批量杀进程（`pkill -f kimi`、`pkill -f corral`、`pkill -f pony` 这类）。停自己起的进程用记下的 PID。
- 不放宽现有测试的断言、不缩短超时、不删测试来过。
- 碰到要拿主意的事（例如报告「未知」里的情况实测和预想不一样、额度不够），停下来，在回复里写清问题、可选办法和你的建议，然后结束这一轮，等主控回复；不在自己窗口里问用户，不开着命令等，不要自己换别的办法。主控的决定写进完成记录。
- 不合并到 main，不推送。只在 `r48g2` 上提交。

## 做完

在本文件末尾追加「## 完成记录」（在你的分支里提交）：做了什么、验证了什么（真 Kimi 用了几轮、每条怎么看出来的）、拿主意的地方、没做的事、对 paddock 的影响、踩过的坑，各几句话。踩过的坑指试了不行、换了办法的，工具或环境的行为和预想不一样的；没有就写「没有」，不要凑。回复里只写这几样，加上有没有要主控决定的事。命令都在前台跑完，全部做完后，回复最后一行写 DONE。
