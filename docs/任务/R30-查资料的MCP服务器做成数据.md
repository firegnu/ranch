# 任务：套墙的 agent 接哪个 MCP 服务器做成数据，默认 Exa，可换成别家（含要 API key 的）

2026-10-20，ranch/main 交给 ranch/dev-r30（Codex，gpt-6-astra / high）。
路由：重 / 交叉审查要 / 影响面：碰要害（路由：重（置信 0.97）、要、碰要害）
类型：功能变更
依据：R29 让四家套墙时接 Exa 的 MCP 查资料，但 Exa 写死在 fence 程序里（`crates/fence/src/runs.rs` 每次运行写出 `fence-exa` 资源，内容 `include_str!` 编进程序）。本轮把「接哪些 MCP 服务器、各家怎么接」都改成数据，fence 程序不认得具体哪一家；默认仍是 Exa，行为不变；用户能换成别的服务商，包括要 API key 的。
提示：围绕已确认的使用目标完成变更，优先沿用现有机制。
你是被委派的 agent：照本文件做，不要再开别的 agent。

## 用户原话

- 10-20：「等一下，现在Exa是不是内嵌再代码里面了」「这个Exa是不是一个MCP服务器？」
- 10-20：主控提议把「给套墙的 agent 接哪些 MCP 服务器」做成通用数据（数据里写服务器名字和地址，种类数据里写各家怎么接一个 MCP 服务器，fence 只照着生成）。用户：「对，我就是这个意思，我可以换搜索服务提供商。」
- 10-20：要 API key 的服务商，用户选「这次一起支持」：key 只写在墙外，fence 启动时交给各家的 MCP 客户端；key 会以请求头或环境变量进墙，agent 有办法看到，只发给对应服务商。
- 10-20：「现在就是默认Exa（现在的状态）。」

## 先读

- `AGENTS.md`「规矩」，特别是「改动要写明对 paddock 的影响」「实现中要改设计，先改 DESIGN.md」。
- `docs/DESIGN.md` §8 sandbox（含 R29 那几段）。
- `crates/fence/README.md`（数据合并顺序、哪些键能写在哪一层、`[services]` 怎么整份替换、占位符）。
- `crates/fence/data/`、`crates/fence/resources/exa/`、`crates/fence/src/runs.rs` 写资源那段、`crates/fence/src/rules.rs` 的 `${run_nonce}`。
- `docs/任务/R29-套墙的agent能查资料.md` 的完成记录（四家各自怎么接 MCP、踩过的坑：pi 的空 `Authorization` 头、omp 要在 URL 里写明工具、Codex 同名表递归合并认证所以用随机名）和 `docs/任务/R29-审查.md`。
- `docs/CONTRACT.md`「sandbox 与 `fence`」一节。

## 要做的

1. MCP 服务器作为一类数据（和模型服务 `services/` 同类的做法）：写名字、地址，需要 key 时写 key 怎么带（请求头名、前缀等）。内置一份 Exa，内容和现在一致。
2. 档案决定默认接哪些 MCP 服务器，`dev` 默认 Exa。用户换服务商只需：在 `~/.fence/` 加一份那家的服务器数据，再在本机补充（`local.toml`）里改一处选用；不用整份替换档案或各家种类数据。
3. 声明了 MCP 服务器，它的域名自动放行，不用另在 `domains` 里再写一遍（白名单里现在单独写的 `mcp.exa.ai` 随之改为来自 Exa 的服务器数据）。
4. 四家「怎么接一个 MCP 服务器」写在各自种类数据里；fence 程序照数据生成启动参数和要写出的文件，程序里不出现 Exa 或任何服务商的名字、地址。R29 的几个要点照旧：Codex 的 server 名每次运行不同（防同名表递归合并认证）；pi 免 key 时的空 `Authorization` 头；omp 的地址里写明要哪些工具。
5. API key：只写在墙外、墙里的 agent 不能直接读的文件里（具体放哪由你定，写进 README）；fence 启动时交给对应 MCP 服务器的配置，只发给这一家。没配 key 的服务器照免 key 处理。
6. 先改 DESIGN.md §8 写清数据怎么组织、怎么换服务商、key 怎么带及它的局限（进墙后 agent 能看到），再实现。README 写一段「怎么换成别的搜索服务商」的步骤。写清对 paddock 的影响。

## 怎么算做完

- 用户原话：「我可以换搜索服务提供商。」：照 README 的步骤，只在 `~/.fence/` 加数据、改一处选用，就能把四家接的 MCP 服务器换成别家；fence 程序里没有 Exa 的名字和地址。
- 用户原话：「现在就是默认Exa（现在的状态）。」：不改配置时，四家套 dev 墙接的仍是 Exa，搜索和读网页照常能用。
- 用户选项「这次一起支持」：配了 key 的服务器，四家启动时 key 都被带上，且只发给那一家；没配 key 的照免 key 处理。
- 验证只做这些：
  - 先写测试再实现：用记录 argv 的合成 runner（沿用 R29 的回归）核对四家在三种情况下实际收到的参数、环境变量和写出的文件——默认 Exa、换成一个假的服务商（假地址）、假服务商配了假 key。改数据和程序前先跑一次看它失败。
  - 项目标准检查各跑一次：`cargo test --all-targets`、`cargo clippy --all-targets -- -D warnings`、`cargo fmt --check`，加 `git diff --check`。
  - 真实回归：用编译目录里的程序，四家依次（不要同时，Exa 免费额度会限流）套 dev 墙用非交互方式各做一次 Exa 搜索、一次读网页。
  - 觉得不够，在回复里说，不要自己加。

## 不要做

- 不注册、不使用任何真实 API key，不连 Exa 以外的真实服务商；key 相关只用假值和合成 runner 测。
- 不改各家已有的读写路径、钥匙串读取和白名单里别的域名。
- 不做 fence 自己的上网网关。
- 不改 corral 的程序和 corral、corral-dispatch 技能。
- 不动 `~/.fence`、各家 agent 的全局配置和认证文件。
- 不改 paddock。
- 不碰用户现有的 agent（`corral ls` 里不是你开的）：不 `stop`、`send`、`keys`、`pause`、`upgrade`，不 attach 打字。
- 不部署：不切 `~/.local/bin` 的链接、不装技能、不往 `~/.local/share/ranch/versions/` 打包、不 `corral upgrade`。
- 不要按项目名或路径批量杀进程（`pkill -f corral`、`pkill -f ranch` 这类）：主控和别的 agent 的进程命令行里都带着项目名和工作目录，一条命令能把它们全杀掉。停自己起的进程用记下的 PID 或 `corral stop`。
- 某一家没法照数据接上，或 key 没法只发给对应服务商，停下来报告，说清原因和可选办法，不要自己换别的办法。
- 不合并到 main，不推送。只在 `r30-mcp-data` 上提交。

## 做完

在本文件末尾追加「## 完成记录」（在你的分支里提交）：做了什么、验证了什么、拿主意的地方、没做的事、踩过的坑，各几句话。踩过的坑指试了不行、换了办法的，工具或环境的行为和预想不一样的；没有就写「没有」，不要凑。回复里只写这几样，加上有没有要主控决定的事。命令都在前台跑完，全部做完后，回复最后一行写 DONE。

## 完成记录

2026-10-20，ranch/dev-r30：已完成、自查通过，待主控及交叉审查；只在 `r30-mcp-data` 提交，未合并、推送或部署。

- 做了什么：先更新 DESIGN §8，再增加 `mcp/` 数据组、档案默认选择和本机顶层 `mcp` 数组整份替换。四家接法全部写进种类 `[mcp]` 的参数／文件模板；删除固定 Exa 资源及 `runs.rs` 的写死逻辑，只为当前种类和所选服务器生成资源，域名从 URL 自动放行。key 从墙外 `$FENCE_HOME/keys/<名字>` 读取，源目录及所选文件的原路径、符号链接真实目标强制禁读；请求头只装进对应 server 配置，生成文件为 0600。README 有换服务商步骤、认证边界和模板说明，CONTRACT 同步兼容新增的 MCP 列表／来源；paddock 不用改。
- 验证了什么：先扩充 R29 的合成 runner 回归，再运行替换服务商、带假 key 两条测试；旧实现因本机 `mcp` 选择不受支持而失败，是有效 RED。实现后默认 Exa、假服务商、假服务商带假 key 三种场景均通过，覆盖四家实际 argv、用户参数、环境、临时文件、域名和 key 源禁读规则；带 key 场景同时选择免 key Exa，确认假 key 不进入 Exa 配置或全局环境／fence 运行记录；保留 Codex 两次运行名称不同的检查。没有使用真实 MCP key。项目标准检查各一次通过：`cargo test --all-targets` **183 过、0 失败、11 按原设置忽略**；`cargo clippy --all-targets -- -D warnings`、`cargo fmt --check`；提交前 `git diff --check` 通过。
- 真实回归：用独立编译目录里的 `debug/fence`、现装 srt 0.0.80 和隔离的 FENCE_HOME／缓存／工作目录，复制已有本机放行规则供 IP Guardian 正常运行，没有复制 key。四家依次前台运行，**每家一次搜索 `Rust official blog`、一次读取 `https://blog.rust-lang.org/`，全部成功，退出码 0，无重试、无限流**。实际工具返回均含 Rust 官方博客标题；Claude 经 ToolSearch，Codex 经 `functions.exec` 的工具目录，pi 直接调用，omp 经原生 `xd://` 设备读取 schema、写入调用。版本为 Claude Code 2.1.304、Codex CLI 0.163.0、pi 1.3.0、omp 18.12.3。日志只留本机临时目录，未提交；实测使用的编译目录运行时文件和 srt 链接已清掉，避免影响之后的“缺少运行时”夹具。
- 拿主意的地方：只支持与现有四家接法一致的 Streamable HTTP MCP，用请求头传 key，不增加网关或 stdio 桥接。Exa URL 统一在数据里带两个工具名，保留 omp 的要求，四家真实验证均成功；认证头字段依 [Exa 官方说明](https://exa.ai/docs/get-started/exa-mcp) 写成 `x-api-key`，默认没有 key。复用锁文件中已有的 `http` 1.5.0 解析地址／校验请求头，没有升级依赖。Codex 沿用随机名称；pi 保留空 Authorization 并叠加该 server 的认证。`check` 不读取 key，认证预览为空；`run` 才读取，拒绝空值和客户端插值／命令求值语法。生成的参数或文件进墙后 agent 仍能看到 key，文档明确这一局限。旧自定义种类仍整份覆盖，需自行补模板。
- 没做的事：未注册／使用真实 MCP API key，未连其他真实搜索服务商；未改原有读写放行、钥匙串读取或其他白名单域名。未改用户 `~/.fence`、agent 全局配置／认证文件、现有 agent、corral 程序或技能、paddock；没有新增任务范围外的验证。未部署、合并到 main 或推送。
- 踩过的坑：没有新的客户端接入坑。沿用 R29 已确认的 `.ts` 后缀、pi 空认证头、omp URL 工具列表及 Codex 工具目录发现方式，四家本轮均一次通过。
- 要主控决定的事：没有新增决策项。

### 第二轮（2026-10-20，主控认可审查第 1–4 条后返工）

状态：**四条均已修复，自查通过，待复审**；只在 `r30-mcp-data` 提交，不合并、推送或部署。上一轮真实服务结果仅是当时版本的记录，本轮未重跑真实服务。

- 做了什么：先修改 DESIGN §8。Claude 改为写 0600 的 MCP 配置文件，参数只带路径；Codex 的种类模板声明 `headers_via_env = true`，`env_http_headers` 只放该服务器专用的随机环境变量名，真实值经独立的 `secret_env` 字段传到 `Command::envs`，不进入可序列化规则环境、argv 或运行记录，保留随机 server 名。组合后的 header 统一拒绝 `$`。源 key 保护改为枚举 `keys/` 全部条目并解析真实路径，不受 MCP 选择影响；仍只读取所选 key，`check` 不读内容。准备失败清理本次新建的实例记录及临时目录，保护启动前已有路径；若清理失败附 `cleanup_errors`。README 和 CONTRACT 已同步，paddock 不用改。
- 验证了什么：先改合成回归再运行，旧实现分别因 argv 含假 key、接受 `$REVIEW_OTHER_TOKEN`、改选后外部链接目标不在禁读集合、准备失败残留目录而失败，四条都有有效 RED。修复后定向回归通过。argv／文件／环境回归检查四家、其他免 key server、Codex 两次随机名及环境变量名、0600 权限、`check`／`denials`／记录不含假 key；插值回归覆盖 `$VAR`、`${VAR}`、`$$` 和 prefix 中的 `$`，只使用假变量；路径回归覆盖单文件链接、keys 目录链接、HOME 外工作目录中的目标，以及选中／改选／空选择。假 key 故意含无效 UTF-8，`check --kind pi` 仍成功，未选中时 `run` 也成功，确认没有读取其内容。清理回归覆盖写出凭证后 `mkdir` 失败，以及启动前已有记录目录、临时目录、普通文件、符号链接均不被删除。
- 指定检查：前台运行 **一次** `cargo test -p fence`，**26 过、0 失败、8 个真实 srt 测试按原设置忽略**；`cargo clippy --all-targets -- -D warnings`、`cargo fmt --check` 各一次通过，提交前 `git diff --check` 通过。所有 Cargo 命令使用任务独立编译目录。自查追踪了 `secret_env` 到子进程环境的唯一出口、runner 的 shell 参数装配、key 路径解析和准备阶段两种失败分支，未发现这四处修复仍有同类缺口。
- 拿主意的地方：专用环境变量以本次随机值和服务器序号隔开，由种类模板选择环境引用接法，Rust 不按客户端或服务商名称分派。目录清理只在确认本次创建成功后才做；启动前就存在的路径保持不动。正常运行仍按完整 `run.json`／`exit.json` 在以后启动时回收，README 不再声称任何异常残留都能七天清理。子进程内可读取专用环境或临时文件仍是已接受的边界。
- 没做的事：按返工范围没有重跑全项目测试、真实客户端、真实服务或真实墙内读取；没有使用真实 key、扫描用户进程、改变用户配置或现有 agent。这里证明的是合成 runner 收到的参数／环境／文件、解析输出及准备失败行为，不将其描述为真实客户端认证或真实 sandbox 隔离实测。未改 corral、技能或 paddock，未部署、合并或推送。
- 踩过的坑：没有；四个缺陷均由直接相关的回归复现并消除。
- 要主控决定的事：没有新增决策项。
