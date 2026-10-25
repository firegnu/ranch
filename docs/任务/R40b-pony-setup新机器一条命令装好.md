# 任务：R40b `pony setup`：新机器上一条命令带着走完安装

**草稿**：要做的、做法里的取舍（标「主控建议」）待用户看过再派。排在 R40a 之后（最后一步用 R40a 的「绑」）。派之前补路由、交给谁、几档和验证预算。

2026-10-21，ranch/main 起草。
类型：功能变更
依据：新机器从头装，从十来步（见 `crates/pony/README.md`「部署与退回」第 1–4 步）减到约 5 步。Slack 那边建应用、装到工作区、建 app token 是 Slack 的规矩，减不掉，只把它变成点链接照着做。不做 R40a 的「绑」。
提示：围绕已确认的使用目标完成变更，优先沿用现有机制。

## 用户原话

- 10-21：「现在加一个bot我感觉有点复杂」
- 10-21：「现在加入一个bot很麻烦，你能说清步骤吗？以你为例子。」
- 10-21：「现在我的问题就是像减少问题2的工作量。」
- 10-21 选：「两种都要」。
- 底线：「不要动ranch中corral的核心代码。」「Slack 的凭证只放在本机，不进任何仓库。」

## 用户那边的步骤，做完以后

1. 跑 `pony setup`，它打印一个链接。点开，Slack 建应用的页面已经填好清单；选工作区、建好、装到工作区；在 Basic Information 建 app token。（页面上怎么点，setup 一步步写出来。）
2. 回到终端，按提示粘两枚 token；setup 当场验证能不能连上，连不上问要不要填代理。
3. setup 显示一个配对码。在 Slack 建私有频道、`/invite @pony`、发 `@pony <配对码>`：pony 认下发这句的人当 owner。
4. setup 列出要装的后台服务，确认后装好。
5. 在那个频道发 `@pony 绑 <仓库>`（R40a）。

## 要做的

1. 新命令 `pony setup`，在用户自己的终端里交互着走，前面做过的步骤跳过，可以重跑。
2. 建 `~/.pony`、`secrets/`，权限照现有规矩（0700、0600）。
3. 打印建应用链接：把设计稿 3.3 的清单放进 Slack 的 `manifest_json` 链接参数（Slack 文档：<https://docs.slack.dev/app-manifests/configuring-apps-with-app-manifests>）。清单、链接、页面上的操作说明属于 Slack，放在 Slack 适配器里。
4. 读两枚 token：终端不回显，不经命令参数、环境变量；写进 `secrets/slack.toml`（0600）。写之前用它们连一次验证；连不上说原因，问要不要填 HTTP(S) 代理，填了写进配置的 `proxy` 再试。
5. 配对认 owner：显示一个配对码，用户在 pony 所在的频道发 `@pony <码>`，发这句的人（真人，不是机器人）写成这一家的 `owner`。
6. 写 `config.toml`：`owner`，`channels` 先空着（要允许空列表）。
7. 装后台服务：照 `pony service install` 先列出要做的，用户确认才装；装完显示 `pony status` 的结果。
8. 已经配好的机器上跑 setup：说明已配好、显示状态，不覆盖凭证和配置。
9. README「部署与退回」改成以 `pony setup` 为主，手写配置的办法留着。

主控建议的取舍（待用户定）：

- 配对码 6 位数字，10 分钟有效，用一次就作废；错 5 次作废，要重跑 setup。码只显示在本机终端，不写日志、不发到 Slack。配对时码不对不回话（免得别人试）。
- 配对时 setup 自己在前台连 Slack 等这句（拿 `pony run` 的那把锁），配好了再装服务；后台 pony 已经在跑时，setup 不做配对，只报状态。
- 不让 pony 自己建频道（要加 Slack 权限、重装应用；用户选了「频道里 @pony 绑」）。

## 范围

- 只动 `crates/pony/`、`docs/DESIGN.md` §10、设计稿里「配置由用户手写」和 3.1 命令表、`docs/CONTRACT.md`（如果 setup 有给程序读的输出），以及本任务文件末尾。
- 共用核心里不出现任何平台名。
- 对 paddock：兼容新增。`pony status` 格式不变；`channels` 为空的配置是新情况，paddock 照现有字段显示即可。

## 怎么算做完

- 「现在我的问题就是像减少问题2的工作量」：新机器上不用手写 `~/.pony` 下的任何文件，不用去 Slack 个人资料复制成员 ID、不用复制频道 ID，按 `pony setup` 的提示走完上面 5 步就能在 Slack 里用。
- 「Slack 的凭证只放在本机，不进任何仓库。」：token 只进 `~/.pony/secrets/slack.toml`，不出现在命令参数、日志、输出里。
- 「不要动ranch中corral的核心代码。」：`crates/corral/` 没有改动。
- 验证：派之前照影响面补。

## 不要做

- 不加 Slack 权限、不让 pony 自己建频道。
- 测试里不连 Slack、不发网络请求，不读真实的 `~/.pony`、`~/.corral`；`HOME`、`PONY_HOME`、`CORRAL_HOME` 都指向临时目录；绝不调真实的 corral，不跑真的 `launchctl`。
- 不碰用户现有的 agent；不部署；不按项目名或路径批量杀进程。
- 不合并到 main，不推送。
