//! Pairing code generator for emusic-server.

use rand::Rng;

/// Generates a secure random 6-digit pairing code (e.g. "849201").
pub fn generate_pairing_code() -> String {
    let mut rng = rand::thread_rng();
    let code: u32 = rng.gen_range(100_000..999_999);
    code.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_pairing_code() {
        let code = generate_pairing_code();
        assert_eq!(code.len(), 6);
        assert!(code.chars().all(|c| c.is_ascii_digit()));
    }
}
