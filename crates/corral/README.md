# Corral core

Rust agent runtime shipped with Saddle. This package builds independently and has no
Saddle, plugin, task, routing or telemetry dependencies. It does not execute the old
Corral CLI or require a Python interpreter. Its public executable remains `corral`.

```sh
cargo build --manifest-path crates/corral-core/Cargo.toml --bin corral
cargo test --manifest-path crates/corral-core/Cargo.toml --all-targets
```

The CLI and pen protocol v1 retain the original Corral baseline `6923da1`.
The reader accepts event formats v1/v2 and replays older cursors into cursor v4.
The original repository remains unchanged. New Claude/Codex hooks use the namespace's
stable helper entry; pi/omp load fact collectors whose interpretation lives in Rust.

Pens outlive clients and TUI instances. Use `corral attach NAME` independently;
Ctrl-] detaches, while explicit `corral stop NAME` stops the agent. New binaries
must live in immutable version directories: do not overwrite or remove a binary
still referenced by a running pen or hook. `scripts/package.sh NEW_DIRECTORY`
builds the host, runtime and official process plugins together without installing
anything. Changing global links/configuration is a separate deployment operation.

`corral upgrade --all` upgrades capable pens and persistent reminders through public
interfaces; `corral recover NAME` retries a Hold in the same epoch. Read every result,
including pending, unknown and needs_restart. `corral after NAME --request-id ID`
reports a reminder's durable phase. See [upgrade contract](resources/UPGRADING.md)
for resource ownership, failure windows, result meanings and the first-transition limit.

Tests use isolated HOME/CORRAL_HOME and copied, fixed runtime binaries. No real
coding agent or user state is required. Optional ignored interoperability tests
accept `CORRAL_COMPAT_BIN` pointing to an unchanged external v1 baseline; this is
only a comparison tool, never a build or runtime dependency.

`node crates/corral-core/tests/collectors.mjs` exercises both shipped TypeScript
collectors with a synthetic extension API (Node with native type stripping).

`install-skills` retains consent and ownership checks. Existing symlinked skills
are reported as foreign and left untouched, so a source-repository link cannot
make installation write into that repository. Deployment must handle these links
explicitly. The command never installs skills on ordinary startup.
