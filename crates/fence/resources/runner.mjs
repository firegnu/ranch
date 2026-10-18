// Runs in srt's Node runtime, outside the sandbox. Policy belongs to fence's TOML data.
import { spawn } from 'node:child_process';
import { readFileSync, writeFileSync, appendFileSync, renameSync } from 'node:fs';
import { dirname, join } from 'node:path';

const argv = process.argv.slice(2);
const separator = argv.indexOf('--');
const options = Object.fromEntries(Array.from({ length: separator / 2 }, (_, i) => [argv[i * 2], argv[i * 2 + 1]]));
const exitPath = options['--exit'];
const monitorPath = join(dirname(exitPath), 'monitor.json');
const monitors = { network: false, files: false };
let truncated = false;
let bytes = 0;
let manager;
const signals = ['SIGHUP', 'SIGINT', 'SIGTERM'];
// pen signals the entire process group. Let the agent decide how and when to exit.
for (const signal of signals) process.on(signal, () => {});
const save = (path, value) => {
  writeFileSync(`${path}.tmp`, JSON.stringify(value) + '\n', { mode: 0o600 });
  renameSync(`${path}.tmp`, path);
};
const saveMonitor = () => save(monitorPath, { monitors, truncated });
const quote = value => `'${value.replaceAll("'", "'\\''")}'`;
try {
  const { SandboxManager, SandboxRuntimeConfigSchema } = await import('../../lib/srt/node_modules/@anthropic-ai/sandbox-runtime/dist/index.js');
  manager = SandboxManager;
  const settings = SandboxRuntimeConfigSchema.parse(JSON.parse(readFileSync(options['--settings'], 'utf8')));
  const store = manager.getSandboxViolationStore();
  let count = 0;
  store.subscribe(events => {
    const added = store.getTotalCount() - count;
    count = store.getTotalCount();
    if (!added || truncated) return;
    for (const event of events.slice(-added)) {
      const detail = event.line;
      const network = detail.match(/deny network-outbound (\S+)/);
      const file = detail.match(/\b(file-read|file-write)[^ ]*\s+(.+?)(?:\s+CMD64_|$)/);
      // srt 0.0.80's Linux monitor emits only write-intent syscalls, not reads.
      const linuxWrite = detail.match(/^deny [a-z][a-z0-9_]* (\/.*)$/);
      const type = linuxWrite ? 'file-write' : network ? 'network' : file ? file[1] : 'other';
      if ((file || linuxWrite) && !monitors.files) { monitors.files = true; saveMonitor(); }
      // Do not persist event.command or encodedCommand (the command includes the prompt).
      const line = JSON.stringify({ t: event.timestamp.getTime() / 1000, type, target: linuxWrite?.[1] ?? network?.[1] ?? file?.[2] ?? null, detail }) + '\n';
      const size = Buffer.byteLength(line);
      if (bytes + size > 5 * 1024 * 1024) { truncated = true; saveMonitor(); break; }
      appendFileSync(options['--denials'], line, { mode: 0o600 });
      bytes += size;
    }
  });
  await manager.initialize(settings, undefined, true);
  monitors.network = true;
  // srt exposes no monitor-health API. Only observed file events prove file logging works.
  saveMonitor();
  const command = argv.slice(separator + 1).map(quote).join(' ');
  const wrapped = await manager.wrapWithSandbox(`exec ${command}`);
  const child = spawn('/bin/sh', ['-c', `exec ${wrapped}`], { stdio: 'inherit' });
  const result = await new Promise((resolve, reject) => {
    child.once('error', reject);
    child.once('exit', (code, signal) => resolve({ code, signal }));
  });
  await manager.reset();
  save(exitPath, { ...result, t: Date.now() / 1000 });
  if (result.signal) {
    for (const signal of signals) process.removeAllListeners(signal);
    process.kill(process.pid, result.signal);
  } else {
    process.exitCode = result.code;
  }
} catch (error) {
  console.error(`fence: ${error.message}`);
  if (manager) await manager.reset().catch(() => {});
  save(exitPath, { code: 125, signal: null, t: Date.now() / 1000 });
  process.exitCode = 125;
}
