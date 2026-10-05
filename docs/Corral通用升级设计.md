# Corral 通用升级设计

2026-10-03。状态：已完成隔离实现与验证，主控及独立审查通过，合并 `dab13d7`；验证范围与返工记录见 `docs/任务/Corral通用升级-实施.md`。尚未部署，不授权首次迁移或操作现存用户 agent。

本文件是 `docs/DESIGN.md` 的通用升级专项设计，补充并在升级范围内替代《Corral核心Rust集成设计》§6、§8.1 的固定 helper/只切入口规则。原调研见《现存agent无缝接管可行性》，最新一致记录见 [实施前一致记录](调研/Corral通用升级-实施前一致记录.md)。历史调查中较强的“风险归零”“只有某条路线”“仅剩实测”等表述不构成保证。

## 1. 目标、授权与边界

用户目标是更新部署后全部现存 agent 自动由最新核心托管，同时保留原进程、上下文、在途工作、输入输出、连接、身份和完成提醒；新界面连接旧 pen、结束后 resume 不算满足。用户明确要求“这个方案不能绑定某一些agent”。

已批准的实施对象是首个具备升级协议版本之后的通用升级能力。今天现存 pen 没有升级入口，固定的 hook/扩展和内存中的旧 after 也不能凭安装新版迁移。这项首次限制未被解决；批准实施不等于接受重启、旧代码永久驻留或提醒丢失。部署和首次过渡另外确认。本轮不执行真实安装、链接切换、服务重启或现存 agent 升级。

主要实现归仓库内 `crates/corral-core`，原独立 Corral 仓库不改。Saddle、插件、部署消费者通过公开 Corral 命令接入，不读取其状态文件、不接收内部 fd；不开 Saddle 也具备升级能力。无中央常驻服务、无反向依赖、无 Python 新代码或旧实现兜底。

成功升级保留 agent PID/进程组/控制终端、上下文和在途工作，保留 name、instance、started、连接、字节顺序及终端模式；正常 exec 保留 pen PID 和退出码。升级不按 agent kind 分支，不调用 kind 专属退出/重启命令，不跳过持续忙碌会话。

所有故障都零损失不是本设计的保证。备用接管会改变 pen PID 并失去子进程退出码；进入新映像 I/O 之后的崩溃不做有损接管，仍可能丢失会话。实现和报告必须保留这些边界。

## 2. 公共入口与结果

- `corral upgrade [--all|NAME] [--exe PATH]`：通过公开能力和状态识别、逐个请求升级；缺省目标为当前 CLI 的不可变真实可执行路径。旧 pen 返回 `needs_restart`，不自动停、重建或 resume。
- pen `upgrade {exe}`：仅正常 Owner、无升级标记、无未退出备用进程时进入；重复同目标仅在已完成时 `already_current`，不能把 pending 算成功。
- pen `recover {exe}`：仅 Hold 内，在同一 epoch 重试现有快照；不另开一轮、不再次 fork、不覆盖在途流状态。CLI 提供对应公开恢复入口，不能要求用户直接访问私有 socket 或文件。
- status 公开实际映像 `exe`、`custody` 和 `upgrade {state, epoch, attempt, target, last_error}`，以及是否支持升级/恢复的能力。错误和 JSON 扩展保留既有 proto 1 消费者兼容。
- `accepted` 只表请求接受；`running_new_pending` 表已运行目标但收尾未完；`complete` 才是本次目标映像已服务且升级状态 none；`failed` 带本次 epoch/attempt 和原因；失去回执又无法查明为 `unknown`。`hold`、`hold_unprotected` 是恢复中的状态，不能算 complete。
- 无回执不自动重发业务。按 epoch、attempt 和 target 查询对应结果；仅 exe 等于目标不构成成功。再次 recover 只递增 attempt，不换 epoch。

批量命令须把部分成功、旧版无能力、失败、pending、hold、unknown 分开输出。不能把入口切换或新 CLI 已启动当作所有 pen 已更新。Corral 负责自己的 helper/提醒交接，宿主只消费公开结果。

## 3. 状态和资源归属

P 是旧 pen，N 是同 PID 的 exec 新映像，K 是临时备用进程。快照只搬运 Corral 自有状态，不读写 agent 内部上下文。

快照包含：name/instance/started、必要元数据与标签关联、旧 exe/目标 exe、epoch/attempt、agent PID/退出状态/退出时间、fd 角色表、master/listener/名称锁/客户端、所有客户端模式/半包/待发输出/行列/顺序、writer、排队输入剩余字节/延时/回执关联、下一写入时刻、输出环、最后输出时刻、Terminal 完整解析状态、人类输入保护与 recent、PTY 状态、resize/stop 定时与剩余停止步骤。状态版本必须可预检，格式不兼容时拒绝升级，不隐式丢字段。

冻结只在单线程 poll 的一致点发生。已读入内存的部分进快照，未读部分仍在内核；已写入内核的字节已经从待写队列排除。fd 保留本身不是完整状态正确的证据。时间语义保持原有绝对时刻，不能升级时重置超时或人类保护窗口。

`Custody` 区分 Owner、Handoff、Observer：Owner 保留正常生命周期清理；Handoff 不主动杀 agent、不删 socket 路径；Observer 不依赖 agent 是子进程，异常清理不主动杀 agent，显式 stop 仍按公共契约处理。错误返回、panic 展开、abort/被杀、部分构造失败必须分别处理；catch_unwind 不代替所有权规则。fd 本身的自动关闭也属于清理，不能只检查自定义 Drop。

正常 N 使用 PID/waitpid 恢复对子进程的观察；K 接管后是 Observer，采用非子进程退出观察与 PTY 状态，真实退出码不可得时为 null，不能伪造。锁与 socket 路径连续，已有 `ls` 清理不能误删升级中的存活实例。

## 4. 正常切换和恢复

| 阶段 | 所有者及动作 | 失败落点 |
|---|---|---|
| S0/S1 | P 正常服务；目标绝对路径与可执行性核验；子进程 `__pen-probe` 检查快照 schema | 拒绝并继续旧服务 |
| S2 | 在一致点冻结 I/O，custody Handoff，私有权限原子写 `upgrade.json` | 可处理失败恢复旧服务；写快照至备用建立之间的不可恢复崩溃无保护 |
| S3 | fork K，建立 P→K 控制管道及 K→P 存活管道 | fork 失败恢复旧服务，清理本次准备 |
| S4 | 保存并调整应继承 fd 的标志，exec 目标 `__pen-resume` | exec 返回错误则恢复标志、取消 K、回旧服务 |
| S5 | N 零流 I/O 验证快照及 fd/锁 inode，用无 kill 语义的恢复结构持有资源 | 尝试旧映像恢复入口；再失败进入 Hold |
| S6 | `json`→`committed`，构造 Handoff Pen，处理元数据更新 | 未开始流 I/O，快照仍可用于恢复 |
| S7 | `committed`→`active` 后开始流 I/O，发送完成相关回执，通知 K 退出 | 此阶段崩溃没有无损恢复保证，不能改称已经完成升级 |
| S8 | 确認 K 退出、清理本次标记、custody Owner | 正常服务中可重试收尾；未完为 pending |

`active` 是不可逆 I/O 边界，`complete` 是可观察的升级完成结果；两者不混同。每个 rename 只对自身原子，回执、I/O、清理和所有权变更不是一个事务。丢失客户端回执不回滚或重新发送输入，查询真实状态。元数据更新失败要如实报告，不能让 stale meta 与成功报告产生身份错误。

K 只覆盖新映像开始流 I/O 前的资源保留，不做 active 之后的有损接管。双管道各端按角色关闭；EOF 必须真正来自唯一持有者退出，不能被 fork/probe 等意外继承遮蔽。P 和 K 后续 exec 各自需要的 fd 标志都必须正确，不能以 P 清除 CLOEXEC 推定 K 的描述符标志也已改变。

K 收取消或退出通知即退出；P/N 失去联系且精确快照存在时，按记录尝试有效恢复映像；active 后或状态不能确认时不从旧快照重放输入。恢复失败可以导致会话丢失，不能声称“备用进程保住 master 所以必然存活”。备用本身失败须反映保护状态；K 未退出前不允许下一轮普通 upgrade 覆盖标记。

Hold 仅由验证失败且回退 exec 也失败进入，保持已有 fd 与精确快照，不读写原 master/客户端。新控制连接仅回答 status/recover，回答后关闭，不混进旧快照。recover 在同一 epoch 增 attempt，原子记录 target，回 accepted 后再尝试 exec；此连接关闭后最终失败通过 status 对应 attempt 观察，不能声称还能补回执。正常恢复成功也必须达到 complete，不能仅看 exe。

Hold 中使用同一个 K；无 K 时报告 hold_unprotected。若 N 也崩溃且所有恢复映像均不可执行，没有存活保证。不得自动停止、注入或重建 agent 解围。

## 5. 通用适配层

| 类别 | 新版本设计 | 边界 |
|---|---|---|
| 无 hook 程序 | 只依赖 PTY/进程/退出，不按 kind 分流升级 | unknown 状态、send confirmed:false 保持既有语义 |
| 外部 hook | 启动时注入 Corral 拥有的稳定绝对入口，指向新 helper；切换通过原子替换入口链接，不覆盖版本包 | 不依赖 agent 热加载；旧硬编码真实路径的会话不会自动得到此能力 |
| 进程内扩展 | pi/omp 扩展只采集原生事件；输入原文关联、回复判定、去抖和重试宽限移到可升级读取端 | v1 已判定记录与 v2 原始事件兼容；采集接口本身是稳定协议，不能声称已加载代码被热替换 |

稳定入口与采集器分离是本轮必要项，和首个可升级版本一起交付，避免先创建另一批冻结旧判定逻辑的会话。不改变 agent 自身 API，不改用户全局配置，不把“支持四种客户端”作为机制通用性的证明。

每个独立 CORRAL_HOME 的 helper 定位和版本切换都由 Corral 公开路径负责；测试在临时命名空间和不可变临时版本包验证。随包资源、公开帮助和安装规则反映新入口；`scripts/package.sh` 继续只生成包，不自行安装或升级真实进程。部署消费者未来执行目标入口切换与 `corral upgrade --all`，本轮只交付该能力与使用说明，不实际执行。

## 6. 持久提醒与交接

新提醒在接收者命名空间持久保存记录：文本、request_id、force、绝对 deadline、被等待方 name/instance、接收方 instance、阶段和结果证据。既有无记录的 after 不擅自发现其内存、杀掉或重建；其首次过渡限制明确报告。

阶段至少区分 waiting、sending、sent、确认结果、not_delivered、unknown、expired。先记录阶段再执行外部动作，不把 pending 当交付。保持既有等待/接收者空闲/人类保护和超时语义；接续不能重置 deadline 或静默改变等待方实例更换的处理。

每条提醒仅锁持有者执行并写记录。锁文件不覆盖、不删除，获取 flock 后复核 inode；记录用临时文件加 rename。handover 仅是交接请求，新 worker 获取锁后重读记录。旧持有者最后一次写入后才释放锁并退出；不用 PID 信号驱赶持有者。等待、确认、空闲重试的循环在迭代边界检查交接，不让黑盒长等待阻塞整个升级。已发出的单次请求先结束并记下可确认事实，再交接。

send 增加 request_id，pen 的 recent 条目区分已接受和 written。该有界记录仅表示特定请求是否已排队/完整写入，不是持久交付账本。事件摘要/时间匹配继续沿用现有 CLI confirmed 语义，但不能证明同文本两条提醒各自精确完成。

sending 恢复时按 request_id 查询证据；已完整写入只做后续确认，排队中继续观察，无法证实则 unknown，不重发、不更换 ID 重试。“recent 中没有”不能推出未接受：请求可能仍在连接/backlog 或已经被挤出。没有接收侧幂等协议，不引入推测性重试。普通程序无事件确认时明确 confirmed:false。

正常 worker 升级在阶段边界串行交接；异常崩溃在宣告发送到记录回执之间可能留下 unknown，不能宣称恰好一次交付已被证明。worker 交接失败和旧 worker 不可迁移都包含在公开批量结果中，不让全部 complete 掩盖部分提醒仍待处理。

## 7. 验证与实施顺序

同一集成分支依序完成 pen/公开入口、适配层、提醒及必要消费者/打包资源。前后半有集成关系，不各自当成已完成产品合并或发布。主控按项目规则审查并做独立交叉审查。

按 AGENTS.md 先建立可运行且因目标行为缺失而失败的检查，再最小实现。隔离验证使用合成程序、假 hook/事件、临时 HOME/CORRAL_HOME 和两个可区分的不可变版本目录；不依赖真实 agent、用户数据或真实安装。覆盖正常繁忙流、半包/排队输入与回执、连接/终端状态、锁/退出/停止、升级故障、Hold 往返、提醒交接及适配语义。具体用例由实现者在验证预算内选择。

不得把编译失败当 RED、把模拟/单测当真实产品升级、把限定复跑绿写成首轮全套绿。共享 CARGO_TARGET_DIR；项目标准 `cargo test --all-targets`、`cargo clippy --all-targets -- -D warnings`。版本生命周期检查从固定临时包执行，避免共享产物覆盖持久测试 pen。

真实 Claude/Codex/pi/omp 的兼容性冒烟是后续验证项，若本轮仅有合成证据就保留未验证标记；不得因此操作用户会话。任何实现发现的实质设计矛盾先向主控报告，不自行降低保留原进程/在途工作/不绑定 agent 的目标。

## 8. Herdr 参照与首次限制

Herdr 的兼容旧服务在默认更新及 Homebrew 更新后继续运行，停止/重启服务才结束 pane；其可选活交接会中断客户端、在途请求等，不满足本目标的全部连续性。暂停失败回滚，并非超时后继续。参考 [官方安装说明](https://herdr.dev/docs/install/#update)、[活交接边界](https://herdr.dev/docs/session-state/#live-handoff)、[v0.9.3 交接源码](https://github.com/herdrdev/herdr/blob/v0.9.3/src/server/headless/lifecycle.rs)。这些是资料依据，不是本项目实测。

首次旧 pen/旧适配器/旧 after 无公开迁移机制。只读原包、保留用户 agent 的约束下不能宣布已完成首次无缝过渡，也不把系统层面穷尽为“不可能”。本轮实施通过后仍等待用户明确部署和首次安排，不自动重启、resume 或注入。
