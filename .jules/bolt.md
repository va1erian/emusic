## 2026-09-28 - Pre-building memmem Finders in PreparedQuery
**Learning:** `memchr::memmem::find` constructs a new `Finder` on every call. Calling it per candidate track in a loop recreates vector search tables thousands of times.
**Action:** Store `memchr::memmem::Finder` in `PreparedTerm` so search finders are pre-computed once when creating `PreparedQuery`.

## 2026-09-29 - Fast TrackId Hasher in Search Index
**Learning:** Standard library `HashMap` uses SipHash-1-3 by default. For candidate lookups by 64-bit `TrackId` keys in high-throughput loops (e.g. 100k candidates per query), SipHash hashing overhead consumes significant CPU cycles.
**Action:** Use `BuildHasherDefault<TrackIdHasher>` with a fast multiplicative integer hash (`0x9e37_79b9_7f4a_7c15`) for 64-bit integer keys in performance-sensitive in-memory indexes.

## 2026-10-01 - Avoid Double-Allocation in Deunicode & Char-by-Char Haystack Appends
**Learning:** `deunicode::deunicode` returns a guaranteed ASCII `String`. Calling `.to_lowercase()` allocates a second `String` on the heap when `.make_ascii_lowercase()` mutates in-place. For pure ASCII strings, `input.is_ascii()` allows bypassing `deunicode` entirely. In haystack string construction, pushing char-by-char causes UTF-8 decoding/re-encoding and incremental reallocations, which can be avoided with initial capacity reservation and `push_str`.
**Action:** Use ASCII fast path and `make_ascii_lowercase()` in string normalization, and pre-allocate capacity with bulk `push_str` when building search haystacks.

## 2026-10-02 - Direct ASCII Lowercasing in Haystack Builder
**Learning:** Even with `normalize_text` fast-pathing ASCII inputs, passing `value` through `normalize_text` allocated temporary `String`s for each track field during index construction. Lowercasing ASCII bytes directly into the pre-allocated haystack `String` buffer eliminates per-field allocations entirely.
**Action:** Append ASCII fields directly into the destination string buffer while converting case byte-by-byte to avoid intermediate `String` allocations during bulk object index building.

## 2026-10-03 - Single-Pass Track Filtering and Display Row Generation
**Learning:** `SearchPopup::collect_rows` iterated over `library.tracks()` twice with `track_match(t.id)`—once to collect matching track IDs and again to take top result display rows. In large libraries, evaluating closure predicates twice over tens of thousands of tracks doubles search popup refresh time.
**Action:** Collect matched candidate IDs and capped display rows simultaneously in a single pass over the candidate collection.
