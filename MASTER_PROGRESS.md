# Master Progress

Read this file to understand the current state of repo-mapper. One-liner per item; all detail is in plan files and git history.

---

## In Progress

| Item | Plan | Notes |
|---|---|---|
| _(none)_ | | |

---

## Completed

### Foundation

| What | Ref |
|---|---|
| SPEC.md reverse-engineered from reference/aider/aider/repomap.py | pre-history |
| AGENTS.md, MASTER_PROGRESS.md, project governance established | _(this commit)_ |

---

## Queued (Not Started)

| Plan | What |
|---|---|
| _(none)_ | |

---

## Known Gaps

Gaps with an active plan are marked. Unplanned gaps are open for pickup.

| ID | Issue | Plan | Notes |
|---|---|---|---|
| OQ-1 | I/O and diagnostic abstraction unresolved | _(no plan)_ | Must decide between `dyn RepoMapIO` trait, callback struct, or `tracing` integration before implementing §3.1 file reading and §13.5 warning emission |
| OQ-2 | Pygments reference fallback (§3.2) has no Rust equivalent | _(no plan)_ | Reference uses Python Pygments for token-type classification; Rust needs a replacement strategy or a v1 deferral decision |
| BUG-REF-1 | Reference captures-processing bug (§3.5) | _(no plan)_ | repomap.py loop indentation bug — intentionally NOT replicated; Rust MUST emit one Tag per captured node with no data loss and no duplicates |
| BUG-REF-2 | `warned_files` class-level shared state (§13.5) | _(no plan)_ | Reference uses class-level set, sharing deduplication across all instances; Rust MUST use per-instance state instead |
