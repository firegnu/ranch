# ranch

Shared runtime for the Saddle (terminal) and paddock (desktop) front ends: the parts
that must exist once per machine and user.

- `crates/corral`: the `corral` agent runtime.
- `crates/dispatch`, `crates/ranch`: the `ranch` command; `ranch dispatch route` routes tasks and
  `ranch dispatch install-skills` installs the corral-dispatch skill.
- `tools/package`: builds an immutable version directory with `bin/corral` and `bin/ranch`.

Front ends use only the installed commands and their JSON output. Design and rules:
`docs/DESIGN.md`, `AGENTS.md` (in Chinese).
