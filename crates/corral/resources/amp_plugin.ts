// corral's amp plugin. amp loads it for every session, so it does nothing unless corral started
// this amp. It writes corral's version 1 events; CORRAL_INTEGRATION changes with this file.
import { appendFileSync } from "node:fs";

const CORRAL_INTEGRATION = 1;

export const description = "corral：只在 corral 启动的 amp 里记录状态，在别处什么都不做。";

export default function (amp: any) {
  const path = process.env.CORRAL_EVENTS;
  if (!path || process.env.CORRAL_KIND !== "amp") return;
  const inst = process.env.CORRAL_INSTANCE ?? "";
  const cwd = process.env.CORRAL_CWD ?? "";
  const write = (ev: string, thread: string, extra: object = {}) => {
    try {
      const record = { v: 1, t: Date.now() / 1000, ev, inst, session_id: thread, cwd, has_transcript: true, version: CORRAL_INTEGRATION, ...extra };
      appendFileSync(path, JSON.stringify(record) + "\n", { mode: 0o600 });
    } catch {
      // Recording must never affect amp.
    }
  };
  // amp has no thread until the first prompt, but corral sends only to an agent that is ready.
  // The first top-level thread then takes over from this placeholder session.
  write("SessionStart", "amp-startup", { source: "startup" });
  // Only top-level threads are the agent's; a subagent's thread has a parent.
  const threads = new Map<string, Promise<boolean>>();
  const watch = (ctx: any): Promise<boolean> => {
    const id = ctx.thread.id;
    let top = threads.get(id);
    if (!top) {
      top = Promise.resolve()
        .then(() => ctx.thread.parentThreadID())
        .then((parent: unknown) => parent === null, () => true)
        .then((isTop: boolean) => {
          if (isTop) write("SessionStart", id, { source: "startup" });
          return isTop;
        });
      threads.set(id, top);
    }
    return top;
  };
  // amp skips agent.start for a turn that begins while it is still loading plugins. Such a turn
  // still starts a thread, so its prompt is read from the thread, or else recorded at its end.
  // Each user message is recorded once.
  const seen = new Set<string>();
  const submitted = (thread: string, id: unknown, prompt: unknown) => {
    const key = `${thread}/${String(id)}`;
    if (seen.has(key)) return;
    seen.add(key);
    write("UserPromptSubmit", thread, { prompt });
  };
  const text = (m: any): string =>
    Array.isArray(m?.content)
      ? m.content.filter((b: any) => b?.type === "text").map((b: any) => b.text).join("\n")
      : "";
  const reply = (messages: any[]): string | undefined => {
    for (const m of [...(messages ?? [])].reverse()) {
      if (m?.role === "assistant" && text(m)) return text(m);
    }
    return undefined;
  };

  amp.on("session.start", async (_event: any, ctx: any) => {
    if (!(await watch(ctx))) return;
    try {
      const last = (await ctx.thread.messages({ from: "end", limit: 1 })).at(-1);
      if (last?.role === "user" && text(last)) submitted(ctx.thread.id, last.id, text(last));
    } catch {
      // agent.start or agent.end records it.
    }
  });
  amp.on("agent.start", async (event: any, ctx: any) => {
    if (await watch(ctx)) submitted(ctx.thread.id, event.id, event.message);
    return {};
  });
  amp.on("tool.result", async (event: any, ctx: any) => {
    if (await watch(ctx)) write("PostToolUse", ctx.thread.id, { tool_name: event.tool });
  });
  amp.on("agent.end", async (event: any, ctx: any) => {
    if (!(await watch(ctx))) return;
    submitted(ctx.thread.id, event.id, event.message);
    if (event.status === "done") {
      write("Stop", ctx.thread.id, { last_assistant_message: reply(event.messages) });
    } else {
      write(event.status === "cancelled" ? "Interrupt" : "StopFailure", ctx.thread.id);
    }
  });
}
