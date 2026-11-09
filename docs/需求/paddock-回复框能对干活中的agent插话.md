# paddock 需求：收件箱回复框能对正在干活的 agent 插话

2026-11-09，ranch/main 写，交用户转给 paddock。ranch 不用改，用现装 corral（`versions/864d900`，R57b）就能做；命令、结果和退出码以 ranch `docs/CONTRACT.md`「`corral send NAME TEXT --steer`（R57b）」一节为准。

## 用户原话

- 11-09：「现在委派出去的agent在工作的时候是没有办法send过去东西的，也就是说不能插话。我的理解是这块设计的有问题，我理解的是是可以插话的。」
- 11-09（corral 做完后主控说 paddock 回复框还只能回空闲的 agent）：「为什么还有这么多没做的？」「可以，两个都写草稿给我看」

## 要的结果

收件箱的回复框，对正在干活的 agent 也能发；发出去的话插进它当前这一轮，它在下一步就能看到，不用等这一轮做完。

## 现在怎样

paddock `docs/DESIGN.md` P5-87「回复框」：只有空闲的 agent 能回，别的状态框换成一句说明；后台调 `corral send <全名> <文字>`。

## 要改的

- **对方空闲**：和现在一模一样，不加 `--steer`。
- **对方在干活**，且它的 `corral status` 里 `capabilities.steer` 是 `1`：回复框照常可用，发送时调 `corral send <全名> <文字> --steer`。其余照现在的规矩：文字原样一个参数，不加 `--force`，不在画界面的线程里等。
- **对方在等你点框（blocked）、启动中、暂停**：和现在一样，框换成一句说明。
- `capabilities.steer` 不是 `1`（agent 是旧版 corral 开的、没 upgrade）：干活时和现在一样不给发，说明里可以提一句「升级后才能插话」。

## 结果怎么显示

| corral 返回 | 意思 | paddock 怎么做 |
|---|---|---|
| 成功（0） | 插进去了 | 和现在发成功一样：框清空、留 `You · sent …`；新回复要等它这一轮全部做完才有，那一块留到那时 |
| 3 `draft_not_submitted` | 字已经写进对方输入框，但没按提交（对方状态变了，例如弹了框） | **不要**把字留在框里让人再发，会重复。框清空，提示「写进去了但没提交，去窗格里看一下」，给 `Open pane` |
| 3 `not_delivered` | 送了但没确认收到 | 和现在一样，不重发 |
| 7 `not_idle`、`state_changed` | 没写进去（对方刚好变了状态） | 字留在框里，写原因，可以再发 |
| 8 `human_active` | 30 秒内有人在它窗口里打过字 | 和现在一样 |
| 9 `unsupported_steer` | 这家 agent 还不支持干活时插话（目前只有 Claude Code、Codex、omp 支持） | 字留在框里，提示「这家还不支持干活时插话，等它空下来再发」 |
| 9 `unsupported` | 栏位进程太旧，或终端不支持 | 字留在框里，提示等空闲再发 |
| 1 `usage` | 干活时以 `/`、`!` 开头的话不能插，或 PATH 上的 corral 太旧、不认识 `--steer` | 字留在框里，写原因；旧 corral 时提示等空闲再发，**不要**改用 `keys` 硬送 |

## 不做

- 不改 Esc、打断这类按钮，不改别的界面。
- 只调 PATH 上的 `corral` 命令、读它的 JSON，不读 `~/.corral`。
- pony（Slack）的插话 ranch 另做，和这件无关。
