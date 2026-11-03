# 任务：R56b corral 本体接 canter

2026-11-03，ranch/main 交给 ranch/dev-r56b（Codex，碰要害「其余」：gpt-6-astra / xhigh；交叉审查交 Claude Code opus[1m] / xhigh）。
路由：碰要害 / 交叉审查要 / 影响面：碰要害（路由：重（置信 1.0）、要、碰要害）
类型：功能变更
依据：本轮照已定方案实现 corral 对 canter 的开、状态、回复、题目和命令、停、升级、不支持的明确报、`integration status`（R56 第 1～8 条）；pony、派活表、技能另有 R56c，不在本件。
提示：围绕已确认的使用目标完成变更，优先沿用现有机制。
你是被委派的 agent：照本文件做。除了下面写明的测试 agent 外，不要再开别的 agent。

## 先读

- `CLAUDE.md`「规矩」。
- `docs/任务/R56-接入canter.md`：用户要的 10 条（本件做第 1～8 条）、这一版不做的、规矩。
- `docs/调研/R56a-接入canter方案.md`：**照它做**，重点「结论先说」、第 1～8 节、第 13 节、第 14 节「R56b」、末尾「用户 11-03 定的」。
- `docs/DESIGN.md` §7「接入 canter」、`docs/CONTRACT.md`「canter（R56）」：方案定下来后主控写的设计和公开约定，实现要和它们一致。
- canter 的公开接口（只读这些，不读 canter 的别的源码当依据）：`canter --help`；`~/Developer/personal_projs/canter/docs/DESIGN.md`「接 corral、paddock、pony（P2-83）」一节；`~/Developer/personal_projs/canter/crates/canter-protocol/README.md`。测试里起假模型只参考 `crates/canter-server/README.md` 的 `config.json` 段、`crates/canter-model/README.md`。

## 在哪里干活

- worktree：`/Users/firegnu/Developer/personal_projs/ranch-worktrees/r56b`，分支 `r56b`（已从 main 建好）。只动 `crates/corral/src/`、`crates/corral/tests/`，需要时更新 `docs/DESIGN.md` §7「接入 canter」和 `docs/CONTRACT.md`「canter（R56）」里和实现对不上的细节。
- 编译目录：`CARGO_TARGET_DIR=$HOME/Developer/personal_projs/ranch-worktrees/.target/r56b`。
- 并行：R56c 在分支 `r56c` 改 `crates/pony/`、`crates/dispatch/resources/`、`crates/corral/resources/`（两份技能和 `AGENT_USAGE.md`）。你不要动这三处。

## 要做的

照方案第 14 节「R56b」实现，做到 R56 第 1～8 条：

1. **开**：`corral start <名字> ... -- canter ...` 能开 canter，首句、`--cwd`、`--env`、`--label` 照现有约定生效；名字、instance、labels 照 corral 现有规矩保存。知道这个 agent 对应 canter 的哪个会话（canter 在 `/new`、`/resume`、分叉、交接后会换会话，要跟着换）。
2. **状态**：`corral ls/status/wait` 对 canter 能正确报 starting / working / idle / blocked（等批准、等回答）/ 被打断，`interrupted` 照 CONTRACT 的规矩。状态来源用 canter 的快照和事件流（不靠钩子）；订阅和翻译放在 `agents/canter.rs`，corral 公共代码只加一个通用挂点（某一家可以带一个盯状态的后台线程，pen 负责起停）。
3. **回复、提问、待批准命令**：`corral reply` 给出这一轮最后的回复；被打断的一轮照 CONTRACT 报 no_reply。等回答时给出 `questions`，等批准跑命令时给出 `command`，照现有截断规矩。
4. **送话、按键、attach、read**：照现有方式经终端，行为不变。
5. **停**：`corral stop` 真正停掉 canter 在服务端上的工作（打断在跑的轮、停后台命令、关会话；会话留盘以后能恢复），不能只关掉终端就算停了。用户定：没确认关掉时仍 `ok:true`，带 `remaining_sessions`。
6. **升级**：`corral upgrade` 换 pen 后，canter 的状态订阅接着工作，不丢、不重报旧回复、不误判为停止；canter 服务端自己升级（`canter serve --replace` / `canter upgrade`）时也不误判为用户停止。
7. **不支持的明确报**：暂停 / 继续对 canter 报 `unsupported_kind`；`--sandbox` 的 fence 档案（显式或从父实例继承的）对 canter 报 `unsupported_kind`，不悄悄忽略、不退成无墙。
8. **`corral integration status`** 能列出 canter：程序、版本、`hooks:"launch"`、技能 target `codex`、`unsupported`。

别家 agent 的行为、输出逐字不变。不替用户记 canter 的目录信任（用户定），不调 canter 的 `DELETE /v1/sessions/{id}`。

依赖：方案推荐不加新依赖、手写最小的 HTTP/1.1 客户端（Unix socket 上的 GET、SSE、chunked）。要加依赖的话，停下来问主控。

## 怎么算做完

- 用户确认的范围是 R56 第 1～8 条（「其他我都没有异议，可以按照你的建议走」「这一版不做pause和fence，其他都会做」）。最后由用户在场联调，主路径：开、看状态、等批准、回复、送话、打断、停、升级 pen。
- 验证只做这些：每条要做的有针对它的测试（照方案第 13 节第 1、2 条：翻译的纯函数测试、用假 canter 服务端和假 `canter` 脚本的 pen 集成测试），加上边角：会话文件晚出现、`/new` 换会话、服务端换 socket 重连、升级前后不丢不重、线程停不下时升级失败、旧快照没有新字段报 `unknown`、stop 的 `remaining_sessions`、pause 和 sandbox（显式、继承）报 `unsupported_kind`、别家输出不变；方案第 13 节第 3 条的真 canter 测试（默认 `#[ignore]`，隔离 `CANTER_HOME` 和假模型）在本机手动跑一次；最后跑一次 `cargo test --all-targets`、`cargo clippy --all-targets -- -D warnings`、`cargo fmt --check`。不录屏。觉得不够，在回复里说，不要自己加。

## 不要做

- 不改 `crates/pony/`、`crates/dispatch/`、`crates/corral/resources/`（R56c 的范围）。
- 不改 canter、paddock 仓库；不读 canter 状态目录里 `server.json` 以外的文件；不读、不改用户真实的 `~/.canter`，不连用户正在跑的 canter 服务端。canter 公开接口做不到、要 canter 改的，停下来写成问题，不在 corral 里绕过去读 canter 的私有文件。
- 测试不调真实模型、不花钱；隔离 `HOME`、`CORRAL_HOME`、`CANTER_HOME`。真 canter 的测试默认 `#[ignore]`，PATH 上没有 canter 就跳过。
- 不碰 `corral ls` 里已有的 agent（都是用户的），不 `stop`、`send`、`keys`、`upgrade` 它们，也不 attach。手动实测要开 agent 时，名字 `ranch/test-r56b-<几个字>`，带 `--label role=test`、`--env CAIRN_DISABLE=1` 和隔离的 `--env HOME=…`、`--env CANTER_HOME=…`，用完 `corral stop`；最多同时开一个。
- 不打包、不部署：不切 `~/.local/bin` 的链接，不 `corral upgrade` 真实的 agent，不装技能，不换共用 helper。
- 不按项目名或路径批量杀进程（`pkill -f canter`、`pkill -f corral` 这类）：主控和别的 agent 的进程命令行里都带着项目名和工作目录，用户自己也开着 canter。停自己起的进程用起的时候记下的 PID。
- 碰到要拿主意的事（包括要偏离方案、要加依赖、canter 实际行为和方案写的不一样），停下来，在回复里写清问题、可选办法和你的建议，然后结束这一轮，等主控回复再接着做；不在自己窗口里问用户，不开着命令等，不要自己换别的办法。主控的决定写进完成记录。
- 不合并到 main，不推送。只在 `r56b` 分支上提交。

## 做完

在本文件末尾追加「## 完成记录」（在你的分支里提交）：做了什么、验证了什么、拿主意的地方、没做的事、踩过的坑，各几句话。踩过的坑指试了不行、换了办法的，工具或环境的行为和预想不一样的；没有就写「没有」，不要凑。回复里只写这几样，加上有没有要主控决定的事。命令都在前台跑完，全部做完后，回复最后一行写 DONE。

## 完成记录

2026-11-03，ranch/dev-r56b，分支 `r56b`。

- **做了什么**：实现 R56 第 1～8 条。canter 适配器负责会话文件跟随、公开 HTTP/SSE 订阅、事件翻译、重连、停止后的会话关闭确认；pen 只增加通用 watcher 起停和不透明游标交接。状态、回复、打断、题目、命令和送话确认沿用现有 v1 事件路径。升级前必须收回线程，旧目标没有 watcher 能力时报 `incompatible`；暂停、继续和显式／继承的 fence 报 `unsupported_kind`。登记 canter 的 `integration status`，没有新增依赖。对 paddock 都是兼容新增：canter 一家及其 `unsupported`、`interaction`、题目／命令和 stop 的 `remaining_sessions`，由 paddock 自己的任务接；别家输出回归测试通过。
- **验证了什么**：先以未注册 canter、尚未翻译事件、未拒绝不支持操作／旧升级目标的失败检查确认 RED，再实现并跑到 GREEN。新增 5 项翻译／游标纯测试、8 项隔离 pen 集成测试，以及线程停不下时取消升级的测试，扩充旧快照兼容检查；覆盖会话文件晚出现、换会话／恢复旧会话、服务端换 socket、升级不丢不重、旧 pen 报 unknown、剩余会话和 fence 继承。默认忽略的 `real_canter_lifecycle` 已手动通过（本机 canter `0.1.0 · 16fb98c`）：隔离 HOME／CORRAL_HOME／CANTER_HOME、本地假模型，验证带首句启动、多行送话确认、回复、提问与按键、shell 批准框、打断、升级 pen、工作中停止并确认服务端会话 closed；不调用真实模型。最后 `cargo test --all-targets`、`cargo clippy --all-targets -- -D warnings`、`cargo fmt --check` 全通过，统一使用 `.target/r56b`，所有命令均等待完成。
- **拿主意的地方**：方案和 CONTRACT 对用户传启动参数的处理原先冲突，已按主控 11-03 决定执行：用户自己传 `--session-file` 报 `usage`；自己传 `--close-on-exit` 接受，corral 不再重复添加。已修正 `docs/CONTRACT.md`「canter（R56）」对应句子，并用启动参数测试覆盖。没有其他待主控决定的事。
- **没做的事**：未改 R56c 范围、canter 或 paddock 仓库；未碰用户真实会话／状态，未替用户记目录信任，未调用 DELETE，未加依赖，未打包、部署、合并或推送。真实测试只为临时目录通过公开 folderTrust 接口建立信任，测试结束收回自己启动的终端和服务端。主控审查、合并部署及用户在场的最终联合验收留给后续。
- **踩过的坑**：corral 会重建登录环境的 PATH，假 canter 最初未被选中，测试改用隔离脚本的绝对路径。macOS Unix socket 在对端已关连接后再次设置读取超时会报 EINVAL，HTTP 客户端改为连接后一次设置超时，读取循环另检查总时限。真实 canter 提问框刚出现时立即按 Enter 未被处理，测试等待终端提示并留 600 毫秒再按键；产品的送话、按键路径没有为此改动。
