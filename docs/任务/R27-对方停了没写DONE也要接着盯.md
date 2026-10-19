# 任务：被委派的 agent 停下来等人、人在它窗口里接着聊，最后写了 DONE，主控也要接得到

2026-10-19，ranch/main 交给 ranch/dev-r27（Codex，gpt-6-astra / medium）。
路由：常规 / 交叉审查不要 / 影响面：看得见（路由：常规（置信 0.15）、要、碰要害；推翻：只改技能和 guide 的文字，程序行为不变，写错了下一次派活就看得出、随时能改回，按看得见走，不交叉审查）
类型：Bug 修复
依据：`send --after` 提醒是一次性的，对方任何一轮结束就送出。技能里「收到提醒后，对方停了但没写 DONE」只写了告诉人去 attach 看，没写再挂提醒；人在对方窗口里接着聊、它最后写了 DONE，主控已经没人盯着，接不到。本轮只改技能文字，不改 corral 程序的行为。
提示：依据证据定位并修复导致问题的原因，保持无关行为不变。
你是被委派的 agent：照本文件做，不要再开别的 agent。

## 用户原话

- 10-19：“在测试中我发现另外一个问题。场景就是你刚才开了agent，之后那个agent在他的会话里面和我交流（不是我esc打断他和她交流），最后他写done，你接不到”
- 10-19：主控列了甲（只改技能文字：没写 DONE 也再挂提醒）、乙（给 corral 加「等到 DONE 才送」的参数）两种改法，用户：“可以，按甲写任务草稿”。

## 先读

- `AGENTS.md`「规矩」，特别是「改动要写明对 paddock 的影响」。
- `crates/corral/resources/SKILL.md`「长任务：交出去不等，做完被提醒」一节（第 4 步收到提醒后的几种情况）。
- `crates/corral/resources/AGENT_USAGE.md`「长任务：交出去不等，做完被提醒」一节（`corral guide` 的来源）。
- `crates/dispatch/resources/corral-dispatch/SKILL.md` 第 4 节「派出去」（开完运行 `corral wait <名字> --timeout 60` 那条、挂提醒那条）和第 5 节开头收到提醒后怎么判断的那几句。
- 两份技能和 guide 怎么编进程序、有没有测试看它们的内容：`crates/corral`、`crates/dispatch` 里引用这几个文件的地方。

## 主控查到的（10-19，R25、R26 两次实际碰到）

- R25：开发 agent 停下来请人按 Esc，提醒送到主控，主控把步骤转给用户。用户直接在开发 agent 窗口里跟它说话，它又跑了几轮，最后写了 DONE；这时提醒已经用掉，主控接不到，是用户回来说「那边结束了」才知道。
- R26：开完 agent 前台 `corral wait --timeout 60`，它在 60 秒内就停下来请人按 Esc，`wait` 返回 `idle`；主控转完话没再挂提醒，同样接不到最后的 DONE。
- 主控是 Claude Code 时另有后台 `corral wait` 循环作第二条通道，它也在对方第一次停下时就结束了。

## 要做的

1. 收到提醒（或后台 wait 叫醒）时，对方停了但最后一行不是 DONE，比如在等人操作、在问问题、卡在 `blocked`：照旧告诉用户，同时再挂一次提醒（主控是 Claude Code 时连后台 wait 一起）。这样人不管是在对方窗口里直接聊，还是通过主控转话，对方下一次停下时主控都会醒，接着按同样的规则判断，直到看到 DONE。对方已经退出、不在了，不再挂。
2. 开完 agent 时前台的那次 `corral wait`（corral-dispatch 第 4 节），如果对方已经停了，照收到提醒那样判断和处理，不能当成「起来了」就结束。
3. 被打断的那条（`interrupted: true`）照现有说法，不改；只在需要时补一句后台 wait 也要重挂。
4. 改 corral 技能（`SKILL.md`）、`corral guide`（`AGENT_USAGE.md`）、corral-dispatch 技能里相关的那几句，三处说法一致。写清对 paddock 的影响。

## 怎么算做完

- 用户原话那个场景：主控开了 agent，它停下来等人，人在它窗口里和它交流，最后它写 DONE，照新的技能做的主控接得到。
- 验证只做这些：`git diff --check`；`corral guide` 和两份技能装出来的内容（用编译目录里的程序 `install-skills --dry-run` 到隔离 HOME，或直接看编进去的文本）是新的说法；如果有测试检查这几份文本，跑那几个测试。觉得不够，在回复里说，不要自己加。

## 不要做

- 不改 corral、ranch 的程序行为（`send --after`、`wait` 等命令和参数都不改），不加新参数。
- 不改别的段落：只动收到提醒后的判断、挂提醒、开完后那次 wait 相关的句子。
- 不改 paddock。
- 不碰用户现有的 agent（`corral ls` 里不是你开的）：不 `stop`、`send`、`keys`、`pause`、`upgrade`，不 attach 打字。
- 不部署：不切 `~/.local/bin` 的链接、不装技能、不往 `~/.local/share/ranch/versions/` 打包、不 `corral upgrade`。
- 不要按项目名或路径批量杀进程（`pkill -f corral`、`pkill -f ranch` 这类）：主控和别的 agent 的进程命令行里都带着项目名和工作目录，一条命令能把它们全杀掉。停自己起的进程用记下的 PID 或 `corral stop`。
- 不合并到 main，不推送。只在 `r27-rearm-reminder` 上提交。

## 做完

在本文件末尾追加「## 完成记录」（在你的分支里提交）：做了什么、验证了什么、拿主意的地方、没做的事、踩过的坑，各几句话。踩过的坑指试了不行、换了办法的，工具或环境的行为和预想不一样的；没有就写「没有」，不要凑。回复里只写这几样，加上有没有要主控决定的事。命令都在前台跑完，全部做完后，回复最后一行写 DONE。
