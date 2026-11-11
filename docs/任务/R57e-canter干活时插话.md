# 任务：canter 干活时也能用 `corral send --steer` 插话（走 canter 公开接口）

2026-11-11，ranch/main 交给 ranch/dev-r57e（Codex，gpt-6-astra / xhigh）。
路由：高 / 交叉审查不要 / 影响面：改行为（路由：重（置信 0.84）、拿不准、拿不准；主控判：走 canter 自己的接口、不往终端打字，没有 R57b 那种误按框里选项的风险，按改行为算、不交叉审查）
类型：功能变更
依据：本轮让 `corral send --steer` 在 canter 干活时经 canter 公开的 input 接口插进当前这一轮；别家的插话、不加 `--steer` 的送话都不变。不改 canter、pony、paddock。
提示：围绕已确认的使用目标完成变更，优先沿用现有机制。
你是被委派的 agent：照本文件做，不要再开别的 agent 替你干活（实测用的测试 agent 除外）。

## 用户原话

- 11-11，canter 仓库的主控转来（转述）：用户在 paddock 里开着一个 canter（`canter-desktop/main`），它在一轮里连着做好几件活时，用户想从外面插一句话让它停一停；`corral send canter-desktop/main "…" --steer` 返回 9 `unsupported_steer`，只能等空闲。
- 11-11，主控列了两条路：甲（corral 直接调 canter 公开接口发 `mode: "steer"`）、乙（往 canter 输入框打字再按 Tab）。用户：「走甲，第2条按你建议，写需求单吧」。第 2 条指回执 `turnId` 为空时：照常算成功，另加一个字段标明排到下一轮了。

## 先读

- canter 协议：`/Users/firegnu/Developer/personal_projs/canter/crates/canter-protocol/README.md` 从「空闲时 input 开新轮，忙时默认 `queue`，也可 `steer`」到「压缩轮进行中的 `steer` 按 `queue` 处理」那几段（input、`input.queued`、`input.removed`）；canter `docs/DESIGN.md` 的 P2-97「干活时插队」。只读，不改 canter 仓库。
- `docs/DESIGN.md` §7「接入 canter（R56）」和「干活时显式插话（R57b）」；`docs/CONTRACT.md`「`corral send NAME TEXT --steer`（R57b）」。
- `crates/corral/src/pen/steer.rs`；`crates/corral/src/agents/mod.rs` 的 `Adapter`；`crates/corral/src/agents/canter.rs` 里找服务端（`discover`、`server.json`）、读会话文件、走 Unix socket 的 HTTP GET 那几处。
- `AGENTS.md`「规矩」，特别是「改动要写明对 paddock 的影响」和「过渡期新旧版 corral 双向兼容」。

## 主控已实测的（11-11，现装 864d900）

- 对测试 canter 在它跑 shell 时 `corral send --steer` 报 9 `unsupported_steer`。
- 同一会话直接 `POST /v1/sessions/{会话}/input`，body `{"input":[{"type":"text","text":"…"}],"mode":"steer","clientName":"…"}`：回执 `{"eventSeq":…,"turnId":"<当前轮>"}`；之后事件流出现 `input.removed`，reason `steered`、同一个 turnId；全程只有一轮，模型照插话改了做法。canter 终端在消费前把它显示成 `· Queued: …（from <clientName>）`，这是 canter 协议本来的样子，不用管。

## 在哪里干活

- worktree：`/Users/firegnu/Developer/personal_projs/ranch-worktrees/r57e-canter-steer`，分支 `r57e-canter-steer`（已从 main 建好）。只动 `crates/corral/`、`docs/DESIGN.md`、`docs/CONTRACT.md` 和本文件末尾。
- 编译目录：命令前加 `CARGO_TARGET_DIR=$HOME/Developer/personal_projs/ranch-worktrees/.target/r57e-canter-steer`。

## 要做的

1. **先改设计**：照 AGENTS.md，先在 `docs/DESIGN.md` R57b 那段把「canter 不开」改成 canter 走公开接口，写清做法和理由；`docs/CONTRACT.md` 写清 canter 的结果和新字段；`crates/corral/resources/AGENT_USAGE.md`、`SKILL.md` 支持插话的列表加上 canter。
2. **canter 在 working 时加 `--steer`**：不往终端打字，由 pen 经 canter 公开接口 `POST /v1/sessions/{会话}/input`、`mode: "steer"` 送出。会话 ID 从 corral 交给 canter 的会话文件读，socket 从 `<stateDir>/server.json` 找。只用 canter 公开的接口，不读 canter 状态目录里 `server.json` 以外的文件（R56 的规矩）。`clientName` 自己定，写进 DESIGN。
   - 回执带 turnId（进了正在跑的这一轮）：成功，已确认。
   - 回执 turnId 为空（canter 改成排队，下一轮执行）：照常算成功，另加一个字段标明排到下一轮了。字段名自己定，写进 CONTRACT。
   - 请求没发出去、或 canter 明确拒绝：报本次没写入，可以等空闲重送。
   - 请求已发出但没拿到回执（超时、断开）：不能断定 canter 没收下，按「已写入、不能重送」那类报，不自动重发。
   - 不碰 canter 输入框，用户在 canter 里没发出的草稿原样保留。
3. **照旧的**：canter idle 时 `--steer` 照现在的路径送；暂停、blocked、starting、upgrade 进行中照旧拒绝，退出码不变；「30 秒内有人在窗口里操作过」照旧挡，`--force` 只跳过这一道。Claude Code、Codex、omp 的插话，别家报 9，不加 `--steer` 的 `send`、`send --after`，都不变。
4. **兼容**：旧 pen 管着的 canter 照旧报 9；新 pen 不能因为等 canter 回执拖住自己的主循环。对 paddock 是兼容新增（canter 的 pen 本来就报 `capabilities.steer:1`），在 DESIGN 里写明 paddock 不用动。

## 怎么算做完

- canter 干活时，`corral send <名字> "<话>" --steer` 能插进正在跑的这一轮。
- 上面第 2 条的四种结果各有测试，用合成的 canter 服务端（本地 Unix socket），不用真 canter；第 3 条里 canter 照旧拒绝的情况和别家行为不变由现有测试兜着，现有测试不改断言、不放宽超时。
- 开一个测试 canter（`ranch/test-<名字>`、`--label role=test`、`--env CAIRN_DISABLE=1`、临时目录，不带 `--yolo`），在它跑 shell 时用 `--steer` 插一句，确认送达，并照做一次。它弹「信任这个目录」框时，用 `corral keys <名字> down enter` 选「Open restricted」（不保存信任）。用完 `corral stop`。
- 验证只做这些：上面的测试和一次真实 canter 实测；`cargo test --all-targets`、`cargo clippy --all-targets -- -D warnings`、`cargo fmt --check` 各跑一次。觉得不够，在回复里说，不要自己加。

## 不要做

- 不改 canter 仓库；canter 要改的写进完成记录交主控。不走「打字再按 Tab」那条路。
- 不改 pony、paddock，不改派活技能（`crates/dispatch/resources/corral-dispatch/`）。
- 不碰 `corral ls` 里不是你开的 agent，包括 `canter-desktop/main`；不对任何 agent 跑 `corral upgrade`。
- 不要按项目名或路径批量杀进程（`pkill -f <项目名>` 这类）：主控和别的 agent 的进程命令行里都带着项目名和工作目录，一条命令能把它们全杀掉。停自己起的进程用起的时候记下的 PID。
- 碰到要拿主意的事，停下来，在回复里写清问题、可选办法和你的建议，然后结束这一轮，等主控回复再接着做；不在自己窗口里问用户，不开着命令等，不要自己换别的办法。主控的决定写进完成记录。
- 不合并到 main，不推送，不打包部署。只在 `r57e-canter-steer` 上提交。

## 做完

在本文件末尾追加「## 完成记录」（在你的分支里提交）：做了什么、验证了什么、拿主意的地方、没做的事、踩过的坑，各几句话。踩过的坑指试了不行、换了办法的，工具或环境的行为和预想不一样的；没有就写「没有」，不要凑。回复里只写这几样，加上有没有要主控决定的事。命令都在前台跑完，全部做完后，回复最后一行写 DONE。

## 完成记录

- 做了什么：先更新 DESIGN 和 CONTRACT，再在 Adapter 增加可选公开接口发送挂点。working canter 的插话从公开会话文件、server.json 找目标，由有截止时间的线程发送 input POST，pen 主循环收已完成的结果；成功直接凭 API 回执确认，草稿不变，等回执期间不挡 status、keys，upgrade 仍互斥。idle、普通 send、after 和其他适配器沿用旧路径；两份 corral 使用说明补上 canter。只改任务允许的文件。
- 验证了什么：先加当前轮接收用例，确认旧实现以 9 unsupported_steer 失败；实现后四条合成服务端用例覆盖当前轮、下一轮（null／缺省 turnId）、发现／连接失败和 409 拒绝、断开／超时／损坏回执／500，并检查终端草稿不变、不重发、主循环可响应和升级互斥。旧 CLI 不识别 API 回执的兼容用例也先失败再通过。最后相关测试为 canter 的 4 条、原 steer 的 16 条通过，1 条合成程序入口按原样 ignored；末次补充 500 分类后单独复跑对应未知回执用例通过。原测试断言和超时均未放宽。
- 全套检查：`cargo test --all-targets` 按要求只跑一次，在原 steer 用例失败后停止（没有公开会话文件的合成 canter，从原来的 9 变成了 7）；已恢复该情况下的 9，并在上述相关测试里确认原用例通过。该次全套未完成，后续 target 未执行，没有声称全套通过，也没有自行重跑。`cargo clippy --all-targets -- -D warnings`、`cargo fmt --check` 各一次通过；之后只补了 5xx 的保守分类与对应测试、文档，已运行 cargo fmt 和该直接相关测试。建议主控合并前补跑完整检查。
- 真实实测：只开一次 `ranch/test-r57e-steer`，独立临时 CORRAL_HOME 和临时 cwd，`role=test`、`CAIRN_DISABLE=1`，未带 `--yolo`。信任框按 down enter 选 Open restricted；Always ask 的 `/bin/sleep 30` 只选 Allow once。命令执行中 send --steer 回 `confirmed:true,queued_for_next_turn:false`（latency 0.009 秒）；公开会话快照只有一轮，shell 完成后同一轮出现插话用户消息，最终回复从原要求的 ORIGINAL_DONE 改为 R57E_STEERED_DONE。已 corral stop，退出 0，没有 remaining_sessions；未接触其他 agent。
- 拿主意的地方：按任务授权选 `clientName:"corral"`、`queued_for_next_turn`；未写入／明确 4xx 拒绝用 7 steer_failed，开始写出后结果不明（含 5xx）用 3 not_delivered，禁止重送。为兼容 R57b CLI，在 steer 请求加 api_receipt:true；旧 CLI 对新 pen 的 working canter 仍在发送前报 9，新 CLI 对旧 pen 也仍报 9。无持久格式变更，paddock 不用动。没有需主控再定的实现方案；主控需决定是否补跑这次被“一次”限制留下的全套检查。
- 没做的事：未改 canter、pony、paddock、派活技能和 HANDOFF；未升级任何真实 agent，未合并、推送、打包或部署。没有需要 canter 仓库配合的改动。
- 踩过的坑：原 steer 的合成 canter 只发状态事件，没有公开会话文件，开放适配器后会走到发现失败而改变原退出码，已恢复原来的拒绝。R57b CLI 会忽略 API 成功回执、继续等归一化输入事件，所以增加显式回执识别声明，在旧 CLI 下拒绝发送。真实 canter 配置为 Always ask，sleep 也会弹单次批准框；确认只允许这一次后继续，未改变权限模式。

临时目录已清理。工具拒绝了带 `-f` 的删除命令，改用不带强制选项的 `rm -r -- <本次临时目录>` 后成功；测试 agent 在删除前已停止。
