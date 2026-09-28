## 2026-09-28 - Pre-building memmem Finders in PreparedQuery
**Learning:** `memchr::memmem::find` constructs a new `Finder` on every call. Calling it per candidate track in a loop recreates vector search tables thousands of times.
**Action:** Store `memchr::memmem::Finder` in `PreparedTerm` so search finders are pre-computed once when creating `PreparedQuery`.
