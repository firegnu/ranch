# paddock 需求：设置 › Sandbox 页显示查资料用的是哪家

2026-10-20，ranch/main 写，交用户转给 paddock。ranch 不用改，用现装 fence（`versions/2f52652`）的现有输出就能做；字段以 ranch `docs/CONTRACT.md`「sandbox 与 `fence`」一节为准。

## 用户原话

- 10-20：「另外你觉得settings是否可以加上search的配置？」
- 10-20：「按你的建议，只做显示，测试按钮先不做」

## 要的结果

Sandbox 页上看得出套墙的 agent 查资料用的是哪一家，以及它是内置的还是本机加的，例如「查资料：exa（内置）」。

## 从哪里拿

- 有哪些：`fence profiles` 的 `mcp` 数组，每项 `name`、`source`（`builtin` 内置，否则是本机加的）。
- 某个档案实际用哪家：`fence check --profile <档案> --kind <种类>` 的 `layers` 里 `source` 为 `mcp` 的项（`name`、`origin`）。没有这一项就是没接查资料的服务器。

## 不做

- 不能在界面里换服务商、添加删除、填 key，也没有测试按钮（用户 10-20 说先不做）。要换服务商照 ranch `crates/fence/README.md` 手改 `~/.fence/`。
- 只调 PATH 上的 `fence` 命令、读它的 JSON，不读写 `~/.fence` 里的文件。
