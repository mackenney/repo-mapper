# PROGRESS.md

## Status
In Progress

## Objective
Implement repo-mapper, a Rust library and CLI that produces a token-budget-respecting textual summary of a source code repository by extracting tags with tree-sitter, building a weighted directed graph, running PageRank, and rendering ranked definitions.

## Conflict Resolutions

### R1: Graph representation — petgraph::DiGraph
**Conflict:** P1 proposes custom HashMap adjacency list; P2 proposes petgraph::DiGraph.
**Resolution:** Use petgraph::DiGraph.
- Battle-tested crate with efficient out-edge iteration needed for PageRank
- Weighted directed multigraph support built-in
- Reduces custom code and bug surface
- More incremental (library vs custom implementation)

### R2: Query file loading — compile-time bundling
**Conflict:** P1 proposes `include_str!`; SPEC §4.1 specifies two runtime search paths.
**Resolution:** Use compile-time bundling with `include_str!`.
- SPEC §4.1 note says queries are "bundled by this implementation itself"
- Single-binary deployment is idiomatic for Rust crates
- Runtime file discovery adds fragility and complicates distribution
- The two search path names become compile-time selection of which bundle to include
- **Justified deviation**: modern Rust crates bundle assets at compile time

### R3: Cache serialization — bincode + serde (not FlatBuffers)
**Conflict:** P2 proposes FlatBuffers; P1/P3 don't specify.
**Resolution:** Use bincode + serde.
- FlatBuffers requires schema definition and codegen step
- bincode is simpler, well-supported, no codegen needed
- Performance difference marginal for tag vectors (typically hundreds, not millions)
- More incremental — add FlatBuffers later if profiling shows need

### R4: TreeContext — full SPEC §9.1/§9.2 implementation
**Conflict:** P2 claims "simplified" TreeContext since all options disabled.
**Resolution:** Implement fully per SPEC.
- Core algorithm is still non-trivial: sentinel-driven boundaries, lois tracking, caching
- Must parse with tree-sitter to find scope headers
- Cache keys: `(rel_fname, sorted(lois), mtime)` for render cache; `rel_fname` for context cache
- The "simplified" refers to output format, not implementation complexity

### R5: `max_map_tokens: Cell<usize>` — confirmed
**Conflict:** P3 proposes Cell; concern about thread safety.
**Resolution:** Cell is correct.
- RepoMap is not expected to be Sync (reference impl is single-threaded)
- Cell is !Sync, which is fine for this use case
- If Sync needed later, switch to AtomicUsize

## Wave Map

| Wave | Steps | Can Parallelize | Depends On |
|------|-------|-----------------|------------|
| 1a   | 01    | —               | —          |
| 1b   | 02    | —               | Wave 1a    |
| 2a   | 03    | —               | Wave 1b    |
| 2b   | 04    | —               | Wave 2a    |
| 3    | 05    | —               | Wave 2b    |
| 4    | 06, 07 | Yes            | Wave 3     |
| 5    | 08, 09 | Yes            | Wave 4     |
| 6    | 10    | —               | Wave 4     |
| 7a   | 11    | —               | Wave 6     |
| 7b   | 12    | —               | Wave 7a    |
| 8    | 13    | —               | Wave 7b    |
| 9    | 14    | —               | Waves 5, 6, 8 |
| 10   | 15    | —               | Wave 9     |
## Dependency Table

| Step | File(s) | Depends On | Depended By |
|------|---------|------------|-------------|
| 01 | Cargo.toml, src/lib.rs | — | All |
| 02 | src/tag.rs, src/path.rs | 01 | 05, 06 |
| 03 | src/lang.rs | 01 | 04, 05 |
| 04 | src/queries.rs, src/parser.rs, queries/*.scm | 03 | 05 |
| 05 | src/extract.rs, src/file.rs | 02, 03, 04 | 06, 08 |
| 06 | src/graph.rs, src/weight.rs | 02, 05 | 10 |
| 07 | src/important.rs | 02 | 10 |
| 08 | src/cache/tag_cache.rs | 05 | 14 |
| 09 | src/cache/map_cache.rs | — | 14 |
| 10 | src/rank/mod.rs, src/rank/personalization.rs, src/rank/pagerank.rs, src/rank/distribute.rs | 06 | 11 |
| 11 | src/render/tree_context.rs | 04, 10 | 12 |
| 12 | src/render/mod.rs, src/render/tree_cache.rs | 11 | 13 |
| 13 | src/tokens.rs, src/budget.rs | 12 | 14 |
| 14 | src/config.rs, src/repo_map.rs, src/edge_cases.rs | 07, 08, 09, 10, 12, 13 | 15 |
| 15 | src/bin/repo-mapper.rs | 14 | — |

## Orchestrator Protocol

1. Read this file to identify current wave
2. Dispatch all steps in current wave in parallel
3. After each step: dispatch reviewer agent (see step file for reviewer instructions)
4. Mark step complete only after reviewer passes
5. Advance to next wave only when all steps in current wave are complete
6. Blockers: stop and report to user with full context

## Subagent Contract

- Workers: Read step file fully before acting. Implement only what the step specifies.
- Workers: Commit changes with message "step-NN: <name>"
- Workers: Report back: "Step NN complete ✅ (commit <hash>)" or "Step NN FAILED: <reason>"
- Reviewers: Run acceptance criteria commands verbatim. Pass or fail with specifics.

## Steps

- [x] [step-01-cargo-skeleton](./step-01-cargo-skeleton.md) — Setup Cargo.toml dependencies and lib.rs module structure
- [x] [step-02-core-types](./step-02-core-types.md) — Tag, TagKind, and path utilities
- [x] [step-03-lang-registry](./step-03-lang-registry.md) — Language detection from filenames
- [x] [step-04-query-parser](./step-04-query-parser.md) — Query registry with bundled .scm files, parser registry
- [x] [step-05-tag-extraction](./step-05-tag-extraction.md) — Core tag extraction with identifier fallback
- [x] [step-06-graph-construction](./step-06-graph-construction.md) — TagIndex, edge weights, Graph with petgraph
- [x] [step-07-important-files](./step-07-important-files.md) — Important files list and filtering
- [x] [step-08-tag-cache](./step-08-tag-cache.md) — Persistent tag cache with rusqlite + bincode
- [x] [step-09-map-cache](./step-09-map-cache.md) — In-memory map cache with refresh policies
- [x] [step-10-pagerank](./step-10-pagerank.md) — Personalization, PageRank, rank distribution, ranked tags
- [x] [step-11-tree-context](./step-11-tree-context.md) — TreeContext for scope-aware rendering
- [x] [step-12-render-tree](./step-12-render-tree.md) — render_tree and to_tree with caching
- [x] [step-13-token-budget](./step-13-token-budget.md) — Token counting and binary search
- [x] [step-14-public-api](./step-14-public-api.md) — RepoMapConfig, RepoMap, get_repo_map, edge cases
- [ ] [step-15-cli](./step-15-cli.md) — CLI binary with clap, file enumeration, exit codes
