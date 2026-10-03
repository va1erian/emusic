## 2026-09-28 - Pre-building memmem Finders in PreparedQuery
**Learning:** `memchr::memmem::find` constructs a new `Finder` on every call. Calling it per candidate track in a loop recreates vector search tables thousands of times.
**Action:** Store `memchr::memmem::Finder` in `PreparedTerm` so search finders are pre-computed once when creating `PreparedQuery`.

## 2026-09-29 - Fast TrackId Hasher in Search Index
**Learning:** Standard library `HashMap` uses SipHash-1-3 by default. For candidate lookups by 64-bit `TrackId` keys in high-throughput loops (e.g. 100k candidates per query), SipHash hashing overhead consumes significant CPU cycles.
**Action:** Use `BuildHasherDefault<TrackIdHasher>` with a fast multiplicative integer hash (`0x9e37_79b9_7f4a_7c15`) for 64-bit integer keys in performance-sensitive in-memory indexes.

## 2026-10-01 - Avoid Double-Allocation in Deunicode & Char-by-Char Haystack Appends
**Learning:** `deunicode::deunicode` returns a guaranteed ASCII `String`. Calling `.to_lowercase()` allocates a second `String` on the heap when `.make_ascii_lowercase()` mutates in-place. For pure ASCII strings, `input.is_ascii()` allows bypassing `deunicode` entirely. In haystack string construction, pushing char-by-char causes UTF-8 decoding/re-encoding and incremental reallocations, which can be avoided with initial capacity reservation and `push_str`.
**Action:** Use ASCII fast path and `make_ascii_lowercase()` in string normalization, and pre-allocate capacity with bulk `push_str` when building search haystacks.

## 2026-10-02 - Positional Column Indexing in SQLite Row Deserialization
**Learning:** `rusqlite`'s `row.get("col_name")` performs string linear/hash lookups on column names per field per row. For wide tables (e.g. 23 columns in `tracks`), deserializing thousands of rows in bulk queries like `sync_since` performs hundreds of thousands of string comparisons.
**Action:** Use positional 0-based integer column indexing (`row.get(0)`, `row.get(1)`, ...) when mapping query rows matching static SQL column lists (`TRACK_COLUMNS`).
