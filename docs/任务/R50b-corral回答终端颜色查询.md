# 任务：R50b corral 在没有客户端连着时回答终端的前景色、背景色查询

2026-10-24，ranch/main 交给 ranch/dev-r50b（Codex，碰要害：gpt-6-astra / xhigh）；交叉审查交 Claude Code（opus[1m] / xhigh）。
路由：重 / 交叉审查要 / 影响面：碰要害（路由：重（置信 1.0）、要、碰要害）
类型：功能变更
依据：照 R50a 定下的方案实现 `corral terminal-colors` 和 pen 回答 OSC 10/11；不答别的查询，不改 paddock。
提示：围绕已确认的使用目标完成变更，优先沿用现有机制。
你是被委派的 agent：照本文件做，不要再开别的 agent（下面写明的测试 agent 除外）。

## 用户原话

- 10-24，经 paddock/main 转：「发给 ranch 主控,先不做，先新建一个draft task」。
- 10-24：「R50 先做吧」。先出方案（R50a，已合并：`docs/调研/R50-corral回答终端颜色查询.md`），用户定了 §6 三条：颜色只用本机设置；只答 OSC 10/11；有客户端连着但它不回答时第一步不处理。
- 10-24，主控问验收，用户选：报告 §5 列的测试都有，加真 Codex 看一次（见「怎么算做完」）。

## 先读

- `docs/调研/R50-corral回答终端颜色查询.md` 全文，重点 §3、§4、§5、§6。
- `docs/CONTRACT.md` 的 `start`、`attach`、`upgrade`；`docs/DESIGN.md` 里 pen 的部分。

## 在哪里干活

- worktree：`/Users/firegnu/Developer/personal_projs/ranch-worktrees/r50b`，分支 `r50b`（已从 main 建好）。只动 `crates/corral/`、`docs/CONTRACT.md`、`docs/DESIGN.md`，以及本任务文件末尾的记录。
- 编译目录：`CARGO_TARGET_DIR=$HOME/Developer/personal_projs/ranch-worktrees/.target/r50b`。

## 要做的

照报告的推荐方案实现：

1. 新命令 `corral terminal-colors [--fg COLOR --bg COLOR | --clear]`，设定、查看、清除本机颜色设置（存 `$CORRAL_HOME/.runtime/terminal-colors.json`），输出和错误照报告 §4 的约定草案。
2. `corral start` 起 pen 时读一次这个设置（坏文件当没设置，`start` 照常成功、带 warning）；agent 输出里出现 OSC 10/11 查询（BEL 或 ST 结尾）的那一刻，如果没有客户端连着且有颜色，pen 立刻用这组颜色回答（结尾和查询一致）；有客户端连着就照旧转给客户端、自己不答。别的终端查询一概不答。没有设置时行为和现在一样。
3. 回答不排在带延时的 `send` 后面，也不插进正在写的块中间；不算人在打字。
4. `corral upgrade` 交接带上颜色，新旧版本双向兼容（报告 §3.3，`SCHEMA` 不变）。
5. 文档：CONTRACT 加报告 §4 那段（含对 paddock 的影响）；DESIGN 加一小节；`crates/corral/README.md`。

## 怎么算做完

- 报告 §5 列的测试都有：`terminal.rs` 认查询（两种结尾、分两次到、设置形式和别的 OSC 不算）；有设置、没有客户端时程序很快收到一模一样的回答；没有设置时收不到；有客户端 attach 着时 pen 不答、客户端收到原样查询；回答不排在带延时的 `send` 后面、不插进正在写的块中间；CLI 设、查、清和各种 `usage` 错误，坏设置文件时 `start` 照常成功带 warning；升级旧到新、新快照交接后颜色还在。
- 真 Codex 看一次：隔离的 CORRAL_HOME 里 `corral terminal-colors` 设成浅色（例如 `--fg '#4c4f69' --bg '#eff1f5'`），用新编出的 corral 后台开一个真 Codex（`ranch/test-r50b-codex`，`--label role=test`，cwd 用 worktree 外的临时目录，不 attach、不发任何话），`corral read` 看到浅色的输入框底色（报告里的例子 `48;2;229;231;235`），再 `corral stop`。对照：不设颜色时是深色。
- 验证只做这些：上面这些测试（先写测试，确认在现在的代码上失败）；`cargo test --all-targets --no-fail-fast`、`cargo clippy --all-targets -- -D warnings`、`cargo fmt --check` 各跑一次；真 Codex 那两次。可以加边角用例：查询正好在客户端 attach、detach 的同时到；一次输出里有两条查询。觉得不够，在回复里说，不要自己加。

## 真 Codex 的边界

- 只启动、不发任何话，不用模型额度；Codex 用读表的默认命令时会带 `--yolo`，这里不需要，用 `-- codex` 即可，不指定模型。
- 用户的真实 Codex 配置会被读到（登录态），但不改任何配置；信任目录的框不点「信任」，看到输入框颜色就停。
- 用隔离的 CORRAL_HOME，不碰用户的 `~/.corral`；用新编出的 corral 二进制（编译目录里的），不用 PATH 上装好的。

## 不要做

- 不答 DA1、CPR 和 OSC 10/11 以外的任何查询；不加 `corral start` 参数或环境变量作为颜色来源；不做「客户端连着但不回答」时的补答。
- 不改 paddock 仓库；paddock 要调新命令的部分由主控交用户。
- 不改 PATH 上装好的 `corral`、`ranch`，不打包、不部署，不装技能，不切共用 helper。
- 不读、不改用户真实的 `~/.corral`、`~/.ranch`、`~/.pony`。测试用隔离的 HOME、CORRAL_HOME 和合成程序（如 `/bin/sh`、`/bin/cat`）。
- 不碰 `corral ls` 里已有的 agent（都是用户的），不 `stop`、`send`、`keys`、`upgrade`，也不 attach。只操作自己开的 `ranch/test-r50b-*`。
- 不按项目名或路径批量杀进程（`pkill -f corral`、`pkill -f codex` 这类）：主控和别的 agent 的进程命令行里都带着项目名和工作目录，一条命令能把它们全杀掉；用户自己也开着 codex。停自己起的进程用记下的 PID。
- 不放宽现有测试的断言、不缩短超时、不删测试来过。
- 碰到要拿主意的事（包括要改公开格式里已有字段的含义、要改 `SCHEMA`），停下来，在回复里写清问题、可选办法和你的建议，然后结束这一轮，等主控回复再接着做；不在自己窗口里问用户，不开着命令等，不要自己换别的办法。主控的决定写进完成记录。
- 不合并到 main，不推送。只在 `r50b` 上提交。

## 做完

在本文件末尾追加「## 完成记录」（在你的分支里提交）：做了什么、验证了什么（含真 Codex 两次的结果）、拿主意的地方、没做的事、对 paddock 的影响、踩过的坑，各几句话。踩过的坑指试了不行、换了办法的，工具或环境的行为和预想不一样的；没有就写「没有」，不要凑。回复里只写这几样，加上有没有要主控决定的事。命令都在前台跑完，全部做完后，回复最后一行写 DONE。

## 完成记录

2026-10-24，ranch/dev-r50b。

- 做了什么：新增 `corral terminal-colors` 的设、查、清和帮助；颜色校验、规范成小写，设置原子写入 `.runtime/terminal-colors.json`。start 在起 pen 前读一次，坏文件只加 warning；pen.log 记本次颜色。pen 只在解析查询时没有 writer 且有颜色的情况下回答 OSC 10/11，保持 BEL/ST 结尾。回答用独立队列，越过 send 块间延时，当前块写完后才插入，不计入人类输入或 recent。补全 CONTRACT、DESIGN 和 corral README。
- 验证了什么：先写 CLI、终端解析和 PTY/升级行为检查，分别确认旧行为报未知命令、没有查询结果、没有回答／warning，升级后也没有颜色回答；随后实现并通过。覆盖两种结尾、所有切分点、设置形式及其他 OSC/DA1/CPR 排除，同批两条查询，无设置、attach 原样转发，250 毫秒内精确回答，3 秒 send 延时和 2 MiB 部分写入的顺序，坏设置 warning，旧快照默认值，新快照待写回答／输入进度，以及真实 pen 升级保留启动时颜色和半截查询。`cargo test --all-targets --no-fail-fast`、`cargo clippy --all-targets -- -D warnings`、`cargo fmt --check` 各执行一次，全部通过；未额外运行仓库中依赖外部环境的 ignored 测试，新增 ignored 合成程序由 PTY 测试启动。
- 真 Codex 两次：使用本分支编出的二进制、各自隔离的 CORRAL_HOME 和 worktree 外临时 cwd，名字均为 `ranch/test-r50b-codex`、标签 `role=test`，命令 `-- codex`，没有发话、attach、指定模型或操作信任框。浅色 `#4c4f69/#eff1f5` 时输入框底色 `48;2;229;231;235` 出现 4 次、深色绿色出现 0 次；未设置时该底色 0 次、深色绿色 `38;2;171;223;167` 出现 2 次。两次各有一条 OSC 10、OSC 11 查询，attached 为 0、last_human_input 为 null、recent_sends 为空；都未出现信任框，最终 `corral stop` 成功、exit_code 为 0。CLI read 用于确认界面文字；ANSI 颜色从它使用的现有 pen read 请求读取，因为 CLI 会剥掉颜色。
- 拿主意的地方：沿用已有 Terminal.pending 保存半截查询，feed 直接返回本次查询，不增加解析持久状态；给输入块增加默认 false 的 started 字段，以区分未开始与部分写入。颜色和回答队列同样加 serde(default)，SCHEMA 仍为 1；不改变已有字段含义。没有需要主控另定的设计问题。
- 没做的事：没有改 paddock、技能、PATH 程序或共享 helper；没有打包、部署、合并、推送，没有操作用户已有 agent 或读取真实 `~/.corral`、`~/.ranch`、`~/.pony`。交叉审查留给主控安排。
- 对 paddock 的影响：兼容新增命令；后续在启动、主题变化和远端开 agent 前调用它，旧版 usage 忽略即可，现有 start／attach／JSON 无需修改。已运行 Codex 不随设置变化，需要重开。
- 踩过的坑：首次精确过滤用了测试名前缀，实际运行 0 项，已修正过滤并取得真实 RED。CLI read 会剥掉 ANSI，最初半截查询等待因此超时，改用现有 pen read 的原始字节后确认了升级缺失回答的 RED，真 Codex 也用同一只读方法观察颜色。嵌套快照测试的 json! 超过宏递归上限，拆成 pen 与外层两份 JSON，没有提高编译器限制。

2026-10-24 审查后补改（主控决定）：采纳审查意见第 1–3 条，第 4、5 条不改，审查文件只读。partial 合成 agent 改为收到完整 2 MiB 输入加回答后继续观察 200 毫秒，读取总上限 10 秒；父测试留 12 秒等读取及结果序列化，原逐字节相等和及时回答断言不变。新建 CORRAL_HOME、`.runtime` 使用与 hooks::helper 一致的递归 DirBuilder／0700，已有目录不改权限。只查时无效 JSON、字段或颜色返回 `invalid_settings`、退出码 1，信息带设置文件路径，CONTRACT 已说明；命令参数错误仍是 usage，start 原有 warning 处理不变。先补权限和坏文件测试，分别确认旧实现建成 0755、坏颜色误报 usage 的 RED，修正后 `cargo test -p corral-core --test protocol terminal_colors` 为 7 通过、1 个合成子程序入口跳过（由 PTY 测试实际启动）。按意见先编译 protocol，再同时运行三份 `--exact terminal_colors_reply_bypasses_send_delay_but_never_splits_a_chunk`，三份均通过（各约 6 秒）；修改前同样跑三份在当时负载下未复现超时，未加额外压力轮次。`cargo clippy --all-targets -- -D warnings`、`cargo fmt --check` 均通过，没有重跑全套或真 Codex。对 paddock：设置调用无需变更，只查坏文件可与旧版不支持区分；没有其他待主控决定的事。未修改审查文件、未合并或推送。
