## 2026-09-28 - Pre-building memmem Finders in PreparedQuery
**Learning:** `memchr::memmem::find` constructs a new `Finder` on every call. Calling it per candidate track in a loop recreates vector search tables thousands of times.
**Action:** Store `memchr::memmem::Finder` in `PreparedTerm` so search finders are pre-computed once when creating `PreparedQuery`.

## 2026-09-29 - Fast TrackId Hasher in Search Index
**Learning:** Standard library `HashMap` uses SipHash-1-3 by default. For candidate lookups by 64-bit `TrackId` keys in high-throughput loops (e.g. 100k candidates per query), SipHash hashing overhead consumes significant CPU cycles.
**Action:** Use `BuildHasherDefault<TrackIdHasher>` with a fast multiplicative integer hash (`0x9e37_79b9_7f4a_7c15`) for 64-bit integer keys in performance-sensitive in-memory indexes.

## 2026-09-30 - ASCII Fast Path in Text Normalization & Haystack Building
**Learning:** `normalize_text` called `deunicode::deunicode` followed by `to_lowercase()`, allocating two strings and scanning unicode tables even for plain ASCII metadata.
**Action:** Fast-path `input.is_ascii()` with `input.to_ascii_lowercase()` for single-allocation ASCII lowercasing, pre-allocate haystack builder capacity, and bulk copy strings when control characters are absent.
