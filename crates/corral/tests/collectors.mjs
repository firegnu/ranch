// Synthetic extension API only; no pi/omp process, account or user state.
import assert from "node:assert/strict";
import { mkdtempSync, readFileSync, rmSync } from "node:fs";
import pi from "../resources/hook_pi.ts";
import omp from "../resources/hook_omp.ts";

const root = mkdtempSync("/tmp/corral-collectors-");
try {
  process.env.HOME = root;
  process.env.CORRAL_HOME = `${root}/pens`;
  process.env.CORRAL_INSTANCE = "synthetic";
  for (const [adapter, install] of [["pi", pi], ["omp", omp]]) {
    process.env.CORRAL_EVENTS = `${root}/${adapter}.events`;
    const callbacks = new Map();
    install({ on: (name, callback) => callbacks.set(name, callback) });
    const ctx = { cwd: "/tmp", hasUI: false, isIdle: () => true,
      sessionManager: { getSessionId: () => "session" } };
    const examples = [
      ["input", { text: "original", streamingBehavior: "steer" }],
      ["before_agent_start", { prompt: "expanded" }],
      ["message_end", { message: { role: "assistant", content: [{ type: "text", text: "reply" }] } }],
      [adapter === "pi" ? "agent_settled" : "agent_end",
        { willContinue: true, messages: [{ role: "assistant", stopReason: "error", errorMessage: "503 unavailable" }] }],
    ];
    for (const [event, body] of examples) callbacks.get(event)(body, ctx);
    const records = readFileSync(process.env.CORRAL_EVENTS, "utf8").trim().split("\n").map(JSON.parse);
    assert.equal(records.length, examples.length);
    for (let i = 0; i < records.length; i++) {
      assert.equal(records[i].v, 2);
      assert.equal(records[i].adapter, adapter);
      assert.equal(records[i].ev, examples[i][0]);
      assert.deepEqual(records[i].event, examples[i][1]);
      assert.equal(records[i].has_ui, false); // reader decides session eligibility
      assert.equal(records[i].idle, true);
    }
    process.env.CORRAL_EVENTS = `${root}/absent/events`;
    assert.doesNotThrow(() => callbacks.get("input")({ text: "fail open" }, ctx));
  }
  console.log("pi/omp v2 collectors: passed");
} finally {
  rmSync(root, { recursive: true, force: true });
}
