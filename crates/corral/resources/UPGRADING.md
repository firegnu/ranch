# Corral 通用升级

此版本建立未来版本之间的升级协议。现存无能力的 pen、硬编码旧 helper 的会话和没有持久记录的 after 不会因此自动迁移。部署、切入口和首次过渡需要单独安排；打包脚本不执行这些动作。

从新、不可变版本目录调用公开命令，不覆盖正在使用的包：

```sh
/absolute/new-version/bin/corral upgrade --all
/absolute/new-version/bin/corral upgrade project/name --exe /absolute/new-version/bin/corral
/absolute/new-version/bin/corral status project/name
/absolute/new-version/bin/corral recover project/name --exe /absolute/compatible-version/bin/corral
```

`--exe` 缺省为该 CLI 的真实绝对路径。`recover` 只接受 Hold 中的实例，在原 epoch 增加 attempt；不会再 fork 一轮备用进程。请求前做 schema 预检。正常升级保留原 pen PID、agent PID、实例、连接、排队输入、输出、终端模式和绝对定时；普通程序走相同机制，不按 agent kind 选择升级路径。

检查 JSON 的 `result`，不能只看命令退出码或 exe：

| 结果 | 含义 |
|---|---|
| accepted | pen 接受了请求，尚不是升级完成 |
| pending | 尚在交接或收尾；status 的 state 可为 running_new_pending |
| complete | 对应 epoch/attempt/target 已服务，升级状态 none，备用已退出且标记已清理 |
| already_current | 当前映像就是目标，且没有未完成的升级 |
| failed | 请求或该次尝试失败；查看 upgrade.last_error 或 error |
| hold / hold_unprotected | 流被冻结，仅新控制连接接受 status/recover；后者无备用保护 |
| unknown | 丢失回执后仍不能确定本次结果，不能自动重发 |
| needs_restart | 旧版无协议能力；命令不会停止、重启、resume 或注入它 |

`status` 包含 `exe`、`custody`、`capabilities` 和 `upgrade`（epoch、attempt、target、state、result、last_error、保护状态）。批量结果逐个列出 pen 与提醒交接；部分失败不会抹去已经完成的项。升级接收成功的命令可能以退出码 0 返回包含失败项的 JSON，因此部署消费者必须检查全部结果。批量只枚举可观察的 pen 和持久提醒，`legacy_after` 明确标注无法发现或迁移的旧内存 worker。

备用进程只覆盖新映像开始流 I/O 前的窗口。备用接管公开为 `custody: observer`，pen PID 会变化，agent 退出码不可获得时为 null。进入 active 后崩溃不重放旧快照；快照写入到备用建立之间，以及 active 之后的崩溃都没有无损保证。Hold 原有连接保留且冻结，新查询连接才获得控制答复。不要将任意崩溃都描述为会话可保全。

每个 CORRAL_HOME 有独立、绝对的 helper 入口。首次 start 建立入口，公开 upgrade/recover 原子切换它，新启动的旧 CLI 不会把它改回旧版本；旧版本包不改写。未来 Claude/Codex hook 使用这个入口，pi/omp 扩展只记录 v2 原生事实。Rust 读取端承担原文输入关联、回复判断、去抖和重试宽限，同时读取 v1/v2。已经加载的旧扩展和硬编码旧路径不在此能力内。

## 持久提醒

```sh
corral send receiver/name "提醒文本" --after worker/name --timeout 3600
corral after receiver/name --request-id <返回的 request_id>
```

`pending: true` 只表示有记录的等待。记录绑定双方 instance，保留原等待结束/更换实例/超时后尝试交付的语义；接收者非空闲或有人输入时继续等待，`--force` 仅覆盖原有人类保护。等待阶段和后续交付重试阶段分别使用原语义的超时，持久化后都是绝对期限，交接不重置。

阶段为 waiting、sending、sent、confirmed、not_delivered、unknown、expired。每条记录只有锁持有者执行和写入，单次请求结束并记录已知事实后才交接。`sending` 恢复只能按原 request_id 查询接受/完整写入证据；recent 记录有界，缺失、退出或证据不足都转 unknown，不重发、不换 ID 重试。普通程序完整写入后为 sent、confirmed:false。事件匹配保留原 confirmed 含义，不能证明相同文本的两条提醒各自精确完成，也不构成持久接收端去重协议。

只通过这些公开命令操作；Saddle、插件和部署消费者不读取状态目录或传递内部 fd。当前合成验证不等于真实 Claude/Codex/pi/omp 兼容性验证。
