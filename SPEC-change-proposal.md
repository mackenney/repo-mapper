# SPEC Change Proposal — Anchor Mode (RWR Seeding)

**Status:** Proposed  
**Touches:** §7.1, §14, §17.4  
**Backward-compatible:** Yes — all new inputs default to empty/disabled.

---

## Problem

The current ranking algorithm is global Personalized PageRank: rank accumulates at
files that are *depended on by many*. This makes widely-imported utilities and core
abstractions rank highly, which is correct for general-purpose mapping.

It fails for **entrypoints**: functions that are called by nothing in the codebase
(Celery tasks, CLI handlers, HTTP handlers, plugin hooks). These nodes have outgoing
edges but zero incoming edges, so they accumulate near-zero PageRank score and are
dropped from the map — even when the user explicitly names them.

The existing `-i`/`--mention-ident` flag boosts edge weights *to* the defining file
(×10 multiplier), but there are no such edges for entrypoints. The path-component
personalization check fires only if the identifier string happens to match a directory
or filename.

The existing `-m`/`--mention-file` flag gives a weak personalization bump but no
guarantee of inclusion, and does not shape the rest of the map around the file.

### Concrete example

```
apps/luna/ai_workflow/tasks.py defines execute_single_tasks (@shared_task)
execute_single_tasks references: TaskService, BatchService, named_advisory_lock, ...
Nothing in the codebase calls execute_single_tasks (invoked by Celery scheduler)
```

Result today: `execute_single_tasks` and its file score ≈ 0, cut from the map.
Expected: map shows `execute_single_tasks` plus the services and utilities it depends on.

---

## Proposed Solution: Anchor Input

Add an **anchor** input — one or more files or identifiers that serve as the
*starting point* for the map. The algorithm becomes Random Walk with Restart (RWR)
seeded at the anchor, which is mathematically equivalent to Personalized PageRank
with the restart vector concentrated on the anchor.

RWR guarantees: even a node with no incoming edges accumulates rank, because the
random walker always has a nonzero probability of restarting there. Its dependencies
(the files it calls) also rank highly, because the walker frequently walks outward
from the anchor into the dependency cone.

### Key distinction from existing flags

| Flag | Excluded from map | Shapes map via PR | Forced inclusion |
|------|-------------------|-------------------|-----------------|
| `-c` (chat) | yes | yes (×50 edges + personalization) | no |
| `-m` (mention file) | no | weak (personalization only) | no |
| `-i` (mention ident) | no | weak (edge weight + path match) | no |
| `-a` (anchor, proposed) | **no** | **yes (heavy restart weight)** | **yes** |

An anchor is "a chat file that also shows up in the map."

---

## Spec Changes

### §7.1 — Personalization vector (extended)

Add the following step after the existing step 4:

> 5. If `anchor_rel_fnames` is non-empty, for each file `f` in `anchor_rel_fnames`:
>    `current_pers += personalize * anchor_weight_multiplier`.
>    This step is independent of steps 2–4; contributions are additive.
>    `anchor_rel_fnames` is derived from `anchor_fnames` and `anchor_idents` as
>    described in §7.1a.

Add new sub-section §7.1a — Anchor resolution:

> Before computing the personalization vector, the implementation MUST resolve
> anchor inputs to `anchor_rel_fnames` (a set of relative file paths) as follows:
>
> 1. For each path in `anchor_fnames`: convert to `rel_fname` relative to `root`.
>    Include even if the file is not in `other_fnames`.
> 2. For each identifier in `anchor_idents`: look up `tag_index.defines[ident]`
>    and include all files in the resulting set.
>    If an identifier is not found in `tag_index.defines`, it is silently ignored.
> 3. The union of all results from steps 1 and 2 is `anchor_rel_fnames`.
>
> Anchor resolution MUST occur after the TagIndex is built (§6.1) and before
> personalization is computed (§7.1).

Add new sub-section §7.1b — Anchor forced inclusion:

> After building `ranked_tags` (§7.5) and after prepending important files (§8.2),
> the implementation MUST prepend a `Bare` entry with `score = f64::MAX` for each
> file in `anchor_rel_fnames` that is not already present in `ranked_tags`.
> Files already present MUST NOT be duplicated.
>
> Rationale: RWR guarantees anchors get a positive PageRank score, but a very tight
> token budget may still cut them in binary search. Forced inclusion mirrors the
> behavior of §8.2 important files.

### §14 — Configuration Parameters (new rows)

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `anchor_fnames` | Vec\<PathBuf\> | [] | Files to use as RWR restart seeds; always included in map output |
| `anchor_idents` | HashSet\<String\> | {} | Identifiers whose defining files are used as RWR restart seeds |
| `anchor_weight_multiplier` | float | 10.0 | Personalization weight multiplier for anchor files relative to `personalize` (100/N) |

### §17.4 — CLI flags (new row)

| Flag | Short | Type | Default | Maps to |
|------|-------|------|---------|---------|
| `--anchor <NAME_OR_PATH>` | `-a` | string (repeatable) | (empty) | `anchor_fnames` or `anchor_idents` |

**Resolution rule:** if the value resolves to an existing file path (absolute, or
relative to CWD), it is added to `anchor_fnames`. Otherwise it is added to
`anchor_idents`. This allows `-a process_job` and `-a apps/luna/tasks.py` to both work.

`--anchor` may be specified multiple times. Each value is resolved independently.

---

## Behavioral invariants

1. With no `-a` flags, output is identical to current behavior.
2. Anchor files always appear in the map (as bare entries at minimum), regardless of
   token budget.
3. Anchors shape the PageRank personalization: their dependency cone ranks higher than
   it would in global mode.
4. Anchors do not suppress existing `-c`/`-m`/`-i` signals; all personalization
   contributions are additive.
5. An anchor identifier that does not exist in the tag index (unrecognized language,
   parse failure, misspelling) is silently ignored — no error, no warning. The anchor
   file list simply remains empty for that identifier.

---

## What does NOT change

- Graph construction (edge direction, weights, self-edges)
- PageRank algorithm (same power iteration, same damping, same convergence check)
- All existing `-c`, `-m`, `-i` semantics
- Tag extraction, rendering, budget binary search
- Cache key computation (anchor inputs are included in the cache key — see impl note)
