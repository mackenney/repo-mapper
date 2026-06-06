# Agent Guidelines — repo-mapper

## Project

`repo-mapper` is a Rust library that produces a compact, token-budget-respecting textual
summary (the "repo map") of a source code repository. All implementation is SPEC-driven:
`SPEC.md` is the authoritative behavioral contract; no code is written for a component
before its spec section is settled.

## Workspace layout

```
src/              Library source and binary entry point
  lib.rs          Public library root
  bin/            CLI binary (repo-mapper)
plans/            Active development plans — committed, deleted when complete
reference/        Source reference implementations (git-ignored; do not commit)
  aider/          aider repomap.py — primary behavioral ground truth
  RepoMapper/     Secondary reference
artifacts/        Transient working files — git-ignored, never committed
SPEC.md           Behavioral contract for the entire library (reverse-engineered)
MASTER_PROGRESS.md  Single source of truth for project-wide work status
```

## Reference Implementation

The primary ground truth is `reference/aider/aider/repomap.py`. When a behavioral question
arises — what does the algorithm do? how are edge weights computed? — read that file, not
guesswork. Secondary reference: `reference/RepoMapper/`.

There is no runnable reference binary. Ground truth comes from reading the Python source
and from `SPEC.md`, which was reverse-engineered from it.

## SPEC-driven development

- Every implementation must be driven by a spec.
- The spec lives at `SPEC.md` (repo root).
- Spec documents use MUST / SHOULD / MAY language (RFC 2119).
- The pipeline is: **investigate → spec → plan → orchestrate**.
- No code is written before the relevant spec section is settled.

## Master Progress

`MASTER_PROGRESS.md` is the single source of truth for project-wide work status.
Every human and agent working on repo-mapper reads it first to understand current state.

**What it contains:**
- **In Progress** — active plans with a link to their plan directory
- **Queued** — not-started plans with a link to their plan directory
- **Completed** — one-liner per finished feature/plan, with commit hash
- **Known Gaps** — identified issues with no active plan owner

**Rules:**
- When a plan completes and is deleted → add a one-liner to Completed with the merge commit
- When a new plan is created → add it to Queued
- When work starts on a plan → move it from Queued to In Progress
- Keep entries as one-liners; all detail lives in plan files and git history
- Never let MASTER_PROGRESS.md drift: update it in the same commit as the plan change

## Plans

- `plans/` is committed; plans are **deleted** once their execution is complete.
- Plans are created by the planner skill; file names follow the planner's
  conventions (e.g. `plans/<feature>/PROGRESS.md`, `plans/<feature>/step-NN-*.md`).
- Code comments and docs must **not** reference plan files.

## Artifacts

- `artifacts/` is a general-purpose directory for transient working files.
- It is git-ignored and never committed.
- Use it freely for investigation scans, fact-check reports, progress trackers,
  brainstorm notes, and any other session-scoped output.

## Testing

Rules for agents:

- All tests are inline unit tests (`#[cfg(test)]` modules in `src/`). They are
  implementation details — change them freely during refactors.

Test commands:
```sh
cargo nextest run       # all tests (preferred)
cargo test              # fallback if nextest unavailable
cargo install --path .  # release build + install to ~/.cargo/bin
```

## Conventions

- Commit messages: imperative mood, 72 chars, no period
- Branch names: `ignacio@repo-mapper/<kebab-description>`
- Format with `cargo fmt`, lint with `cargo clippy` before committing
- No `#[allow(clippy::*)]` without a comment explaining why
- No section-separator comments (`// ---`, `// ===`, etc.)
- Comments explain WHY, not WHAT

## Tooling

- **Shell:** bash; `jq`, `rg`, `fdfind` available
- **Build:** `cargo build`, `cargo build --release`, `cargo clippy`, `cargo fmt`
- **Tests:** `cargo nextest run` (preferred)
