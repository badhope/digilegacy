//! Deterministic fuzz smoke for parsing and security boundaries.
//!
//! These are not coverage-guided libFuzzer runs. They are seeded, repeatable
//! inputs that exercise the same public entry points on every `cargo test`, so a
//! parser panic cannot hide behind a hand-written happy path.

use digilegacy_security::crypto::parse_kdf_params;
use digilegacy_security::recovery::{normalize_recovery_code, validate_recovery_seal_structure};
use digilegacy_security::shares::{decode_share, encode_share};
use digilegacy_security::validate_external_url;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

const SEED: u64 = 0xD161_E6A1;
const CASES: usize = 10_000;
const CHARS: &[char] = &[
    'a', 'Z', '0', '9', '-', '_', '.', ':', '/', '?', '#', '[', ']', '@', '%', '+', '=', '&', '<',
    '>', '"', '\'', '\\', ' ', '\t', '\r', '\n', '\0', '中', '文', 'é', '🧪',
];

fn text(rng: &mut StdRng, max_chars: usize) -> String {
    let len = rng.random_range(0..=max_chars);
    (0..len)
        .map(|_| CHARS[rng.random_range(0..CHARS.len())])
        .collect()
}

fn base64ish(rng: &mut StdRng, max_chars: usize) -> String {
    const B64: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/=-_";
    let len = rng.random_range(0..=max_chars);
    (0..len)
        .map(|_| B64[rng.random_range(0..B64.len())] as char)
        .collect()
}

#[test]
fn fuzz_external_url_validation_never_panics() {
    let mut rng = StdRng::seed_from_u64(SEED);
    for _ in 0..CASES {
        let candidate = format!("http://{}", text(&mut rng, 72));
        if let Ok(accepted) = validate_external_url(&candidate) {
            let lower = accepted.to_ascii_lowercase();
            assert!(
                lower.starts_with("http://") || lower.starts_with("https://"),
                "accepted a non-http URL: {accepted:?}"
            );
            assert!(
                !accepted.chars().any(|c| c.is_control()),
                "accepted a control character: {accepted:?}"
            );
            assert!(
                !accepted.contains('\u{0}'),
                "accepted a NUL byte: {accepted:?}"
            );
        }
    }

    for _ in 0..CASES {
        let candidate = text(&mut rng, 96);
        let _ = validate_external_url(&candidate);
    }
}

#[test]
fn fuzz_recovery_codes_and_seals_never_panic() {
    let mut rng = StdRng::seed_from_u64(SEED ^ 0x0052_4543);
    for _ in 0..CASES {
        let input = text(&mut rng, 64);
        let normalized = normalize_recovery_code(&input);
        assert!(!normalized.chars().any(|c| c.is_whitespace() || c == '-'));
        assert_eq!(
            normalize_recovery_code(&normalized),
            normalized,
            "normalization is not idempotent for {input:?}"
        );
    }

    for prefix in ["", "RS2.", "RS3."] {
        for _ in 0..CASES {
            let seal = format!("{prefix}{}", base64ish(&mut rng, 80));
            let _ = validate_recovery_seal_structure(&seal);
        }
    }
}

#[test]
fn fuzz_share_decoding_never_panics() {
    let mut rng = StdRng::seed_from_u64(SEED ^ 0x0053_4841);
    for _ in 0..CASES {
        let code = format!("DGL1-{}", base64ish(&mut rng, 96));
        if let Ok(share) = decode_share(&code) {
            let encoded = encode_share(&share);
            assert!(
                decode_share(&encoded).is_ok(),
                "decoded share did not round-trip: {code:?}"
            );
        }
    }
}

#[test]
fn fuzz_kdf_parameters_stay_within_bounds() {
    let mut rng = StdRng::seed_from_u64(SEED ^ 0x004B_4446);
    for prefix in ["", "v2.", "v3."] {
        for _ in 0..CASES {
            let salt = format!("{prefix}{}", base64ish(&mut rng, 64));
            if let Ok(digilegacy_security::crypto::KdfParams::Argon2id {
                m_cost,
                t_cost,
                p_cost,
                ..
            }) = parse_kdf_params(&salt)
            {
                assert!(m_cost >= 8 * 1024, "Argon2 m_cost below floor: {m_cost}");
                assert!(t_cost >= 1, "Argon2 t_cost below floor: {t_cost}");
                assert!(p_cost >= 1, "Argon2 p_cost below floor: {p_cost}");
            }
        }
    }
}
