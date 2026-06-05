# Master Progress

Single source of truth for project-wide work status.

## In Progress

_(none)_

## Completed

- SPEC.md reverse-engineered from `reference/aider/aider/repomap.py`
- Project governance: AGENTS.md, MASTER_PROGRESS.md (`e6d1201`)
- Full implementation: tag extraction, graph construction, PageRank, caching, rendering, token budget, public API, CLI (`46075db`)
- Public release prep: docs, missing_docs lint, CLI help, Cargo metadata, LICENSE (`HEAD`)

## Queued

_(none)_

## Known Gaps

**BUG-REF-1** — Reference captures-processing bug (§3.5)
repomap.py has a loop indentation bug that causes duplicate/dropped tags. Intentionally not replicated — the Rust implementation emits one `Tag` per captured node with no data loss and no duplicates.

**BUG-REF-2** — `warned_files` class-level shared state (§13.5)
The reference uses a class-level set, sharing deduplication across all instances. The Rust implementation uses per-instance state instead, which is the correct design.
