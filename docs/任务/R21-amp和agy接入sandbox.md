# 任务：amp、agy 也能关进 fence 里照常干活

**草稿**（用户 10-19：“加一个draft task，agy和amp的sandbox接入。”）。还没定做不做、什么时候做。派之前要定：
- 两家一起做，还是先做 amp、agy 放后面（主控建议先 amp：agy 很少用，R13 留下的老毛病也没收尾）；
- 验收用不用下面建议的几条；
- 实测要用真实 amp、agy 登录跑几轮，花订阅额度，用户同意了才跑。

2026-10-19，ranch/main 起草，待用户看过、定验收。交给谁、几档、路由，派之前照 corral-dispatch 第 3 节再定。
类型：功能变更
依据：R17b 只给 claude、codex、pi、omp 写了种类数据，amp、agy 带 `--sandbox` 报 `unsupported_kind`。本轮给这两家补数据，在本机（macOS）实测；Linux 不在本轮（R17c 暂缓）。
提示：围绕已确认的使用目标完成变更，优先沿用现有机制。

## 用户原话

- 10-19：“我有个疑问，现在sandbox是不是只是针对适配的几个起作用？”“amp和agy套上难度大吗？”
- 10-19：“加一个draft task，agy和amp的sandbox接入。”
- 10-18（关于严格程度）：“不需要严格限制。我只是在要通过做这个来彻底完全的了解一下sandbox。”
- 10-14（agy）：“主要是想通过agy加进来读屏幕的能力，agy很少用。”

## 先读

- `docs/CONTRACT.md`「sandbox 与 `fence`（R17b）」一节，以及 R18–R20 的段落。
- `crates/fence/README.md`：数据文件的键、合并次序、占位符。
- `crates/fence/data/agents/` 下现有四家，`crates/fence/data/profiles/dev.toml`，`crates/fence/data/services/`。
- `docs/DESIGN.md` §7「全局安装（R6）」「接入 agy（R8）」「agy 派活修整（R13）」，§8 sandbox。
- `docs/调研/R17a-srt小试验.md`：前四家是怎么一家家试出来的。

## 主控已知的情况（10-19 查的，没在墙里试过）

- 两家都是编译好的本机程序（Mach-O arm64）：`/opt/homebrew/bin/amp`，`~/.local/bin/agy`。不是 Node 脚本，认不认 `HTTP_PROXY`/`HTTPS_PROXY` 不知道。srt 靠代理管外网，不认代理就连不上网。
- amp 的目录：`~/.config/amp`（corral 全局插件 `plugins/corral.ts` 在这里）、`~/.local/share/amp`、`~/.cache/amp`。
- agy 的目录：`~/.gemini/antigravity-cli`（信任列表 `settings.json`，pen 在墙外替它补）、`~/.gemini/config`（corral 插件 `plugins/corral/`，`config.json`），可能还有 `~/.gemini` 下别的。agy 走 Google 登录。
- agy 的钩子要运行 pen 给的 `CORRAL_HELPER`，它指向版本目录里的 corral 程序。
- 派活的启动方式：amp 不能带 `--prompt`，等 `idle` 再 `corral send`；agy 必须带 `--prompt`。
- IP Guardian 只包了 codex、claude，碰不到这两家。

## 要做的

1. 照 R17a、R17b 的办法，分别弄清 amp、agy 在墙里要读、要写哪些目录，要连哪些域名，要不要伪终端；凭证存在文件里还是钥匙串。
2. 写 `crates/fence/data/agents/amp.toml`、`agy.toml`；需要新的服务数据就加在 `crates/fence/data/services/`，在 `dev` 档案的 `[services]` 里接上。照现有四家的写法：凭证和设置文件读得到、写不了（`deny_write`），会话和缓存目录可写。
3. 两家关在墙里，corral 插件和钩子照常把状态报给 pen：`corral status` 能看到 working、idle，`corral reply` 拿得到回复。
4. 更新 fence README、CONTRACT 里「有数据的种类」的说法，DESIGN §8 记下两家的结论。

## 怎么算做完（建议，待用户定）

- `corral start --sandbox dev` 起 amp、agy（各开一个 `ranch/test-sandbox-<种类>`，`--label role=test`），在临时合成仓库里各做一件小活：改文件、编译或跑个脚本、提交，`corral status`/`reply` 正常。
- 同一个 agent 在墙里读不到 HOME 里别的东西（用假文件试）、写不了 worktree 外面、连不上没放行的网站，`fence denials` 有记录。
- `fence check --kind amp`、`--kind agy` 不报 `unsupported_kind`。
- 验证预算待派之前按影响面定（数据层碰安全，大概率是「碰要害」：要交叉审查）。

## 不要做

- 不读、不打印、不拷贝真实凭证；验拦截一律用假文件、假密钥。
- 程序不认代理时，不要靠放开全部网络或关掉网络隔离来凑通，停下来报告。
- 要改 fence、corral 代码（不只是数据）才能通的，停下来报告，等决定。
- 不改用户的 amp、agy 设置，不重装、不停用 corral 全局插件；只读它们的目录来弄清布局。
- 不碰 `corral ls` 里用户的 agent；测试 agent 用完 `corral stop`。不按名字或路径批量杀进程。
- 不去 5090，不做 Linux 实测。
- 不合并到 main，不推送。
