# ranch

Shared runtime for the Saddle (terminal) and paddock (desktop) front ends: the parts
that must exist once per machine and user.

- `crates/corral`: the `corral` agent runtime.

Front ends use only the installed commands and their JSON output. Design and rules:
`docs/DESIGN.md`, `AGENTS.md` (in Chinese).
