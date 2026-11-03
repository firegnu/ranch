# 任务：R56c pony 按钮、派活表和技能接 canter

2026-11-03，ranch/main 交给 ranch/dev-r56c（Codex，中档「其余」：gpt-6-astra / high）。
路由：常规 / 交叉审查不要 / 影响面：改行为（路由：常规（置信 0.58）、拿不准、拿不准；主控按规则定：只是把 pony 现有的 Kimi 结构化核对扩到 canter、派活表和技能照 Kimi 加一家，拿不准时定改行为）
类型：功能变更
依据：本轮做 R56 第 9、10 条：pony 对 canter 的「拒绝」「打断」按钮和认标题，派活表加 canter 一家，corral-dispatch 和 corral 两份技能照改；corral 本体另有 R56b，不在本件。
提示：围绕已确认的使用目标完成变更，优先沿用现有机制。
你是被委派的 agent：照本文件做，不要再开别的 agent。

## 先读

- `CLAUDE.md`「规矩」。
- `docs/任务/R56-接入canter.md`：用户要的 10 条（本件做第 9、10 条）、这一版不做的。
- `docs/调研/R56a-接入canter方案.md`：**照它做**，重点第 9、10 节、第 13 节第 4 条、第 14 节「R56c」、末尾「用户 11-03 定的」。
- `docs/CONTRACT.md`「canter（R56）」：corral 对 canter 报的字段（`interaction`、`unsupported`、`unsupported_kind`），pony 和技能照它写；R56b 还没合并，真 corral 现在报不出这些。
- 照着做的先例：`docs/任务/R48d-Kimi-Code派活.md`（派活表和技能怎么加 Kimi）、`docs/任务/R48g2-pony在Slack上回答Kimi的提问和批准框.md`（pony 的 Kimi 结构化核对）。

## 在哪里干活

- worktree：`/Users/firegnu/Developer/personal_projs/ranch-worktrees/r56c`，分支 `r56c`（已从 main 建好）。只动 `crates/pony/`、`crates/dispatch/resources/`、`crates/corral/resources/`（`SKILL.md`、`AGENT_USAGE.md`），以及这几处直接相关的测试和 README。
- 编译目录：`CARGO_TARGET_DIR=$HOME/Developer/personal_projs/ranch-worktrees/.target/r56c`。
- 并行：R56b 在分支 `r56b` 改 `crates/corral/src/`、`crates/corral/tests/`、`docs/DESIGN.md`、`docs/CONTRACT.md`。你不要动这几处。

## 要做的

1. **pony**（R56 第 9 条）：作为主控时，pony 对 canter 窗口的「拒绝」「打断」按钮能用：给 canter 按一次 Esc；能认出 canter 的终端标题（标题里带 `canter`，格式 `⠋ canter · <目录名>`，第一个字符是转圈动画）。canter 的框开着时每 2 秒重画一次，按方案第 9 节改用 Kimi 那套按 `interaction` 的结构化核对，不比较屏幕；有子 agent 的框或多个框时不按，提示回电脑处理。
2. **派活表**（R56 第 10 条）：`dispatch.toml` 加 canter 一家和同名格子，照 Kimi 的做法：只在用户或任务文件点名时派。别的 agent 委派出来的 canter 一律带 `--yolo` 开（用户 11-03：「如果其他agent委派出canter，要使用--yolo开出。」；canter 的 `--yolo` 是 Allow all、从不弹批准框，命令仍在 canter 自带沙箱里跑）。不分档、不选模型。
3. **技能**（R56 第 10 条）：用到的 corral-dispatch 技能说明一并照改，照方案第 10 节：家名单加 canter、启动表加一行、信任框那段写明 `--yolo` 不跳过 canter 的「是否信任」框，卡在这个框上时 agent 会一直是 starting，照现有的「卡在信任框」处理；canter 不能套墙，带 `--sandbox` 或主控设了 child-sandbox 时 `corral start` 报 `unsupported_kind`，不要去掉墙重试。corral 技能和 `AGENT_USAGE.md` 照方案第 10 节加 canter 的开法、不支持暂停和 `--sandbox`。canter 自带沙箱和 fence 分开写。

这一版不做：pony 的「允许」按钮、按请求号远程回答和批准、在 Slack 里显示 canter 的题目。别家 agent 在 pony 里的行为不变。

## 怎么算做完

- R56 第 9、10 条（用户原话见上）。真 corral 要 R56b 部署后才报得出 canter 的字段，pony 本件只用假 corral 测；最后由用户在场联调。
- 验证只做这些：一条针对 pony 的测试（用假 corral 喂 canter 的 status：单框时出按钮、按一次 Esc、要求 interrupted；子框或多框不按；标题 `canter · <目录>` 不当摘要），一条针对派活表的测试（`ranch dispatch cell canter` 返回 `canter --yolo`、标签为空），再跑一次 `cargo test --all-targets`、`cargo clippy --all-targets -- -D warnings`、`cargo fmt --check`。觉得不够，在回复里说，不要自己加。

## 不要做

- 不改 `crates/corral/src/`、`crates/corral/tests/`、`docs/DESIGN.md`、`docs/CONTRACT.md`（R56b 的范围）。
- 不改 canter、paddock 仓库；不读 `~/.canter`、`~/.pony` 的凭证。
- 不重启、不重装用户的 pony 服务，不连 Slack，不往 Slack 发消息；只用假 corral 和现有的测试替身。
- 不打包、不部署、不装技能（`install-skills`），不改 `~/.ranch/dispatch.toml`。
- 不碰 `corral ls` 里已有的 agent（都是用户的），不 `stop`、`send`、`keys`、`upgrade` 它们，也不 attach。
- 不按项目名或路径批量杀进程（`pkill -f pony`、`pkill -f corral` 这类）：主控和别的 agent 的进程命令行里都带着项目名和工作目录，用户的 pony 服务也在跑。停自己起的进程用起的时候记下的 PID。
- 碰到要拿主意的事（包括要偏离方案、要改 corral 本体才能做的），停下来，在回复里写清问题、可选办法和你的建议，然后结束这一轮，等主控回复再接着做；不在自己窗口里问用户，不开着命令等，不要自己换别的办法。主控的决定写进完成记录。
- 不合并到 main，不推送。只在 `r56c` 分支上提交。

## 做完

在本文件末尾追加「## 完成记录」（在你的分支里提交）：做了什么、验证了什么、拿主意的地方、没做的事、踩过的坑，各几句话。踩过的坑指试了不行、换了办法的，工具或环境的行为和预想不一样的；没有就写「没有」，不要凑。回复里只写这几样，加上有没有要主控决定的事。命令都在前台跑完，全部做完后，回复最后一行写 DONE。

## 完成记录

2026-11-03，ranch/dev-r56c 在 `r56c` 分支实现并自查，交主控审查。

- 做了什么：pony 对 canter 的拒绝、打断及线程回「停」复用 Kimi 的结构化单框核对，核对实例、轮次、框时间、人工输入和框内容，不比较重画的屏幕；仅按一次 Esc，须原轮 idle 且 interrupted 才报成功。子框、多框或缺 interaction 时不按，提示回电脑处理。摘要忽略带转圈、✓、✋ 的 `canter · <目录>` 标题。派活表新增 canter 家和同名点名格，命令 `canter --yolo`，无模型、强度、标签或专用环境变量；同步两份技能、AGENT_USAGE 和 pony README，写明信任框处理、暂停不支持，以及 canter 自带沙箱与 fence 的区别和不能去墙重试。
- 验证了什么：按任务只新增一条 pony 测试和一条派活表测试。先确认 RED：框重画后 pony 没发 Esc、派活表返回 unknown_cell；实现后两条均 GREEN。pony 测试覆盖拒绝／打断按钮、重画不影响操作、一次 Esc、idle 未 interrupted 时不报成功、子框／多框不按和标题过滤；派活表核对完整命令、空标签、默认模型／强度及不进入其他格子。既有格子数量断言随新增从 13 改为 14，未放宽断言。随后 `cargo test --all-targets` 一次通过（504 passed、0 failed、17 ignored），`cargo clippy --all-targets -- -D warnings`、`cargo fmt --check` 各一次通过，`git diff --check` 通过。命令均前台跑完，编译目录始终为 `.target/r56c`。
- 拿主意的地方：照 R56a 和本任务的既定方案执行，无偏离设计或需主控新增决定的事项。共享的 `structured_dialog` 只用于单框核对，保留 Kimi 专用的 `kimi_dialog` 门槛，避免把 canter 接进回答、允许和双 Esc 流程。
- 没做的事：未提供 canter 的允许、远程回答或结构化题目展示；未改 R56b 的 corral 本体、DESIGN、CONTRACT。未连 Slack、读凭证、操作用户 agent 或服务，未打包、部署、安装技能、改本机派活表、合并 main 或推送。真 corral 和 canter 联调仍待 R56b 部署后由用户在场进行。
- 对 paddock 的影响：pony 公开格式不变，paddock 不用动；派活表兼容新增 canter 家和格子，现有调用不变，前端若要增加展示由 paddock 自己的任务接。canter 能力和错误码仍以 R56b 的既定 CONTRACT 为准。
- 踩过的坑：没有。
- 需要主控决定的事：没有。

## 主控审查

2026-11-03，ranch/main。
- 实际开的是 `ranch/dev-r56c-1`，Codex gpt-6-astra / high。
- 看了 diff：只动了 pony、`dispatch.toml`、两份技能、`AGENT_USAGE.md` 和相关测试、README，没碰 R56b 的范围。canter 走 Kimi 的结构化核对（`structured_dialog`），框上、干活时都只按一次 Esc（按两次仍只给 amp 和 Kimi 的框），要求 `interrupted`；子框或多框不按；标题去掉 `✓`、`✋` 后 `canter ·` 开头的不当摘要；派活表 `canter --yolo`、标签为空、只在点名时派；技能写明信任框照「卡在信任框」处理、不能套墙、自带沙箱和 fence 分开。
- 主控重跑 `cargo test --all-targets`：504 通过、0 失败、17 跳过，与完成记录一致。
- 对方没有要主控定的取舍。合并等 R56b 审完一起合（pony 依赖 R56b 报的 `interaction`、`unsupported`），真实联调在部署后。
结果：一次过
