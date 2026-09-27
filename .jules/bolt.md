## 2026-09-27 - Precompile String Finders in Query Preparation

**Learning:** `memchr::memmem::find(haystack, needle)` constructs a `memchr::memmem::Finder` instance internally on every invocation. Calling `find()` inside a loop over 100,000 candidate items introduces repeated finder construction overhead (~7% runtime regression). Storing a pre-constructed `Finder` (or `Box<Finder>` to prevent `large_enum_variant` clippy warnings) in the prepared query struct avoids per-item finder setup overhead.

**Action:** Always pre-build `memchr::memmem::Finder` instances during query compilation / preparation instead of calling convenience functions like `memchr::memmem::find` inside inner search loops over collections.
