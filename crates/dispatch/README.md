# ranch-dispatch

任务路由和 `corral-dispatch` 技能，由 `ranch dispatch` 命令使用：

```sh
echo "<三五句任务摘要>" | ranch dispatch route
ranch dispatch install-skills [--target all|claude|codex] [--dry-run] [--yes]
```

`route` 从标准输入读任务摘要，用环境里的 `TYPESAFE_API_KEY` 请求 TypeSafe 的分类模型（`jev-1.13.0`），把建议的档位、要不要交叉审查、影响面写成 JSON 到 stdout，退出码 0；没有 key、摘要不是 UTF-8、调不通或回复不对时，stdout 是 `{"ok":false,"error":…}`，退出码 1。任务书不发出去，只发摘要。路由只给建议，主控按项目规则和当次授权做最终决定。`install-skills` 见 [技能说明](resources/corral-dispatch/README.md)。

## 来源

从 Saddle 内置 dispatch 插件迁来：Saddle 提交 `c21674a` 的 `plugins/dispatch`（用户 10-05 定：dispatch 拆到 ranch，不要遥测）。改动：

- 去掉插件外壳（`crates/core-plugin` 的 manifest 和 `Recorder`）和路由时的遥测采集（`--record-context`、`--brief-file`、stderr 回执）；路由规则、请求、重试和输出不变。
- 技能删去 `遥测操作.md` 和讲遥测的段落，路由命令改成 `ranch dispatch route`；`项目AGENTS模板.md` 逐字节保留（有测试固定）。
- 安装改由 `ranch dispatch install-skills`，照 corral 的 `install-skills` 做法，不再用 Saddle 的插件资源安装和归属记录。

路由语义来自 Corral `6923da1ee0c766468c31e1f577fb8370b8b6da77` 的 route.py（SHA-256 `d4ab1cc82fe5c1b10987800eaf1c44d1e1fe026e7b1227b0eeb508cce548167a`），规则原文在 `src/rules.rs`，测试固定了规则指纹。请求用有字段顺序的类型，响应用局部有序 JSON（不开 serde_json 的 preserve_order）；数字舍入对二进制值取三位小数、平局取偶。

HTTP 用精确固定的 ureq 3.4.2（只开 rustls），固定 URL 和模型、校验证书、不跟随重定向。每个阶段 10 秒，最多两次尝试：只有第一次遇到 429/529 时等 1 秒再试；发送完成前的错误和超时立即重试一次，真正发完后的非超时接收错误不重试。响应最多读 16 MiB。错误说明不带库错误原文、请求头或 key。

## 测试

只用合成输入、假传输、本机回环服务器和临时 HOME，不联网。`tests/fixtures/tls-cert.pem`、`tls-key.pem` 是 ureq 3.4.2 发行包里公开的测试证书和私钥，只在内存 TLS 测试里用，那里关闭了证书校验；生产代码始终校验。
