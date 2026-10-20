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
