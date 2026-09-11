//! PIN gate crypto (spec §6.4.1 / §7.3, finalized 2026-09-08): Argon2id via
//! the RustCrypto `argon2` crate, PHC string format. `pin_hash` stores the
//! full PHC string (self-describing salt + cost params); `pin_salts` stores
//! the base64 salt separately for explicit access / future rotation.
//!
//! Flow semantics (spec §6.4.1):
//! - **set-PIN** (first run, exactly once): 4–12 digits, hashed with
//!   Argon2id, stored with its salt. Set never overwrites an existing
//!   account (no PIN change flow in V1); account creation triggers the
//!   character-creation bootstrap (§6.4.11).
//! - **verify**: re-derive against the stored PHC string (missing account =
//!   locked); a wrong PIN is a rejection, not an error. A malformed stored
//!   hash is an operational error.

use crate::{ProbeError, Result};
use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::Argon2;
use rand_core::OsRng;

pub const MIN_PIN_DIGITS: usize = 4;
pub const MAX_PIN_DIGITS: usize = 12;

/// A freshly hashed PIN: the full PHC string for `accounts.pin_hash` and the
/// separate base64 salt for `accounts.pin_salts` (§6.4.1 storage contract).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PinHash {
    pub phc_string: String,
    pub salt_b64: String,
}

/// Rejects PINs that are not 4–12 digits (spec §6.4.1: digits only).
pub fn validate_pin(pin: &str) -> Result<()> {
    let is_valid = pin.len() >= MIN_PIN_DIGITS
        && pin.len() <= MAX_PIN_DIGITS
        && pin.chars().all(|character| character.is_ascii_digit());
    if !is_valid {
        return Err(ProbeError::InvalidPin);
    }
    Ok(())
}

/// Hashes a validated PIN with Argon2id (default OWASP-tuned crate params)
/// into the PHC string format, extracting the base64 salt for `pin_salts`.
pub fn hash_pin(pin: &str) -> Result<PinHash> {
    validate_pin(pin)?;
    let salt = SaltString::generate(&mut OsRng);
    let phc_string = Argon2::default()
        .hash_password(pin.as_bytes(), &salt)
        .map_err(|error| ProbeError::PinHashing(error.to_string()))?
        .to_string();
    let salt_b64 = salt.as_str().to_owned();
    Ok(PinHash {
        phc_string,
        salt_b64,
    })
}

/// Outcome of a verify attempt (spec §6.4.1): a wrong PIN is a rejection,
/// not an error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PinVerifyOutcome {
    Accepted,
    Rejected,
}

/// Re-derives the PIN against a stored PHC string using the params embedded
/// in the string. A malformed stored hash is an operational error, never a
/// login rejection.
pub fn verify_pin(pin: &str, stored_phc_string: &str) -> Result<PinVerifyOutcome> {
    let parsed = PasswordHash::new(stored_phc_string)
        .map_err(|error| ProbeError::PinHashing(error.to_string()))?;
    let matches = Argon2::default()
        .verify_password(pin.as_bytes(), &parsed)
        .is_ok();
    Ok(if matches {
        PinVerifyOutcome::Accepted
    } else {
        PinVerifyOutcome::Rejected
    })
}

#[cfg(test)]
mod tests {
    use super::{
        hash_pin, validate_pin, verify_pin, PinHash, PinVerifyOutcome, MAX_PIN_DIGITS,
        MIN_PIN_DIGITS,
    };
    use crate::ProbeError;

    #[test]
    fn accepts_valid_digit_pins_and_rejects_everything_else() {
        assert!(validate_pin("1234").is_ok());
        assert!(validate_pin("000000").is_ok());
        assert!(validate_pin(&"9".repeat(MAX_PIN_DIGITS)).is_ok());

        for invalid_pin in [
            "",                       // too short
            "123",                    // 3 digits < MIN
            "1234567890123",          // 13 digits > MAX
            "12a4",                   // letter
            "12 4",                   // whitespace
            "12.4",                   // punctuation
            "+1234",                  // sign
            "１２３４",                // non-ASCII digits
        ] {
            assert!(
                matches!(validate_pin(invalid_pin), Err(ProbeError::InvalidPin)),
                "PIN {invalid_pin:?} should be rejected"
            );
        }
        assert_eq!(MIN_PIN_DIGITS, 4);
    }

    #[test]
    fn hash_pin_rejects_invalid_pins_before_hashing() {
        assert!(matches!(hash_pin("12"), Err(ProbeError::InvalidPin)));
        assert!(matches!(hash_pin("abcd"), Err(ProbeError::InvalidPin)));
    }

    #[test]
    fn hash_pin_produces_argon2id_phc_string_with_separate_salt() {
        let hashed = hash_pin("31337").unwrap();
        assert!(hashed.phc_string.starts_with("$argon2id$"));
        assert!(hashed.phc_string.contains("m="));
        assert!(hashed.phc_string.contains("t="));
        assert!(hashed.phc_string.contains("p="));
        // SaltString::generate emits PHC base64 (B64 encoding, no padding),
        // so the salt never contains '=' or non-base64 characters.
        assert!(!hashed.salt_b64.is_empty());
        assert!(hashed
            .salt_b64
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'+' || byte == b'/'));
        assert!(!hashed.phc_string.ends_with(&hashed.salt_b64));
    }

    #[test]
    fn hash_pin_generates_a_unique_salt_per_call() {
        let first: PinHash = hash_pin("2468").unwrap();
        let second = hash_pin("2468").unwrap();
        assert_ne!(first.salt_b64, second.salt_b64);
        assert_ne!(first.phc_string, second.phc_string);
    }

    #[test]
    fn verifies_correct_pin_and_rejects_wrong_pin() {
        let hashed = hash_pin("90210").unwrap();
        assert_eq!(
            verify_pin("90210", &hashed.phc_string).unwrap(),
            PinVerifyOutcome::Accepted
        );
        assert_eq!(
            verify_pin("90211", &hashed.phc_string).unwrap(),
            PinVerifyOutcome::Rejected
        );
        assert_eq!(
            verify_pin("", &hashed.phc_string).unwrap(),
            PinVerifyOutcome::Rejected
        );
    }

    #[test]
    fn verification_survives_serialization_round_trip() {
        // simulates storage in accounts.pin_hash (text column)
        let stored = hash_pin("1357").unwrap().phc_string;
        assert_eq!(
            verify_pin("1357", &stored).unwrap(),
            PinVerifyOutcome::Accepted
        );
    }

    #[test]
    fn malformed_stored_hash_is_an_operational_error_not_a_rejection() {
        let error = verify_pin("1234", "not-a-phc-string").unwrap_err();
        assert!(matches!(error, ProbeError::PinHashing(_)));
    }
}
