# SPEC.md — repo-mapper Behavioral Specification

**Status:** Draft — Reverse-engineered from reference implementations  
**Sources:** `reference/aider/aider/repomap.py` (primary), `reference/RepoMapper/` (secondary)  
**RFC 2119 keywords apply throughout.**

---

## 1. Overview

repo-mapper is a library that produces a compact, token-budget-respecting textual summary
(the "repo map") of a source code repository. It identifies the most structurally relevant
files and definitions given a set of "chat" files (the ones a user is actively editing) and
a set of "other" files (the rest of the repository).

The pipeline is:

1. **Tag extraction** — parse each source file with tree-sitter to extract definition and reference tags.
2. **Graph construction** — build a weighted directed multi-graph of file-to-file relationships via shared identifiers.
3. **Ranking** — run PageRank on that graph, personalized toward chat-relevant files.
4. **Rendering** — convert the ranked tags into a human-readable text output.
5. **Budget enforcement** — binary-search for the largest subset of ranked tags that fits within the token limit.

---

## 2. Data Types

### 2.1 Tag

A **Tag** MUST carry exactly the following fields:

| Field       | Type   | Description                                                       |
|-------------|--------|-------------------------------------------------------------------|
| `rel_fname` | string | Path of the file relative to the repository root                  |
| `fname`     | string | Absolute path to the file                                         |
| `line`      | int    | Source line where the identifier appears (see §3.4 for indexing)  |
| `name`      | string | The identifier text                                               |
| `kind`      | string | Either `"def"` (definition) or `"ref"` (reference)               |

### 2.2 Path Representations

The spec uses two tiers of file paths consistently throughout:

- **Absolute path** (`fname`): the OS-native absolute path to the file. Used as cache keys (§5.3) and for reading file content.
- **Relative path** (`rel_fname`): the path relative to the repository root, computed as `relpath(fname, root)`. Used as graph node identifiers, map output labels, and ranking keys.

The `rel_fname` for any file MUST be computed as the relative path from `root`. On platforms where `relpath` raises an error (e.g., Windows cross-drive paths), the implementation MUST fall back to the absolute path unchanged.

`chat_rel_fnames` is the set of relative paths derived from `chat_fnames`.
`other_rel_fnames` is the set of relative paths derived from `other_fnames`.

Before processing, the implementation MUST compute the combined file set as `set(chat_fnames) ∪ set(other_fnames)` (deduplicated). Files present in both sets are processed once; any file in `chat_fnames` is treated as a chat file regardless of whether it also appears in `other_fnames`. The combined set MUST be sorted lexicographically before tag extraction begins, for deterministic processing order.

### 2.3 FileEntry

During graph construction, files are represented as their `rel_fname` string (nodes in the graph).

---

## 3. Tag Extraction

### 3.1 Per-file extraction

For each source file the implementation MUST:

1. Determine the file's language from its filename (using a language-detection lookup table equivalent to `grep-ast`'s `filename_to_lang`).
2. If the language is unrecognized, produce no tags and skip the file silently.
3. Load the corresponding tree-sitter parser for that language. If the parser is unavailable, skip the file.
4. Load the `.scm` query file for that language (see §4). If no `.scm` exists for the language, produce no tags and skip the file.
5. Read the file's text content as UTF-8. If the file is empty or unreadable, produce no tags.
6. Parse the source text. Execute the query using a `QueryCursor`, passing the source bytes
   as the text provider (`QueryCursor::captures(query, root_node, source_bytes)`). Iterate
   captures in source order; each iteration yields a `(QueryMatch, capture_index)` pair.
   Resolve the capture name as `query.capture_names()[capture.index]`.
7. For each capture whose name begins with `"name.definition."`, emit a `Tag` with `kind = "def"`.
8. For each capture whose name begins with `"name.reference."`, emit a `Tag` with `kind = "ref"`.
9. All other capture names MUST be ignored (not emitted as tags).
10. The tag's `name` field MUST be the UTF-8 decoded text of the captured tree-sitter node.

### 3.2 Pygments reference fallback

If tree-sitter extraction for a file produces **at least one definition tag and zero reference tags**, the implementation MUST apply a Pygments-based fallback to backfill reference tags:

1. Use Pygments to identify a lexer for the file (`guess_lexer_for_filename`). If this
   call raises any exception, the fallback MUST be silently skipped — definition tags
   already produced for this file are retained and no reference tags are emitted.
   Otherwise, lex the file content with that lexer.
2. For every token whose Pygments type is in the `Token.Name` family, emit a `Tag` with:
   - `kind = "ref"`
   - `name` = the token text
   - `line = -1` (sentinel: no meaningful line number)
3. The backfilled reference tags are appended after the definition tags for this file.

If tree-sitter produces **no definitions and no references**, the Pygments fallback MUST NOT be applied.

If tree-sitter produces **references** (with or without definitions), the Pygments fallback MUST NOT be applied.

### 3.3 Non-file inputs

If a path does not resolve to a regular file (e.g., directory, symlink to non-file, or missing), the implementation MUST skip it, emit a warning to the caller once per unique path, and produce no tags for it.

### 3.4 Line number indexing

Line numbers MUST be 1-indexed. The `line` field of a `Tag` MUST store the 1-indexed line
number of the captured node (i.e., `node.start_point[0] + 1` in tree-sitter terms).

The `line` field for Pygments-backfilled reference tags MUST be `-1` (sentinel: no meaningful line number).

### 3.5 Known bug in reference implementation: captures-processing

[KNOWN BUG IN REFERENCE: `repomap.py` lines 304–315 have a loop indentation bug. Two statements are placed outside the inner `for node in nodes` loop: `captures_by_tag[tag].append(node)` and `matches.append((node, tag))`. This produces two failure modes: (1) Non-TSL_PACK path — `matches` only collects the last `(node, tag)` per capture-type name; all earlier nodes for the same capture type are silently discarded (e.g., a file with 10 class definitions produces exactly 1 definition tag). (2) TSL_PACK path — `captures_by_tag[tag]` receives an extra duplicate of the last node outside the inner loop, generating one duplicate tag per capture type. Intended behavior: emit exactly one `Tag` per captured node — no data loss and no duplicates. The Rust implementation MUST NOT replicate either bug.]
---

## 4. Language and Query Support

### 4.1 Query file resolution

For a given language identifier, the implementation MUST locate the corresponding `.scm`
query file. Two search paths are supported (in priority order):

1. `queries/tree-sitter-language-pack/{lang}-tags.scm`
2. `queries/tree-sitter-languages/{lang}-tags.scm`

If neither path resolves to an existing file, the language has no query support and MUST be skipped.

Note: the `.scm` tag query files are bundled by this implementation itself. The Rust tree-sitter
parser crate (`tree-sitter-language-pack` or equivalent) provides parsers only; it does NOT
supply `name.definition.*` / `name.reference.*` tag queries.

### 4.2 Supported languages (query coverage)

The following language identifiers have known query coverage based on the reference files.
This list is informational; the actual runtime set depends on which `.scm` files are bundled:

`arduino`, `c`, `chatito`, `commonlisp`, `cpp`, `csharp`, `d`, `dart`, `elisp`,
`elixir`, `elm`, `gleam`, `go`, `hcl`, `java`, `javascript`, `kotlin`, `lua`, `ocaml`,
`ocaml_interface`, `php`, `pony`, `properties`, `python`, `ql`, `r`, `racket`, `ruby`,
`rust`, `scala`, `solidity`, `swift`, `typescript`, `udev`

### 4.3 What each supported language defines

The following captures are present in the reference query files. The Rust reimplementation
MUST produce equivalent captures.

**Python:**
- `name.definition.class` — class names
- `name.definition.function` — function names
- `name.definition.constant` — module-level identifier assignments (TSL_PACK path only; absent from tree-sitter-languages fallback)
- `name.reference.call` — call targets (direct identifiers and attribute access)

**Rust:**
- `name.definition.class` — struct, enum, union, and type alias names
- `name.definition.method` — methods inside `impl` blocks
- `name.definition.function` — free functions
- `name.definition.interface` — trait names
- `name.definition.module` — `mod` items
- `name.definition.macro` — macro definitions
- `name.reference.call` — direct call and field method call targets; macro invocations
- `name.reference.implementation` — trait and type names in `impl` items

**Go** (the following captures are present in both backends):
- `name.definition.function` — top-level function declarations
- `name.definition.method` — method declarations
- `name.definition.type` — type specifications
- `name.reference.call` — call targets
- `name.reference.type` — type identifier references

The following Go captures are present only in the TSL_PACK backend (`CACHE_VERSION=4`):
- `name.definition.module` — package clause identifier
- `name.definition.interface` — interface type specifications
- `name.definition.class` — struct type specifications
- `name.reference.module` — entire `import_spec` node text (not just the package name)
- `name.definition.variable` — var declarations
- `name.definition.constant` — const declarations

**JavaScript:**
- `name.definition.method` — method definitions (excluding `constructor`)
- `name.definition.class` — class and class declaration names
- `name.definition.function` — function declarations, generator functions, arrow/function variables and assignments, and object-literal function-valued properties
- `name.reference.call` — call targets (excluding `require`)
- `name.reference.class` — `new` expression constructors

**TypeScript** (standalone SCM; does NOT inherit from or extend the JavaScript SCM):
- `name.definition.function` — function signatures and function declarations
- `name.definition.method` — method signatures, abstract method signatures, and method definitions
- `name.definition.class` — abstract class declarations, class declarations, and interface declarations (used as class)
- `name.definition.interface` — interface declarations
- `name.definition.module` — module declarations
- `name.definition.type` — type alias declarations
- `name.definition.enum` — enum declarations
- `name.reference.type` — type annotation identifiers
- `name.reference.class` — `new` expression constructors

Note: TypeScript's SCM has **no `name.reference.call` capture**. Tree-sitter query files have no inheritance mechanism; the TypeScript SCM is a completely standalone file. TypeScript files that contain no type annotations and no `new` expressions will produce zero reference tags, triggering the Pygments fallback (§3.2).

**Java:**
- `name.definition.class` — class declarations
- `name.definition.method` — method declarations
- `name.definition.interface` — interface declarations
- `name.reference.call` — method invocation names
- `name.reference.implementation` — type identifiers in `implements`/`extends` lists
- `name.reference.class` — `new` expressions and superclass references

---

## 5. Tag Cache

### 5.1 Purpose

Tag extraction is computationally expensive. The implementation MUST cache parsed tags on
disk and reuse them when the file has not changed.

### 5.2 Cache location

The persistent cache MUST be stored at `{root}/.aider.tags.cache.v{CACHE_VERSION}` where
`CACHE_VERSION` is an integer that MUST be incremented whenever the cache format changes or
query files are updated in a way that invalidates prior results.

[KNOWN REFERENCE BEHAVIOR: The effective `CACHE_VERSION` is runtime-determined: `3` when using the tree-sitter-languages backend and `4` when using the tree-sitter-language-pack backend (`USING_TSL_PACK = True`). The Rust implementation MUST use distinct cache directory names or version numbers for each backend variant to prevent cross-contamination of cached tag data.]

### 5.3 Cache key and value

- **Key:** the absolute path string of the file.
- **Value:** a record containing:
  - `mtime`: the file's modification timestamp (floating-point seconds) at the time of
    last extraction.
  - `data`: the list of `Tag` objects produced by extraction.

### 5.4 Cache validity

On a cache lookup, the implementation MUST check that the stored `mtime` equals the current
file modification time. If they differ, the cache entry MUST be treated as a miss and the
file re-parsed.

### 5.5 Cache error recovery

If the persistent cache becomes corrupt or unusable (e.g., SQLite errors), the
implementation MUST:

1. Attempt to delete and recreate the cache directory.
2. If recreation also fails, fall back to an in-memory cache (an ordinary hash map) for
   the duration of the current process. The in-memory cache MUST follow the same key/value
   semantics as the persistent cache.
3. Emit a warning to the caller when falling back.

### 5.6 In-memory map cache

The implementation MUST maintain a separate in-memory cache for the full computed repo map
string. The cache is process-local and is not persisted to disk. The cache key is
mode-dependent (see §11 for the full specification): in `"auto"` mode the key is a 5-tuple
including `mentioned_fnames` and `mentioned_idents`; in `"files"` mode the key is a 3-tuple
excluding those fields. `"manual"` mode does NOT use the keyed map cache (see §11). Each
non-None key component is a `tuple(sorted(...))` of values. Empty collections are
represented as `None` in the key rather than empty tuples.

---

## 6. Graph Construction

### 6.1 Data structures

The following intermediate structures are built from all tags across all files:

- `defines`: `identifier_name → set<rel_fname>` — maps each identifier to all files that
  define it.
- `references`: `identifier_name → list<rel_fname>` — maps each identifier to all files
  that reference it (with multiplicity: the same file may appear multiple times if it
  references the same name in multiple places).
- `definitions`: `(rel_fname, identifier_name) → set<Tag>` — maps each (file, name) pair
  to the set of definition tags for that name in that file.

### 6.2 No-reference fallback

If the global `references` structure is empty after processing all files, the implementation
MUST substitute `references = {name: list(fnames) for name, fnames in defines.items()}`,
treating every defined identifier as also self-referenced by every file that defines it.

### 6.3 Graph type

The graph MUST be a weighted directed multi-graph (multiple edges with distinct `ident`
attributes are allowed between the same pair of nodes).

### 6.4 Edge weights

For each identifier `ident` that appears in both `defines` and `references`:

1. Compute a base multiplier `mul`:

   | Condition                                                             | Effect      |
   |-----------------------------------------------------------------------|-------------|
   | `ident` is in `mentioned_idents`                                      | `mul *= 10` |
   | `ident` length ≥ 8 AND (`_` present with ≥1 alpha char, OR `-` present with ≥1 alpha char, OR both upper and lower case present) | `mul *= 10` |
   | `ident` starts with `"_"`                                             | `mul *= 0.1`|
   | More than 5 distinct files define `ident`                             | `mul *= 0.1`|

   Each applicable condition is applied independently and multiplicatively.

2. Compute `num_refs` = the number of times each unique `referencer` appears in
   `references[ident]` (i.e., count occurrences; `references[ident]` is a list with
   multiplicity). For each unique `(referencer, definer)` pair:

   - Compute `use_mul = mul`.
   - If `referencer` is one of the chat files (in `chat_rel_fnames`): `use_mul *= 50`.
   - Edge weight = `use_mul × sqrt(num_refs)`.
   - Add a directed edge `referencer → definer` with this weight and `ident` metadata.

3. Note: self-edges (referencer == definer) are allowed and not filtered out.

### 6.5 Self-edges for unreferenced definitions

For each identifier in `defines` that does NOT appear in `references` (after the no-reference
fallback in §6.2 has been applied — i.e., this applies when the fallback was NOT triggered
and the identifier genuinely has no references), the implementation MUST add a self-edge
for every file that defines it:

```
definer → definer, weight = self_edge_weight (see §14), ident = identifier
```

This ensures that files with only definitions still participate in PageRank.

[SOURCE AMBIGUITY: The self-edge logic runs against `defines.keys()` filtered by `ident not in references`. After the no-reference fallback (§6.2), all define-only identifiers would already be in references, so this branch only activates when there are *some* references globally but specific identifiers have no references at all.]

---

## 7. PageRank Ranking

### 7.1 Personalization vector

Before running PageRank, the implementation MUST compute a per-file personalization score.
Let `N` be the total number of unique files across `chat_fnames` and `other_fnames`.
Let `personalize = 100.0 / N`.

For each file (identified by `rel_fname`), compute `current_pers` as follows:

1. Start at 0.
2. If the file's absolute path is in `chat_fnames`: `current_pers += personalize`.
3. If `rel_fname` is in `mentioned_fnames`: `current_pers = max(current_pers, personalize)`.
   (This avoids double-counting files that are both in chat and explicitly mentioned.)
4. If any component of `rel_fname`'s path (directory parts, basename with extension,
   basename without extension) appears in `mentioned_idents`:
   `current_pers += personalize`.

Only files with `current_pers > 0` are included in the personalization dict.

The `dangling` nodes dict MUST be set equal to the personalization dict when personalization
is non-empty.

### 7.2 PageRank algorithm

The implementation MUST run PageRank on the constructed graph with:

- `weight = "weight"` (use the edge weight attribute)
- `personalization` and `dangling` dicts as computed in §7.1 (or absent if empty)
- Damping factor: **0.85** (configurable via `pagerank_damping`; see §14)
- Convergence tolerance: **1.0e-6** (configurable via `pagerank_tol`; see §14)
- Maximum iterations: **100** (configurable via `pagerank_max_iter`; see §14)

### 7.3 ZeroDivisionError handling

If PageRank raises a division-by-zero error:

1. Retry PageRank without personalization.
2. If it raises again, return an empty result (no ranked tags).

### 7.4 Distributing rank to definitions

After PageRank assigns a scalar rank to each graph node (file), the implementation MUST
distribute each node's rank across its outgoing edges proportionally by weight:

```
For each node `src` in the graph:
    src_rank = pagerank[src]
    total_weight = sum of weights of all outgoing edges from src
    For each outgoing edge (src → dst, weight w, ident i):
        ranked_definitions[(dst, i)] += src_rank * w / total_weight
```

This produces a per-(file, identifier) rank score.

### 7.5 Building the ranked tag list

1. Sort `ranked_definitions` items descending by score (tie-break: reverse-lexicographic
   (descending) on the `(dst, ident)` key).
2. For each `(fname, ident)` entry (excluding files in `chat_rel_fnames`), append all
   definition `Tag` objects from `definitions[(fname, ident)]` to `ranked_tags`.
3. Track which files have already been included via their definition tags.
4. Iterate over all graph nodes sorted by PageRank score descending:
   - Remove the node from `rel_other_fnames_without_tags` (tracking which other-files
     are represented in the graph at all).
   - If the file is not already included in `ranked_tags`, append a bare file entry
     `(rel_fname,)` (a 1-element tuple, no associated tags).
5. For any file in `other_fnames` that was never added to the graph at all
   (remains in `rel_other_fnames_without_tags`), append a bare file entry `(rel_fname,)`
   at the end of `ranked_tags`.
6. If `exclude_unranked` is `true` (see §14), remove from `ranked_tags` any bare file entry
   whose computed PageRank score is ≤ **0.0001**. Files appended in step 5 (never added to
   the graph) have an implicit PageRank score of 0.0 and MUST be treated as ≤ 0.0001 for
   this filter. Definition-tag entries (files with at least one tag in the output) MUST NOT
   be removed by this filter.

---

## 8. Important Files

### 8.1 Definition

A set of "important" files is pre-defined by filename (and optionally directory pattern).
These represent conventional repository files such as `README.md`, `Cargo.toml`,
`package.json`, `Dockerfile`, `Makefile`, and similar.

The exact list is implementation-defined but SHOULD include all files from
`reference/aider/aider/special.py`'s `ROOT_IMPORTANT_FILES`.

### 8.2 Ordering guarantee

Before the binary search (§10.2), the implementation MUST prepend important-file entries to
the ranked tag list:

1. Compute `important = filter_important_files(other_rel_fnames)` — the subset of
   `other_fnames` whose relative paths match the important-file list.
2. Remove from `important` any file already present in `ranked_tags` (to avoid duplication).
3. Convert each remaining entry to a bare file entry `(rel_fname,)`.
4. Prepend these entries to the front of `ranked_tags`.

The result is: `[important_files...] + [ranked_definition_tags...] + [unranked_files...]`

**Rationale:** Important files always appear at the top of the map when budget allows, even
if PageRank would score them low.

---

## 9. Map Rendering

### 9.1 to_tree

If the input tag slice is empty, `to_tree` MUST return an empty string `""` immediately
(not `"\n"`).

Otherwise, given a slice of the ranked tag list and the set of `chat_rel_fnames`, the implementation
MUST produce a UTF-8 text output as follows:

1. Sort the input tag slice lexicographically. (Tags sort by `(rel_fname, fname, line, name, kind)`;
   bare 1-element tuples sort by their `rel_fname` string.)
   The combined file set deduplication in §2.2 guarantees a file never appears as both a full
   Tag and a bare tuple in the same input; no tie-break between the two types is needed.
2. Append a sentinel entry `(None,)` at the end to flush the last file.
3. Iterate through the sorted-plus-sentinel list. Maintain `cur_fname`, `cur_abs_fname`,
   and `lois` (list of integer line numbers, or `None`).
4. If `tag[0]` is in `chat_rel_fnames`: skip this tag entirely (do not execute the
   remaining steps for this iteration).
5. Whenever `tag[0] != cur_fname` (file boundary):
   a. If `lois is not None`: emit `"\n" + cur_fname + ":\n"` followed by the rendered tree
      for `(cur_abs_fname, cur_fname, lois)` (see §9.2). Reset `lois = None`.
   b. Else if `cur_fname is not None` (and it is not `None`): emit `"\n" + cur_fname + "\n"`.
   c. If the incoming tag is a full `Tag` object (not a bare tuple): set `lois = []` and
      record `cur_abs_fname = tag.fname`.
   d. Set `cur_fname = tag[0]`.
6. If `lois is not None` (after the file-boundary check):
   append `tag.line` to `lois`.
7. After building the full output string, truncate every line to at most `max_line_length`
   characters (see §14).
8. Append a final newline `"\n"` to the output.

### 9.2 render_tree

Given `(abs_fname, rel_fname, lois)`:

1. Read the file's text content. If unreadable, return an empty string.
2. Ensure the content ends with a newline (append `"\n"` if not).
3. Create (or retrieve from cache) a `TreeContext` for the file with parameters:
   - `color = false`
   - `line_number = false`
   - `child_context = false`
   - `last_line = false`
   - `margin = 0`
   - `mark_lois = false`
   - `loi_pad = 0`
   - `show_top_of_file_parent_scope = false`
4. Reset the context's `lines_of_interest` to an empty set, call `add_lines_of_interest(lois)`,
   call `add_context()`, and return the result of `format()`.

The render result MUST be cached by the key `(rel_fname, tuple(sorted(lois)), mtime)`. The
`TreeContext` object itself MUST be stored in a dict keyed by `rel_fname` alone; on lookup
the stored `mtime` MUST be compared against the current file `mtime` and the entry replaced
on mismatch (old entries are overwritten, not accumulated).

The `tree_cache` (render result cache) MUST be cleared at the start of each uncached map
computation (§10.2). The `tree_context_cache` persists across computations for the lifetime
of the `RepoMap` instance.

---

## 10. Token Budget and Binary Search

### 10.1 Token counting

The implementation MUST count tokens using a tiktoken-compatible BPE encoder
(domain constraint: tiktoken is the standard tokenizer for OpenAI-family models; the
`tiktoken-rs` crate wraps the same underlying Rust BPE implementation that the Python
`tiktoken` package uses, producing byte-identical token counts). The specific encoding
SHOULD default to `cl100k_base` unless the caller overrides it at construction time.
For texts shorter than **200 characters**, the encoder is called directly on the full text.
For longer texts, the implementation MUST use sampling:

1. Split the text into lines, preserving line endings (`splitlines(keepends=True)`).
2. Compute `step = num_lines // 100 or 1`. Sample every `step`-th line.
3. Count tokens in the sampled text.
4. Estimate: `est_tokens = (sample_tokens / len(sample_text)) * len_text`.

### 10.2 Binary search

Given `max_map_tokens` and the full `ranked_tags` list of length `N`:

1. Initialize:
   - `lower_bound = 0`, `upper_bound = N`
   - `best_tree = None`, `best_tree_tokens = 0`
   - `middle = min(max_map_tokens // 25, N)` (floor division)
2. Loop while `lower_bound <= upper_bound`:
   a. Render `to_tree(ranked_tags[:middle], chat_rel_fnames)`.
   b. Count tokens: `num_tokens`.
   c. Compute `pct_err = |num_tokens - max_map_tokens| / max_map_tokens`.
   d. If `(num_tokens <= max_map_tokens AND num_tokens > best_tree_tokens) OR pct_err < 0.15`:
      - Update `best_tree = current_tree`, `best_tree_tokens = num_tokens`.
      - If `pct_err < 0.15`: **break early** (result is close enough).
   e. Standard binary search step:
      - If `num_tokens < max_map_tokens`: `lower_bound = middle + 1`
      - Else: `upper_bound = middle - 1`
      - `middle = (lower_bound + upper_bound) // 2`
3. Return `best_tree` (may be `None` if no valid tree was found).

The **0.15 (15%) tolerance** means the implementation MAY return a tree that slightly
exceeds `max_map_tokens` if the overrun is within 15%.

### 10.3 No-chat-files mode

When `chat_fnames` is empty AND `max_context_window` is set, compute:

```
effective_max = min(max_map_tokens * map_mul_no_files, max_context_window - 4096)
```

If `effective_max > 0`, use it as `max_map_tokens` for the current call only. (If
`max_context_window < 4096`, `effective_max` is negative and the substitution does NOT occur.)

The default `map_mul_no_files` MUST be **8**.

---

## 11. Map Cache (Refresh Policy)

The implementation MUST support four named refresh modes:

| Mode       | Behavior                                                                                     |
|------------|----------------------------------------------------------------------------------------------|
| `"manual"` | If a map has been computed before in this session, return it as-is. Never recompute.         |
| `"always"` | Never use the in-memory map cache. Always recompute.                                         |
| `"files"`  | Always use the in-memory map cache if the key matches.                                       |
| `"auto"`   | Use the cache only if the previous computation took more than **1.0 second**. Otherwise recompute. |

`force_refresh = true` bypasses all cache checks and forces recomputation.

Regardless of mode or `force_refresh`, the implementation MUST write the computed result to
both the keyed map cache and `last_map` after every computation. `force_refresh` bypasses
cache **reads** only; the write-back always occurs.

`"manual"` mode does NOT consult the keyed map cache. It stores the most recent computed
result in a separate, keyless per-instance field (`last_map`) and returns it on all
subsequent calls regardless of input parameters.

In `"auto"` mode the cache key MUST be the 5-tuple
`(tuple(sorted(chat_fnames)), tuple(sorted(other_fnames)), max_map_tokens, tuple(sorted(mentioned_fnames)), tuple(sorted(mentioned_idents)))`.
In `"files"` mode the key MUST be the 3-tuple
`(tuple(sorted(chat_fnames)), tuple(sorted(other_fnames)), max_map_tokens)`, excluding
`mentioned_fnames` and `mentioned_idents`. Empty collections are represented as `None`
rather than empty tuples. The reference source is unambiguous on this point
(`if self.refresh == "auto"` guard).

---

## 12. Output Format

### 12.1 Final string assembly

The return value of `get_repo_map` MUST be:

```
{repo_content_prefix} + {files_listing}
```

Where:
- `repo_content_prefix` is a caller-supplied prefix string (MAY contain a `{other}` 
  placeholder that is substituted with `"other "` when chat files are present or `""` when
  they are not).
- `files_listing` is the output of the binary-search-selected `to_tree` call.

If `files_listing` is empty or `None`, `get_repo_map` MUST return `None` / empty.

### 12.2 File entries with tags

```
\n{rel_fname}:\n{rendered_tree_content}
```

### 12.3 File entries without tags

```
\n{rel_fname}\n
```

### 12.4 Line truncation

Every line in the output MUST be truncated to at most `max_line_length` characters
(see §14; default **100**) before the final newline is appended.

---

## 13. Edge Cases

### 13.1 Empty or zero token budget

If `max_map_tokens <= 0`, `get_repo_map` MUST return `None` immediately.

### 13.2 No other files

If `other_fnames` is empty, `get_repo_map` MUST return `None` immediately.

### 13.3 No tags anywhere

If tag extraction yields no tags for any file, the graph is empty. The implementation MUST
still produce a listing of file names (bare entries) using the file-only format (§12.3) up
to the token budget.

### 13.4 RecursionError

If the ranking algorithm raises a `RecursionError` (can happen for extremely large repos),
the implementation MUST:
1. Emit an error to the caller.
2. Set `max_map_tokens = 0` (permanently disabling the map for this instance).
3. Return `None`.

### 13.5 Missing files

Files in `chat_fnames` or `other_fnames` that do not exist on disk at ranking time MUST be
skipped. The implementation MUST emit a warning the first time a given path is skipped, and
MUST NOT warn again for the same path within the same process.

[KNOWN BUG IN REFERENCE: `warned_files` is defined as a class-level attribute (`class RepoMap: warned_files = set()`). Mutating it via `self.warned_files.add(fname)` modifies the shared class-level set, so warning deduplication spans all `RepoMap` instances in the process rather than being per-instance. Intended behavior: per-instance deduplication (MUST NOT warn again for the same path within the same instance). The Rust implementation SHOULD use per-instance state.]

If a file passes the existence check at ranking time but disappears before its mtime can be
read (TOCTOU), `get_tags` MUST return an empty tag list for that file and MAY emit a warning.
This is distinct from the upfront existence check — the file is silently skipped at extraction
time without updating `warned_files`.

### 13.6 Binary files / unreadable files

Files whose content cannot be decoded as UTF-8 MUST produce no tags. The implementation
SHOULD attempt to read with error substitution rather than raising (i.e., replace
undecodable bytes).

### 13.7 Chat files excluded from output

Files that appear in `chat_fnames` MUST NOT appear in the repo map output. They are excluded
at both the ranking step (§7.5 step 2) and the rendering step (§9.1 step 4).

### 13.8 Known reference dead code

The following constructs in the reference implementation are dead code and MUST NOT be
replicated:

- `self.cache_threshold = 0.95` — set in `__init__` but never read; vestigial field.
- `if tags is None: continue` — `list()` never returns `None`; the branch is unreachable.

The Rust implementation SHOULD omit both.

---

## 14. Configuration Parameters

| Parameter             | Type         | Default   | Description                                                                   |
|----------------------|--------------|-----------|-------------------------------------------------------------------------------|
| `map_tokens`          | int          | 1024      | Maximum tokens in the repo map output                                         |
| `root`                | path         | cwd       | Repository root directory                                                     |
| `repo_content_prefix` | string\|None | None      | Prefix prepended to the map output                                            |
| `verbose`             | bool         | false     | Emit diagnostic output: initialization message, per-file scan progress, rendered map token count, and cache errors |
| `max_context_window`  | int\|None    | None      | LLM context window size; enables no-chat expansion                            |
| `map_mul_no_files`    | int          | 8         | Multiplier for token budget when no chat files are open                       |
| `refresh`             | string       | "auto"    | Map cache refresh mode: "auto", "manual", "always", "files"                  |
| `force_refresh`       | bool         | false     | Bypass cache reads and force map recomputation (§11)                          |
| `exclude_unranked`    | bool         | false     | When true, files with PageRank ≤ 0.0001 are excluded from the map output      |
| `self_edge_weight`    | float        | 0.1       | Weight applied to self-edges for unreferenced definitions (§6.5)              |
| `max_line_length`     | int          | 100       | Maximum characters per output line before truncation (§9.1, §12.4)           |
| `pagerank_damping`    | float        | 0.85      | PageRank damping factor (§7.2)                                                |
| `pagerank_tol`        | float        | 1.0e-6    | PageRank convergence tolerance (§7.2)                                         |
| `pagerank_max_iter`   | int          | 100       | PageRank maximum iterations (§7.2)                                            |

**`map_tokens` and `max_map_tokens`:** `map_tokens` is the constructor parameter stored at
initialization. Internally, the algorithm references this as `max_map_tokens`. They represent
the same value; `max_map_tokens` may be temporarily overridden within a single call (§10.3) or
permanently set to 0 by §13.4. The stored `map_tokens` field is never mutated.

**I/O abstraction:** The reference implementation injects a `main_model` for token counting and
an `io` object for file reading and diagnostic emission. The Rust library MUST provide
caller-injectable equivalents for: (1) token counting — satisfied by the tiktoken encoder
(§10.1); (2) diagnostic emission (warnings, errors, verbose output) — the mechanism is an
Open Question (see §15).

---

## 15. Open Questions

1. **I/O and diagnostic abstraction** — The reference implementation injects `io` for file
   reading, warning emission, and verbose output. The Rust library must define its own
   abstraction. Options include: a trait object (`dyn RepoMapIO`), a callback struct, a
   logging crate integration (e.g., `tracing`), or simply `eprintln!`/`log::warn!`. The
   choice affects the public API surface. Resolve before implementing §3.1 file reading
   and §13.5 warning emission.

---

## 16. Non-Goals

The following are explicitly out of scope for this specification:

- The LLM interface or model integration.
- File watching or automatic re-indexing on filesystem changes. DEFERRED
- Git integration (the reference uses git history only for warning messages about deleted files).
- The interactive UI (spinner, progress callbacks) — these are implementation details for the caller. DEFERRED
- Any output format beyond the plain-text repo map string described in §12. DEFERRED
