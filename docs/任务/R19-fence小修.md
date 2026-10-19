# 任务：fence 小修——启动失败原因留得住、拦截按类型计数、zsh 临时文件、两处 CONTRACT 改准，查 count-objects 的 garbage

10-19，ranch/main 交给 ranch/dev-r19（Codex，gpt-6-astra / medium）。
路由：常规 / 交叉审查不要 / 影响面：改行为（路由：常规（置信 0.55）、拿不准、拿不准；后两项主控按规则定：不放宽墙，`TMPPREFIX` 指到墙已给的临时目录，其余是记录、计数和文字）
类型：功能变更
依据：10-19 paddock/main 做 P5-120、P5-140 时碰到的几件 fence 的事，ranch/main 核过后列成这一件，用户同意（经 paddock/main 转达：“按你的建议转给ranch吧”），排在 R18 之后。本轮改 fence 和 CONTRACT 里 fence 那一节；第 5 项只查不修。不改 corral 的行为。
提示：围绕已确认的使用目标完成变更，优先沿用现有机制。
你是被委派的 agent：照本文件做，不要再开别的 agent。

## 先读
- `docs/CONTRACT.md`「sandbox 与 `fence`（R17b）」一节：`fence denials`、记录目录、内部 `run` 的退出码 125、墙内 `FENCE_RUN` 那几段。
- `docs/DESIGN.md` §8。
- `crates/fence/src/runs.rs`（`run`、`denials`）、`crates/fence/resources/runner.mjs`（失败时写 `exit.json` 那段）、`crates/fence/data/profiles/dev.toml`、`crates/fence/tests/`。
- `CLAUDE.md`「规矩」全部。

## 要做的
1. **墙启动后才失败时，原因留得住**。现在 runner 或 `fence run` 自己失败（退出码 125）时，原因只写到 agent 的终端里；agent 一退出，`corral ls` 清掉目录，就再也读不到（ranch/main 10-19 用假 fence 复现过）。要做到：
   - 这种失败时，运行记录的 `exit.json` 多一个 `message`，`fence denials` 的 `exit` 里照原样给出，例如 `{"code":125,"signal":null,"t":…,"message":"…"}`。
   - `message` 只放 fence、runner、srt 自己的错误说明，不放 agent 命令和首句（CONTRACT 已有的规矩）。
   - agent 自己退出（包括它自己以 125 退出）时不加 `message`。
   - CONTRACT 里「原因在终端中，可用 `corral read` 读取」改成实际情况：原因在 `fence denials` 的 `exit.message`；agent 退出后 `corral read` 读不到。
2. **`fence denials` 按类型给条数**：
   - 输出多一个 `counts`，按 `type` 数这次运行全部记录的条数，不受 `--limit` 影响。四个已知类型 `network`、`file-read`、`file-write`、`other` 总在、没有为 0；认不出的类型按原值另起一个键。
   - `--limit 0` 只要条数：`denials:[]`，`counts` 照给。
   - `truncated:true` 时，`counts` 只数已经记下的。
   - 卡片数字从这里取，不用再拉全量（paddock 10-19 的需要）。
3. **zsh 的 heredoc 临时文件**：zsh 写 heredoc 用 `TMPPREFIX`（默认 `/tmp/zsh`），不看 `TMPDIR`，所以在墙里 `cat <<EOF` 会因写 `/private/tmp/zsh*` 被拦（paddock 10-19 实测，Claude Code 写提交说明失败）。在 `dev` 档案的 `env` 里加上 `TMPPREFIX`，指到墙的临时目录 `${tmp}` 下。
4. **CONTRACT 写准墙里调 corral 的结果**：现在写的是「再调 corral 用户命令报 `sandbox`，退出码 6」。实际上 PATH 上的 `~/.local/bin/corral` 在 HOME 禁读里，shell 常常报找不到或没权限（127 一类），走不到报 6 的那道检查（paddock 10-19 实测）。改成：通常报 127；corral 程序读得到时报 `sandbox`、退出码 6；两种都派不了活。不为了凑出 6 去放开 HOME。
5. **只查不修：worktree 里 `git count-objects -v` 报 `garbage found: <主仓库>/.git/worktrees/<wt>/refs`**（paddock 10-19 在墙里提交后看到）。查清：是不是 fence／srt 造成的（比如墙外为放行预先建的目录、墙里留下的东西），墙外不套墙的 worktree 会不会也这样，有没有害处。结论写进完成记录，附直接依据；要改的写出建议改法，本轮不改。
6. CONTRACT 照上面改；都是兼容新增或文字改准，写明对 paddock 的影响（P5-120 照约定忽略新字段，不改也能用；想省掉拉全量，在 paddock 自己的任务里改用 `counts`）。DESIGN 有要改的在完成记录里写，不要自己改。

## 在哪里干活
- worktree：`../ranch-worktrees/r19-fence-fixes`，分支 `r19-fence-fixes`（已从 main 建好）。
- 编译目录：`CARGO_TARGET_DIR=$HOME/Developer/personal_projs/ranch-worktrees/.target/r19-fence-fixes`。
- 会动：`crates/fence/`、`docs/CONTRACT.md`。

## 怎么算做完
- 用户同意的范围（10-19：“按你的建议转给ranch吧”），即上面 1–6。
- 验证只做这些：
  - 第 1–3 项各一条针对的测试，先写、确认它因为现在的问题失败（隔离的 HOME、FENCE_HOME，合成程序）。第 3 项的 heredoc 要在真 srt 里验，加进 `crates/fence/tests/srt.rs` 那组 `#[ignore]` 测试，本机跑一次。
  - 标准测试跑一次：`cargo test --all-targets`、`cargo clippy --all-targets -- -D warnings`、`cargo fmt --check`。
  - 第 5 项：在隔离的合成仓库里，墙里和墙外各提交一次，对比 `git count-objects -v`。
  - 觉得不够，在回复里说，不要自己加。

## 不要做
- 不改 corral（`crates/corral/`）的行为，不改技能，不改 paddock。
- 不放宽墙：不为了第 4、5 项放开 HOME 或 `.git` 的读写。
- 不往真实的 `~/.fence/` 写东西（测试用隔离的 `FENCE_HOME`），不动用户的 `~/.fence/local.toml`。
- 不部署：不切 `~/.local/bin` 的链接、不装技能、不往 `~/.local/share/ranch/versions/` 打包、不 `corral upgrade`。
- 不碰用户现有的 agent（`corral ls` 里不是你开的）：不 `stop`、`send`、`keys`、`pause`、`upgrade`、`child-sandbox`，不 attach 打字。要起真实 agent 验证时，自己开 `ranch/test-r19-<名字>`（`--label role=test`、`--env CAIRN_DISABLE=1`），用完 `corral stop`。
- 不靠删测试、放宽断言、缩短超时来通过。
- 不要按项目名或路径批量杀进程（`pkill -f corral`、`pkill -f ranch` 这类）：主控和别的 agent 的进程命令行里都带着项目名和工作目录，一条命令能把它们全杀掉。停自己起的进程用记下的 PID 或 `corral stop`。
- 拿不到 srt 自己的失败原因（比如 srt 的库接口不给错误说明），停下来报告，不要自己换别的办法。
- 不合并到 main，不推送。只在 `r19-fence-fixes` 上提交。

## 做完
在本文件末尾追加「## 完成记录」（在你的分支里提交）：做了什么、验证了什么、拿主意的地方、没做的事、踩过的坑，各几句话；第 5 项的结论单列。踩过的坑指试了不行、换了办法的，工具或环境的行为和预想不一样的；没有就写「没有」，不要凑。回复里只写这几样，加上有没有要主控决定的事。命令都在前台跑完，全部做完后，回复最后一行写 DONE。

## 完成记录

- 做了什么：fence 执行 runner 失败及 runner 捕获的自身/srt 错误写入 `exit.json.message`，`denials` 原样返回；agent 自己退出 125 不加 message。`denials.counts` 统计全部已记录类型，固定四键补零、未知类型保留，支持 `--limit 0`。dev 增加 `TMPPREFIX=${tmp}/zsh`，不放宽路径。CONTRACT 改准错误读取位置、墙内 corral 的 127/6，说明 paddock P5-120 可忽略兼容新增字段，采用 counts 在前端自己的任务里做。
- 针对验证：三项各新增一条测试，均先得到目标缺陷的 RED，再改为 GREEN。错误测试先因 `exit.message` 缺失失败，最终覆盖 srt 初始化错误、fence exec 失败、agent 自退 125 及命令/首句不落入记录；计数测试先因 counts 缺失失败，最终覆盖空记录、未知类型、limit 0/1/200、truncated。heredoc 测试放在 `tests/srt.rs` 的 ignore 组，本机用已安装的 srt 0.0.80（仅借用库，未修改安装）实跑：先报 `can't create temp file for here document: operation not permitted`，改后输出 `heredoc works`。测试使用隔离 HOME/FENCE_HOME 和合成程序；fence CLI 相关测试 12 条通过。
- 拿主意的地方：沿用现有错误捕获点，只保存运行时错误说明，不采集子进程 stderr；创建运行记录之前的错误仍走 stderr，CONTRACT 明示该边界。TMPPREFIX 用已有临时目录下的 `zsh` 前缀。没有改设计；DESIGN §8 原句“墙里调 corral 报 sandbox”建议主控同步为 CONTRACT 的“通常 127，能执行 corral 时 6”，按任务要求本分支未改 DESIGN。
- 标准验证：`CARGO_TARGET_DIR=$HOME/Developer/personal_projs/ranch-worktrees/.target/r19-fence-fixes` 下，`cargo test --all-targets`、`cargo clippy --all-targets -- -D warnings`、`cargo fmt --check` 各跑一次，全部通过；命令均在前台运行并等待退出。自查差异仅涉及 fence、CONTRACT 和本任务完成记录，`git diff --check` 通过。
- 没做的事：没有改 corral、paddock、技能或用户规则，没有部署、升级、合并或推送；没有修第 5 项，没有增加真实 agent 验证。
- 踩过的坑：错误测试的合成 srt 最初直接返回传入命令，和 runner 外层的 exec 拼成 `exec exec`，agent-125 分支得到 127；改为现有 runner 测试同样的 shell 包装后通过。此夹具错误不作为 RED 依据；此前 srt-error 分支已经因缺少 message 得到有效 RED。墙内 git 提交仍会打印已有的 `packed-refs.lock` 禁写错误，但提交成功，与本次 garbage 无关。

### 第 5 项：count-objects 的 garbage（只查不修）

结论：本机 Git 2.56.0 自身创建 worktree 的 refs 目录、又把它报成 garbage，不是 fence 的预建目录或 srt/墙内命令留下的垃圾。隔离合成仓库中先 `git init -b main`、空提交，再各建 outside/inside 两个 worktree；尚未运行 fence、尚未在 worktree 提交，两个 `git count-objects -v` 都已报 `.git/worktrees/<wt>/refs`，`garbage: 1`、`size-garbage: 0`。随后墙外和真 srt 墙内各空提交一次，均成功，两个结果仍为同样的告警和数字；墙外再次读 inside 结果一致。两处 refs 均为空目录。

直接源码依据（Git v2.56.0）：

- [`refs/files-backend.c` 的 `files_ref_store_create_on_disk`](https://github.com/git/git/blob/v2.56.0/refs/files-backend.c#L3425-L3465) 无条件创建 refs，只对 worktree 跳过 heads/tags 子目录。
- [`path.c` 的 `common_list` 和 `report_linked_checkout_garbage`](https://github.com/git/git/blob/v2.56.0/path.c#L344-L364) 将 refs 列为不忽略的条目，只要目录存在就上报。
- [`builtin/count-objects.c`](https://github.com/git/git/blob/v2.56.0/builtin/count-objects.c#L39-L50) 只打印 warning、增加计数和目录大小；最终返回 0，不删除内容。`size-garbage` 按 KiB 向下取整，所以显示 0 不表示目录本身占用为零。

影响：就这条空 refs 告警而言，是统计误报，不能据此判定对象损坏；实验中的两次提交均成功。建议保留目录，不在 fence 增加删除逻辑或放宽 `.git`；若要消掉告警，另行向 Git 上游反馈，让 linked-checkout garbage 检查识别合法的 worktree refs（并覆盖 worktree 专有 refs），或在确认新版修复后单独升级 Git。本轮未改。

## 主控审查

- 看了 diff 和完成记录：只动了 `crates/fence` 和 CONTRACT，1–6 项都做到，「不要做」里的一件没做。`exit.message` 只取 runner 捕获的错误和 exec 的系统错误，不带 agent 命令和首句。第 5 项结论（Git 2.56.0 自身误报，墙外新建 worktree 同样复现）证据充分，同意不改。
- 重跑标准测试：153 通过、0 失败、10 跳过（含新加的真 srt heredoc 测试），clippy、fmt 通过，和完成记录一致。
- 照开发建议同步了 DESIGN §8 墙内调 corral 的说法（127／6）。
- 结果：一次过
