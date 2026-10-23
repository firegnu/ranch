# 任务：R47d pony：「允许这一次」认得出真实的 Claude Code Bash 权限框

2026-10-23，ranch/main 交给 ranch/dev-r47d（Codex，碰要害：gpt-6-astra / xhigh）；交叉审查交 Claude Code（opus[1m] / xhigh）。
路由：碰要害 / 交叉审查要 / 影响面：碰要害（路由：常规（置信 0.58）、要、碰要害）
类型：Bug 修复
依据：R47c 上线（`dd02e36`）后第一次真机实测，Bash 权限框没给「允许这一次」，只给了「拒绝」并写「认不出框的样子」。原因是识别表照 `ping` 那一个样本写死了，真实的框会因命令不同而换文字。本轮让 pony 认得出这类框，安全规矩不变。
执行：派出
你是被委派的 agent：照本文件做，不要再开别的 agent。

## 用户原话

- 10-23，看到 Slack 里只有「拒绝」按钮：「没有允许啊」。
- R47c 的验收（用户 10-22）：「『允许』按钮：只给 Claude Code 权限框『允许这一次』和计划批准」「『永远允许』一类都不给」。
- 底线：「不要动ranch中corral的核心代码。」（R47c 追加里改钩子是用户的一次例外，本件不再动 corral。）

## 现场

10-23 15:5x，测试 agent `ranch/test-allow`（Claude Code，sonnet，manual 模式）请求 `touch /tmp/pony-allow-test-r47c`。`corral status` 的 `command` 是 `{"text":"touch /tmp/pony-allow-test-r47c","truncated":false}`，钩子正常。`corral read --bytes 4000` 屏幕尾部（分隔线缩短了）：

```
Bash command
Tip: auto mode handles these prompts for you — choose "switch to auto mode" below
Create test marker file in /tmp
╌╌╌╌╌╌╌╌╌╌╌╌
touch/tmp/pony-allow-test-r47c
╌╌╌╌╌╌╌╌╌╌╌╌
Doyouwanttoproceed?
❯1. Yes
2. Yes,andalwaysallowaccessto/tmpfromthisproject
3. Yes,andswitchtoautomode · auto mode handles these prompts for you
4. No
Esc to cancel · Tab to amend
```

和 `crates/pony/tests/fixtures/claude-bash-permission.txt`（ping 样本）比，差两处：

1. 没有 `This command requires approval` 这一行；
2. 第 2 项是 `Yes, and always allow access to /tmp from this project`，不是 `Yes, and don't ask again for: …`。

`approvals.rs` 的 `recognize` 要求第 1 处那行必须在，`menu` 只放行写死的几种持久项文字，所以整框认不出。

## 先读

- `docs/任务/R47c-pony允许这一次按钮.md` 的完成记录、追加记录和主控审查；`docs/任务/R47c-审查.md` 的审查和复核意见（尤其是必须改第 1 条和整窗计数）。
- `crates/pony/src/core/engine/approvals.rs`：`RULES`、`recognize`、`menu`、整窗计数。

## 在哪里干活

- worktree：`/Users/firegnu/Developer/personal_projs/ranch-worktrees/r47d`，分支 `r47d`（已从 main 建好）。
- 编译目录：命令前加 `CARGO_TARGET_DIR=$HOME/Developer/personal_projs/ranch-worktrees/.target/r47d`。
- 只动 `crates/pony/`、`docs/DESIGN.md` §10 里 R47c 的识别规矩、`docs/调研/R38a-Slack机器人设计.md` 对应说明、pony README，以及本任务文件末尾。

## 要做的

1. Bash 框的识别改成按结构认，不按某一条命令的说明文字认：
   - 框头 `Bash command` 和第一条分隔线之间的行（Tip、命令说明）不管内容；
   - `This command requires approval` 可有可无；`Do you want to proceed?` 必须有；
   - 选项：只要一项和 `Yes` 一字不差、末项是 `No`，中间其他以 `Yes,` 开头的持久项不管写的是什么都可以出现（pony 只按 `Yes` 那一项自己的编号，从来不按它们）；出现不以 `Yes,` 开头的未知项照旧认不出。
   - 其余不变：原始命令比对（去空白后和框里命令一致）、整窗计数（多出的框头、分隔线不给）、不可见字符和长串空白不给、确认框、点之前的核对、只按一次。
2. 把上面的现场样本加成测试夹具，和 ping 样本一起用。
3. 计划框（Ready to code?、Exit plan mode?）这次不改。

## 怎么算做完

- 用户原话：上面这种真实的 Bash 权限框要给「允许这一次」，「永远允许」一类照旧不给、不按。
- 验证只做这些：
  - 针对这件的测试：现场样本给按钮、按的是 `1`；没有 `This command requires approval` 也认；第 2 项换成别的 `Yes, …` 持久项也认、但按的仍是 `Yes` 那项的编号；`Yes` 那项不在、或末项不是 `No`、或出现不以 `Yes,` 开头的未知项都不给；R47c 原有的批准用例都还过。
  - `cargo test --all-targets`、`cargo clippy --all-targets -- -D warnings`、`cargo fmt --check` 各一次。
  - 不用开测试 agent：现场样本已经在上面。部署和用户在场的真机实测由主控做。

## 不要做

- 不改 corral、fence、dispatch；不升级依赖。
- 不放宽原始命令比对、整窗计数、不可见字符和空白、核对、只按一次这些规矩。
- 不改真实的 Slack 应用设置、`~/.pony`，不重启用户的 pony，不部署；不对任何 agent `read`、`keys`、`send`、`stop`、`attach`。
- 不要按项目名或路径批量杀进程（`pkill -f ranch`、`pkill -f corral`、`pkill -f pony` 这类）。
- 碰到要拿主意的事，停下来，在回复里写清问题、可选办法和你的建议，然后结束这一轮，等主控回复；不在自己窗口里问用户。
- 不合并到 main，不推送。只在自己的分支上提交。

## 做完

在本文件末尾追加「## 完成记录」（在你的分支里提交）：做了什么、验证了什么、拿主意的地方、没做的事、踩过的坑，各几句话。踩过的坑没有就写「没有」。回复里只写这几样，加上有没有要主控决定的事。命令都在前台跑完，全部做完后，回复最后一行写 DONE。

## 完成记录

2026-10-23，ranch/dev-r47d，分支 `r47d`。

- **做了什么**：Bash 框改按结构识别，保留框头至第一条分隔线间任意 Tip／说明文字；`This command requires approval` 可有可无，`Do you want to proceed?` 仍必需。菜单接受任意 `Yes,` 开头的持久项，但只给唯一精确 `Yes` 的「允许这一次」并按它自己的编号，末项仍须为 `No`。可选提示仍按整窗计数，只能在命令后、问题前出现一次；多余框头、分隔线、提示及旧框仍拒绝。加入任务书原样的 touch 现场夹具，与原 ping 夹具一起回归。先补 DESIGN §10，再同步 R38a 和 pony README；两种计划框、原始命令比对、不可见字符／连续空白限制、确认框、点前核对和只按一次均保持原样。
- **验证了什么**：先运行现场夹具用例，确认因只出现一个「拒绝」按钮而失败（期望两个按钮、实际一个），修复后同一用例通过，确认正文保留原始 touch 命令，重复点击也只按一次编号 `1`。新增共 3 项引擎集成测试，覆盖可选提示有／无、不同说明和持久项文案、精确 `Yes` 位于第 1／2／3 项且高亮可停在持久项、缺少／重复 `Yes`、末项不是 `No`、未知项、缺问题行、重复可选提示；将现场夹具加入原有整窗额外标记测试。`cargo test -p pony --test core approval_` 的 23 项全部通过，包含 R47c 原有批准用例。随后 `cargo test --all-targets`、`cargo clippy --all-targets -- -D warnings`、`cargo fmt --check` 各一次通过；全仓 444 通过、0 失败、13 项原有忽略。`git diff --check` 通过。测试沿用临时隔离 Store、内存聊天适配器和假 corral；全部 Cargo 命令使用 `.target/r47d`，全部命令均在前台等到结束。全仓测试和 clippy 日志在 `/tmp/r47d-checks.bFvzVq`。
- **拿主意的地方**：按任务规定保留整窗限制；可选提示从必需标记改为单独计数，出现时仍须位于原来的位置，避免放过旧输出或命令里的同名标记。只扩展 Bash 的持久项识别，不把它们变成可批准项。没有需要主控决定的事。
- **对 paddock 的影响**：只改 pony 内部识别与说明，公开命令、JSON、状态格式均未变，paddock 不用动。
- **没做的事**：未改 corral、fence、dispatch、依赖或其他仓库；未操作真实 agent、Slack 设置、`~/.pony` 或用户的 pony，未开子 agent、测试 agent。未部署、合并到 main 或推送；交叉审查、部署和用户在场的真机实测由主控做。
- **踩过的坑**：没有。
