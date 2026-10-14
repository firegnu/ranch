// corral's amp plugin. amp loads it for every session, so it does nothing unless corral started
// this amp. It writes corral's version 1 events; CORRAL_INTEGRATION changes with this file.
import { execFileSync } from "node:child_process";
import { appendFileSync } from "node:fs";

const CORRAL_INTEGRATION = 1;

export const description = "corral：只在 corral 启动的 amp 里记录状态，在别处什么都不做。";

// The amp corral started is a child of corral's pen. An amp started from inside it inherits the
// same environment, but its parent is not the pen. The plugin runs in a child of its amp.
function startedByCorral(): boolean {
  const pen = process.env.CORRAL_PEN_PID;
  if (!pen || process.env.CORRAL_KIND !== "amp" || !process.env.CORRAL_EVENTS) return false;
  try {
    const parent = execFileSync("/bin/ps", ["-o", "ppid=", "-p", String(process.ppid)], { encoding: "utf8" });
    return parent.trim() === pen;
  } catch {
    return false;
  }
}

export default function (amp: any) {
  if (!startedByCorral()) return;
  const path = process.env.CORRAL_EVENTS!;
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
  let current = "amp-startup";
  write("SessionStart", current, { source: "startup" });
  // Only top-level threads are the agent's; a subagent's thread has a parent. A thread whose
  // parent cannot be read is skipped this time and asked about again next time.
  const tops = new Map<string, boolean>();
  const top = async (ctx: any): Promise<boolean> => {
    const id = ctx.thread.id;
    if (!tops.has(id)) {
      try {
        tops.set(id, (await ctx.thread.parentThreadID()) === null);
      } catch {
        return false;
      }
    }
    return tops.get(id)!;
  };
  // The agent's session is the thread amp last opened or switched to, or else a top-level
  // thread seen for the first time. Later events of other threads do not take it over.
  const announced = new Set<string>();
  const adopt = (id: string) => {
    announced.add(id);
    if (id !== current) {
      current = id;
      write("SessionStart", id, { source: "startup" });
    }
  };
  const mine = async (ctx: any): Promise<boolean> => {
    if (!(await top(ctx))) return false;
    if (!announced.has(ctx.thread.id)) adopt(ctx.thread.id);
    return true;
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
    if (!(await top(ctx))) return;
    adopt(ctx.thread.id);
    try {
      const last = (await ctx.thread.messages({ from: "end", limit: 1 })).at(-1);
      if (last?.role === "user" && text(last)) submitted(ctx.thread.id, last.id, text(last));
    } catch {
      // agent.start or agent.end records it.
    }
  });
  amp.on("agent.start", async (event: any, ctx: any) => {
    if (await mine(ctx)) submitted(ctx.thread.id, event.id, event.message);
    return {};
  });
  amp.on("tool.result", async (event: any, ctx: any) => {
    if (await mine(ctx)) write("PostToolUse", ctx.thread.id, { tool_name: event.tool });
  });
  amp.on("agent.end", async (event: any, ctx: any) => {
    if (!(await mine(ctx))) return;
    submitted(ctx.thread.id, event.id, event.message);
    if (event.status === "done") {
      write("Stop", ctx.thread.id, { last_assistant_message: reply(event.messages) });
    } else {
      write(event.status === "cancelled" ? "Interrupt" : "StopFailure", ctx.thread.id);
    }
  });
}
