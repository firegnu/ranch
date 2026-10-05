// Stable v2 collector: native facts only. Corral's reader owns turn judgement.
// Loaded code stays in the agent; upgrading Corral changes the reader, not this collector.
import { appendFileSync } from "node:fs";

export default function (api: any) {
  for (const ev of ["session_start", "input", "before_agent_start", "tool_execution_start", "tool_execution_end", "ui_prompt_start", "ui_prompt_end", "message_end", "agent_settled", "session_shutdown"]) {
    api.on(ev, (event: any, ctx: any) => {
      try {
        const path = process.env.CORRAL_EVENTS;
        if (!path) return;
        const record = {
          v: 2, adapter: "pi", ev, t: Date.now() / 1000,
          inst: process.env.CORRAL_INSTANCE ?? "",
          session_id: ctx?.sessionManager?.getSessionId?.(), cwd: ctx?.cwd,
          has_ui: ctx?.hasUI, idle: ctx?.isIdle?.(),
          event: {
            reason: event?.reason, text: event?.text, prompt: event?.prompt,
            streamingBehavior: event?.streamingBehavior, toolName: event?.toolName,
            message: event?.message, messages: event?.messages, willContinue: event?.willContinue,
          },
        };
        appendFileSync(path, JSON.stringify(record) + "\n", { mode: 0o600 });
      } catch {
        // Hook failures must not affect the agent or write to its terminal.
      }
    });
  }
}
