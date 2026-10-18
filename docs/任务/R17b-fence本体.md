# 任务：fence 本体，加 `corral start --sandbox`——四家在本机关进墙里照常干活（需求单；等方案定了再派实现）

（交给谁、几档：待定）
路由：待定
类型：功能变更
依据：用户 10-18 定的方案（`docs/任务/R17-agent放进sandbox.md`、DESIGN §8）和 R17a 实测（`docs/调研/R17a-srt小试验.md`）。本轮在本机做出 fence 和 corral 的接入，四家跑通；Linux 实测留给 R17c。
你是被委派的 agent：照本文件做，不要再开别的 agent。

## 要做的（用户要的结果）
- `corral start ... --sandbox <档案> -- <agent 命令>`：agent 关在 srt 的墙里跑；不加 `--sandbox` 就是现在的样子（用户：“提供sandbox的选项，加了sandbox就是sandbox里面跑，不加就是现在的形式”）。派出去的 agent 默认不加，corral-dispatch 技能不改。
- fence 是单独的一块：`crates/fence`，出自己的命令 `fence`；corral 不在 Cargo 里依赖它，只按一条窄约定调它的命令（用户：“类似插件式的（以后可以很方便的把sandbox剥离出来成为像cairn这样独立的服务）”）。fence 和 corral 打进同一个版本目录。
- 规则：读、写、上网一律默认不许、按需放行。全局档案为主，用占位符（工作目录、编译目录、HOME 等），开 agent 时填真实路径；仓库特有的由仓库里一个小文件补充，只写额外放行。各家要的目录、域名、环境变量和启动参数按 agent 种类各一份数据，模型服务单独一块；以后加一家只加数据，不改 corral、fence 的代码。还没有数据的种类带 `--sandbox` 时报不支持。
- 四家（Claude Code、Codex、pi、omp）的数据照 R17a 的结论写，在本机关在墙里能照常干完一件活（改代码、编译、测试、提交）。R17a 里撞过的墙（Codex 的 trustd、Claude 的钥匙串和临时目录、pi/omp 的锁目录和运行目录、伪终端、编译目录要能读、worktree 提交要放行的几处、Codex 目录信任只对这次生效）都要在数据里照顾到。用户 10-18：“不需要严格限制”，不要求收窄到最小。
- 要支持 Linux：规则和数据要能在 Linux（bubblewrap）上用，写规则只用具体路径、不用通配符，本地 socket 不靠按路径放行。Linux 实测不在本轮。
- 前端用的公开命令（`fence` 自己的命令）：列档案、检查档案、看某个 agent 被拦了什么；`corral status` 写明用的哪个档案。都是兼容的新增，写进 `docs/CONTRACT.md`，paddock 在 P5-120 里接。
- corral 管套了墙的 agent：`stop`、`pause`/恢复、`corral status`/`ls` 判断还在不在、hook 报状态，都照常。
- srt 的版本由 ranch 固定，打包时装进版本目录（用户 10-18 同意）。
- Codex 的数据默认开 `enableWeakerNetworkIsolation`（trustd），档案说明里写明它削弱网络隔离（用户 10-18 同意）。

## 怎么算做完
- 用户 10-18：“按你的建议来，四条都同意”。
- 四家都在本机先跑通（用户：“是的，四家都在本机先跑通”）：Claude Code、Codex、pi、omp 用 `corral start --sandbox` 关在墙里，各干完一件活（改代码、编译、测试、提交）。
- 该拦的确实拦住了（读没放行的密钥文件、写主仓库、连没放行的网站）。
- 被拦的记录找得到（用 `fence` 的命令按 agent 取）。
- 关在墙里时状态照常更新（`corral status`）。
- `corral stop`、暂停和恢复照常。

## 不要做
- 不改 corral-dispatch 技能，不让派出去的默认带墙。
- 不做容器式、云端。不做 paddock 界面。
- 不接 amp、agy 的数据（以后加）。
- 不全局装 srt，不改各家 agent 的全局配置。
- 不部署（不切链接、不装技能），不合并到 main，不推送。

## 怎么派（用户 10-18 同意）
先出方案、再实现：方案任务 `docs/任务/R17b-fence方案.md` 交给 Claude Code，用户看过定了，再把本需求单补成实现任务交给 Codex。
