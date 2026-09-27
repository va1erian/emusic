//! Bindings generator entry point.
//!
//! Built only with `--features cli`:
//!
//! ```text
//! cargo run -p emusic-mobile --features cli --bin uniffi-bindgen -- \
//!     generate --library <libemusic_mobile.so> --language kotlin --out-dir <dir>
//! ```

#![forbid(unsafe_code)]

fn main() {
    uniffi::uniffi_bindgen_main()
}
