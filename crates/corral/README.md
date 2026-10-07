# Corral core

Rust agent runtime shipped with ranch and used by the Saddle and paddock front ends.
This package builds independently and has no front-end, plugin, task, routing or
telemetry dependencies. It does not execute the old
Corral CLI or require a Python interpreter. Its public executable remains `corral`.

```sh
cargo build -p corral-core --bin corral
cargo test -p corral-core --all-targets
```

The CLI and pen protocol v1 retain the original Corral baseline `6923da1`.
The reader accepts event formats v1/v2 and replays older cursors into cursor v4.
The original repository remains unchanged. New Claude/Codex hooks use the namespace's
stable helper entry; pi/omp load fact collectors whose interpretation lives in Rust.

Pens outlive clients and TUI instances. Use `corral attach NAME` independently;
Ctrl-] detaches, while explicit `corral stop NAME` stops the agent. New binaries
must live in immutable version directories: do not overwrite or remove a binary
still referenced by a running pen or hook. `cargo run -p ranch-package` builds
an immutable version directory (`~/.local/share/ranch/versions/<commit>/` by default)
without installing anything. Changing global links/configuration is a separate deployment operation.

`corral pause NAME` stops the agent with SIGSTOP, together with every descendant and every process
in a session one of them is in (so children adopted by init after their parent exited are included;
only a daemon that detaches into a new session at that very moment can escape); `corral resume NAME` continues them with SIGCONT. `status` and `ls`
report `paused`; while paused, send and keys are refused (exit 10) and attached typing is dropped.
Older corral binaries do not read `paused`: their `wait` and `send --after` misjudge a paused
agent. Switch the public link and run `corral upgrade --all` (pens and reminders together) before
pausing anything; see `docs/DESIGN.md` §6 of the repository.

`corral upgrade --all` upgrades capable pens and persistent reminders through public
interfaces; `corral recover NAME` retries a Hold in the same epoch. Read every result,
including pending, unknown and needs_restart. `corral after NAME --request-id ID`
reports a reminder's durable phase. See [upgrade contract](resources/UPGRADING.md)
for resource ownership, failure windows, result meanings and the first-transition limit.

Tests use isolated HOME/CORRAL_HOME and copied, fixed runtime binaries. No real
coding agent or user state is required. Optional ignored interoperability tests
accept `CORRAL_COMPAT_BIN` pointing to an unchanged external v1 baseline; this is
only a comparison tool, never a build or runtime dependency.

`node crates/corral/tests/collectors.mjs` exercises both shipped TypeScript
collectors with a synthetic extension API (Node with native type stripping).

`install-skills` retains consent and ownership checks. Existing symlinked skills
are reported as foreign and left untouched, so a source-repository link cannot
make installation write into that repository. Deployment must handle these links
explicitly. The command never installs skills on ordinary startup.

## Source

Moved from Saddle `crates/corral-core` at commit `a31dea2` (unchanged since `9648ff0`).
Each Rust file names its original path. `resources/` and `tests/collectors.mjs` were
byte-for-byte copies without a header, because the resources are compiled into the
program or installed into user directories. Files that ranch has changed since say so in
their header; `resources/AGENT_USAGE.md` and `resources/SKILL.md` gained pause and resume. Saddle's `tests/product.rs` was not moved:
it tests the Saddle terminal host together with corral. The two design documents in
the repository's `docs/` come from Saddle `docs/` at the same commit.
