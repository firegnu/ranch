# 任务：R47c 追加：corral 记下权限框的原始命令，确认框显示原始命令

2026-10-23，ranch/main 交给 ranch/dev-r47c-1（同一个 agent、同一个 worktree 和分支 `r47c`，接着 R47c 和第一次返工做）。
类型：功能变更
依据：R47c 交叉审查发现确认框里的命令是从读屏拿的，空格会丢（`rm -rf ~ /tmp/x` 显示成 `rm-rf~/tmp/x`），命令里夹假框头时还可能只显示后半截。用户定：让 corral 像 R43 记题目那样记下原始命令，pony 用它。
你是被委派的 agent：照本文件做，不要再开别的 agent。

## 用户原话

- 10-23，主控问「确认框里的命令会丢空格，怎么处理」，用户选：「corral 记原始命令 (Recommended)」。这个选项的说明是：「像 R43 记题目那样，让 corral 在权限框弹出时把原始命令记下来（钩子里本来就有，空格完整）。pony 的确认框显示这份原始命令，还要求它和屏幕上的框对得上。这样也能堵住审查说的『命令太长、真框头被挤出读屏范围』的残余风险。要改 corral 的钩子代码，需要你同意。R47c 先不上线，等这件做完一起上。」
- 用户 10-23 同时提醒：「手机上『允许这一次』的确认框里看到的命令会丢掉空格，比如 `rm -rf ~ /tmp/x` 可能显示成 `rm-rf~/tmp/x`，看上去像是只删一个子目录。这个也要注意下」。
- 这件是用户对「不要动 ranch 中 corral 的核心代码」的一次例外，只限下面「要做的」第 1 条。

## 先读

- `docs/任务/R43-corral认出框上被Esc和记下提问题目.md` 和 `crates/corral/src/hooks.rs` 里记 `questions` 的做法；`docs/CONTRACT.md` 里 `questions` 那一节（82–93 行附近）。
- 主仓库 `/Users/firegnu/Developer/personal_projs/ranch/docs/任务/R47c-审查.md` 的「## 审查意见」必须改第 1 条、建议改第 2 条。

## 在哪里干活

- 还是 `/Users/firegnu/Developer/personal_projs/ranch-worktrees/r47c`，分支 `r47c`，编译目录 `.target/r47c`。
- 这次可以动：`crates/corral/src/hooks.rs` 和 corral 里把这个字段带到 `status`／`wait` 输出所需的最少几处、corral 的相关测试、`docs/CONTRACT.md`；pony 照旧只动 `crates/pony/` 和 R47c 已经在动的文档。

## 要做的

1. **corral**：Claude Code 的 `PermissionRequest` 钩子里 `tool_name` 是 `Bash` 时，记下 `tool_input.command`，照 `questions` 的样子在 `corral status`、`wait` 的状态对象里给一个兼容新增的只读字段（建议 `command`：`{"text": "...", "truncated": false}`，上限照 `questions` 定 16 KiB，只保留完整 Unicode 字符的前缀）。只在当前状态 `blocked`、`last_tool` 是 `Bash` 时给，其他情况缺省或 `null`。只记这一个参数，不记别的工具、别的参数；钩子仍只观察，不批准、不回答。新旧 reader、新旧 helper 交替的兼容照 R43 的做法（旧 reader 忽略、新 reader 接受没有这个字段的旧事件）。CONTRACT 写清楚。
2. **pony**：Bash 的「允许这一次」只在这些都成立时给：`status` 里有这份原始命令、没截断，并且去掉所有空白后和屏幕上框里的命令一字不差。没有这份命令（旧 helper、字段缺省、截断）或对不上，就照「认不出」降级、保留「拒绝」。点击时的核对也加上这一条（原始命令和发提醒时相同）。
3. 确认框正文改用这份原始命令（空格完整）。超过 200 字时明确写出没显示全、共多少字、看不全请回电脑。第一次返工加的「读屏会吞掉空格，看不清请回电脑」那句，在显示原始命令时去掉。
4. 计划批准不变（这次不记计划正文）。
5. 文档：DESIGN §10、R38a、pony README 跟着改；第一次返工写进已知限制的那条残余风险（命令长到挤出读屏窗口），照现在的做法改写。对 paddock 的影响写进完成记录（预计：`status` 兼容新增字段，paddock 不用动）。

## 怎么算做完

- 用户原话那两条：确认框里看到的命令空格完整；命令和屏幕上的框对不上时不给按钮。
- 验证只做这些：
  - corral：钩子记录 `command` 的测试（Bash 有、别的工具没有、超长截断、旧事件没有这个字段）；用隔离的 HOME、CORRAL_HOME。
  - pony：有原始命令且对得上给按钮、确认框是原始命令；没有字段／截断／对不上（包括审查那个夹假框头的例子）不给；点击时原始命令变了不按。
  - 改动碰了 corral，最后 `cargo test --all-targets`、`cargo clippy --all-targets -- -D warnings`、`cargo fmt --check` 各一次。
  - 要核对 Claude Code 钩子真实传来的 `tool_input.command`，可以再开一个自己的测试 agent（`ranch/test-<名字>`，`--label role=test`，临时工作目录，`--env CAIRN_DISABLE=1`，`claude --model sonnet --permission-mode manual`），注意它的钩子要用你分支编出来的 corral（隔离 CORRAL_HOME，不切共用 helper），用完 `corral stop`。开之前先在回复里说打算怎么测，结束这一轮等主控回复。

## 不要做

- 不切换共用的 `~/.corral/.runtime/helper`，不部署，不动真实的 `~/.pony`、用户的 pony 和 Slack。
- 不记别的工具的参数，不改钩子的观察性质。
- 不升级依赖；不改 fence、dispatch。
- 不要按项目名或路径批量杀进程。
- 碰到要拿主意的事，停下来在回复里写问题、选项和建议，结束这一轮等主控回复。
- 不合并、不推送。

## 做完

在任务文件 `docs/任务/R47c-pony允许这一次按钮.md`（你分支里那份）末尾追加「## 追加记录（corral 记原始命令）」：做了什么、验证了什么、拿主意的地方、没做的事、踩过的坑。提交。回复里只写这几样和有没有要主控决定的事，最后一行写 DONE。
