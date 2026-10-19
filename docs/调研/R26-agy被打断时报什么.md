# R26：agy 被打断时报什么

2026-10-19，本机 agy 1.4.2，默认模型 Gemini 4 Argon (High)，已安装 corral `06e2c27`。仅调研，不改代码、公开约定或部署；paddock 不用动。

## 方法与证据范围

只开 `ranch/test-interrupt-agy`，隔离 CORRAL_HOME，HOME 下临时合成工作目录，带 `CAIRN_DISABLE=1`、`--prompt`、`-- agy --dangerously-skip-permissions`，不带 label。两次 Esc 和后续输入由真人在 `corral attach` 的终端中操作；没有用 keys 模拟。

启动前将隔离目录的 `.runtime/helper` 指向安装版 corral，让 pen 选择这个共享路径；start 返回后、首个钩子到来前，将该隔离链接替换为临时 shell 采集器。采集器先把 stdin 原样保存，再以 `exec` 将同一输入交给原版 corral 的 `__hook`，保持父进程关系。没有改全局插件、安装版程序或用户设置。采到了 SessionStart、PreInvocation 和两次正常 Stop，corral 也收到转换后的事件，说明采集链路有效；不能据此保证 agy 内部从未尝试过未到达采集器的钩子。

仅阅读本测试会话的 transcript 和经该 agy PID 的 lsof 确认的 `cli-20261019_193230.log`。下文时间均为本地 UTC+8；路径、会话号、对话正文不收入报告。原始资料在停止前通过 read/reply 和上述钩子采集保存，整理后删除临时副本。没有读取其他会话或凭证。

## 1. 生成文字时 Esc

**实测。** 19:32:37 收到 SessionStart、PreInvocation（invocationNum=0）。模型未调用工具，正在生成游记；19:32:56 真人按 Esc。

- 截至下一次真人输入前，没有收到 Stop，也没有 `terminationReason: USER_CANCELED`。
- 停下后的 corral status 仍为 `working`、`interrupted: false`，最后事件 UserPromptSubmit；reply 报 `no_reply`，原因是此前没有任何已结束回复。
- transcript 末尾是截断的 PLANNER_RESPONSE，`source: MODEL`、`status: DONE`，不是取消枚举；前面是 USER_INPUT / USER_EXPLICIT / DONE。没有新增取消步骤。正文中途截断不能作为可靠判据。
- agy 日志同一时刻明确写了以下两条（会话号已删）：

```text
19:32:56.589212 Cancelling in-progress response for conversation <session>
19:32:56.589295 Cancelling conversation <session> (killBackgroundTasks=false)
```

这是日志中的取消请求记录；不是成功终止的结构化钩子。

## 2. 命令仍在运行时 Esc

**实测，但有重要边界。** 真人要求前台运行 `sleep 300`、不放后台。19:34:00 开始这一轮；实际工具参数为 `run_command`、`IsDaemon: false`、`WaitMsBeforeAsync: 10000`。19:34:06 创建工具步骤，约 10 秒后 agy 自行返回后台任务说明，19:34:16 又来了 invocationNum=1 的 PreInvocation。19:34:20 真人按 Esc。

- 该轮收到两个 PreInvocation，没有 PostToolUse，也没有 Stop / USER_CANCELED。
- 下一条真人输入前的 transcript 末尾仍是 `type: GENERIC`、`source: MODEL`、`status: RUNNING`，内容说明工具作为后台任务运行。其前一个 PLANNER_RESPONSE / DONE 保存 run_command 调用。没有明确取消步骤或取消状态。
- 日志有以下记录：

```text
19:34:20.387189 Cancelling in-progress response for conversation <session>
19:34:20.387314 Cancelling conversation <session> (killBackgroundTasks=false)
```

- 后续短回复完成时，终端仍显示 `sleep 300 running`。没有证据表明 Esc 停掉了这个命令；这次取消的是命令仍在运行期间的回复。没有在第二次 Esc 与紧接着的新输入之间单独采 status，不能把最终 idle 当成打断后的状态。

**与 R22 对比（本轮实测 + 读取 R22 完成记录）。** 相同的是没有结束钩子、日志记取消且不杀后台任务、对话记录没有明确取消标记。不同的是 R22 记录 corral 最后事件为 PostToolUse，本轮采集器未收到 PostToolUse；本轮明确观察到工具自动转后台。这两次不能写成完全相同的工具执行路径。

**未知。** 真正仍在工具同步等待阶段（本轮的前 10 秒内）按 Esc 会不会发 Stop，本轮没有覆盖。按任务只实测一遍，没有自行增加重测。

## 3. 打断后继续输入，正常结束

**实测。** 两次都收到了原始 Stop；下列 JSON 是实际输入的字段摘录，不是合成测试数据。两次共同还有 `artifactDirectoryPath`、`conversationId`、`modelName`、`transcriptPath`、`workspacePaths`，因路径与会话信息略去其值。输入没有对话正文。

文字打断后，只回复标记的一轮，19:33:50：

```json
{
  "error": "",
  "executionNum": 0,
  "fullyIdle": true,
  "terminationReason": "NO_TOOL_CALL"
}
```

命令运行期间打断后，只回复标记的一轮，19:34:33：

```json
{
  "error": "",
  "executionNum": 0,
  "fullyIdle": false,
  "terminationReason": "NO_TOOL_CALL"
}
```

两次 transcript 最后回复均为 PLANNER_RESPONSE / MODEL / DONE，内容分别为请求的合成标记。最终 corral 为 idle、interrupted=false、last_event=Stop、last_input_source=human，reply 正确返回第二个标记。

第二个 fullyIdle=false 与屏幕仍显示后台命令运行相符（关联解释，不是单凭字段证明原因）。它说明不能把 fullyIdle=false 直接当作用户打断；这条 Stop 对应的是正常完成的短回复。

原始钩子总计 8 次：SessionStart 1、PreInvocation 5、Stop 2、PostToolUse 0。PreInvocation 除共同字段外有 initialNumSteps、invocationNum；四轮的 invocationNum 依次为 0、0、0/1、0。两次 Esc 本身均无原始 Stop 输入可附，不能捏造 USER_CANCELED 样本。

## 4. corral 能靠什么识别

**读代码事实。** `crates/corral/resources/agy_hooks.json` 安装 SessionStart、PreInvocation、PostToolUse、Stop。`crates/corral/src/agents/agy.rs` 的 `hook` 已将 Stop 的 USER_CANCELED 转为 Interrupt，将 ERROR 转为 StopFailure，其余转 Stop；`turn` 只读 USER_INPUT 和 PLANNER_RESPONSE 的正文，不判断取消。当前转换在真的收到 USER_CANCELED 时已足够，本轮不缺一个枚举分支。

**本轮结论。** 已测两次打断均没有可供现有钩子转换使用的明确事件；transcript 中 DONE 和 RUNNING 都不能可靠区分正常与取消。因此不能靠本轮观察到的钩子或 transcript 让 corral 认出这两次打断。此结论限于上述版本和操作路径，不推广到 agy 所有取消路径。

**有依据的候选方案，而非已验证实现。** agy 自己的日志两次都明确记录了对应会话的取消请求，是可以继续调查的信号来源。但那是自由文本诊断日志，不是公开的事件协议，也不是取消完成确认。本轮没有验证它是否只由真人 Esc 触发、其他取消来源是否复用、失败或竞态是否仍写同一句，以及日志轮转和多会话归属的规则。不能直接宣称扫描这一句就能可靠识别「真人打断并已结束」。

若 agy 修复/补发已有 Stop / USER_CANCELED，corral 现有转换无需改动；若走日志兜底，则需新增日志定位、会话与当前轮关联、增量读取及兼容性处理和回归验证，不是一行映射的小改，初步属中等范围（推测，尚不足以估工时）。可靠性前提未确认，不能作为本轮实施建议。屏幕文字、半截正文、静默时间和 fullyIdle=false 均不应替代明确取消事件。

## 清理、验证与待决定

- 仅做任务指定的这遍真人实测与 `git diff --check`，未编译、未跑额外测试、未重测同步工具阶段。
- read/reply 证据保存后，以 corral stop 停止本测试实例（SIGHUP，exit_code=0）；隔离 corral ls 返回空。
- pen 自动加入的信任条目为 `$HOME/r26-agy-probe.O38o74/work`，停止后确认已撤回。未手改 agy 设置。
- 删除临时工作目录、隔离 CORRAL_HOME、采集器及原始证据副本；agy 自己写入的本测试会话目录和 CLI 日志也在取证后删除。未动其他会话。
- 未改代码、部署、合并或推送。paddock 不用动。
- 交主控/用户决定：是否需要补测同步工具等待阶段，或继续查日志能否成为可靠的取消信号；本轮不自行扩展调查或实现。
