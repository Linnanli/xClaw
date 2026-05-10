//! PEM ↔ DER decoding and SHA-256 fingerprinting helpers.
//!
//! Shared by all platform backends (macOS / Windows / Linux). Kept
//! deliberately small: only the public functions actually used by the
//! `TrustStore` impls are exposed.

use sha2::Digest;

use crate::Result;
use crate::error::Error;

/// Decode the first `CERTIFICATE` block from a PEM byte slice into DER.
///
/// Returns [`Error::InvalidPem`] when:
/// - the input contains no `-----BEGIN CERTIFICATE-----` header,
/// - the base64 body is malformed,
/// - the first item is not a `CERTIFICATE` (e.g. a private key block).
pub(crate) fn decode_first_certificate(ca_pem: &[u8]) -> Result<Vec<u8>> {
    let mut cursor = std::io::Cursor::new(ca_pem);
    match rustls_pemfile::read_one(&mut cursor) {
        Ok(Some(rustls_pemfile::Item::X509Certificate(der))) => Ok(der.as_ref().to_vec()),
        Ok(Some(_)) => Err(Error::InvalidPem(
            "first PEM block is not a CERTIFICATE".into(),
        )),
        Ok(None) => Err(Error::InvalidPem(
            "no CERTIFICATE block found in PEM input".into(),
        )),
        Err(err) => Err(Error::InvalidPem(format!("PEM decode failed: {err}"))),
    }
}

/// Compute the lowercase-hex SHA-256 fingerprint of a DER-encoded certificate.
///
/// This is the canonical identity dasclaw uses to recognize "its" CA inside
/// the OS trust store. Stored alongside the installed CA in
/// `$CODEX_HOME/proxy/installed-ca.fingerprint` so subsequent
/// `status` / `uninstall` calls can locate it without iterating every entry.
pub(crate) fn sha256_hex(der: &[u8]) -> String {
    let digest = sha2::Sha256::digest(der);
    let mut s = String::with_capacity(digest.len() * 2);
    for byte in digest {
        use std::fmt::Write as _;
        let _ = write!(s, "{byte:02x}");
    }
    s
}

/// PEM-wrap a DER blob with standard 64-char line wrapping.
///
/// Shared by every platform backend's `export()` implementation so the
/// output is byte-identical regardless of trust store. Base64 output is
/// pure ASCII by construction, so chunk slicing never crosses a UTF-8
/// boundary and we can index `&str` directly without a UTF-8 re-check.
pub(crate) fn der_to_pem(der: &[u8]) -> Vec<u8> {
    use base64::Engine as _;
    let b64 = base64::engine::general_purpose::STANDARD.encode(der);
    let mut out = String::with_capacity(b64.len() + 64);
    out.push_str("-----BEGIN CERTIFICATE-----\n");
    let bytes = b64.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let end = (i + 64).min(bytes.len());
        // Base64 alphabet is ASCII, so byte slice == char slice.
        out.push_str(&b64[i..end]);
        out.push('\n');
        i = end;
    }
    out.push_str("-----END CERTIFICATE-----\n");
    out.into_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;

    // Self-signed CA generated with `openssl req -x509 -newkey rsa:2048
    // -keyout /tmp/k.pem -out /tmp/c.pem -days 1 -nodes -subj
    // "/CN=dasclaw-test-ca"`. Used purely as a fixture; private key is
    // discarded.
    const SAMPLE_CA_PEM: &[u8] = include_bytes!("../tests/fixtures/sample-ca.pem");

    #[test]
    fn decode_first_certificate_accepts_valid_pem() {
        let der = decode_first_certificate(SAMPLE_CA_PEM).expect("valid PEM should decode");
        assert!(!der.is_empty(), "DER body must not be empty");
        // X.509 v3 certs start with a SEQUENCE tag (0x30).
        assert_eq!(der[0], 0x30, "DER must start with ASN.1 SEQUENCE tag");
    }

    #[test]
    fn decode_first_certificate_rejects_garbage() {
        let err = decode_first_certificate(b"not a pem at all").expect_err("must reject garbage");
        assert!(matches!(err, Error::InvalidPem(_)));
    }

    #[test]
    fn decode_first_certificate_rejects_empty_input() {
        let err = decode_first_certificate(b"").expect_err("must reject empty input");
        assert!(matches!(err, Error::InvalidPem(_)));
    }

    #[test]
    fn decode_first_certificate_rejects_non_certificate_block() {
        // RSA private key in PEM — first item is not a CERTIFICATE.
        let pkey = b"-----BEGIN PRIVATE KEY-----\n\
                     MC4CAQAwBQYDK2VwBCIEIB7yzExZsh8WJSE9C9XbVPWa3wzWjAdJYwcl3vIY3Wnh\n\
                     -----END PRIVATE KEY-----\n";
        let err = decode_first_certificate(pkey).expect_err("must reject private key");
        match err {
            Error::InvalidPem(msg) => {
                assert!(msg.contains("not a CERTIFICATE"), "wrong message: {msg}");
            }
            other => panic!("expected InvalidPem, got {other:?}"),
        }
    }

    #[test]
    fn sha256_hex_matches_known_digest() {
        // Sanity-check the formatter: SHA-256("") is well-known.
        let expected = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
        assert_eq!(sha256_hex(b""), expected);
    }

    #[test]
    fn sha256_hex_is_stable_for_sample_ca() {
        let der = decode_first_certificate(SAMPLE_CA_PEM).expect("decode");
        let fp = sha256_hex(&der);
        assert_eq!(fp.len(), 64, "SHA-256 hex must be 64 chars");
        // Re-running yields identical output (no randomness).
        assert_eq!(fp, sha256_hex(&der));
    }

    #[test]
    fn der_to_pem_round_trips_through_decode() {
        let der = decode_first_certificate(SAMPLE_CA_PEM).expect("decode");
        let pem = der_to_pem(&der);
        // Header / footer are mandatory.
        assert!(pem.starts_with(b"-----BEGIN CERTIFICATE-----\n"));
        assert!(pem.ends_with(b"-----END CERTIFICATE-----\n"));
        // Re-decoding the wrapped PEM yields the same DER body.
        let der2 = decode_first_certificate(&pem).expect("re-decode");
        assert_eq!(der, der2, "PEM rewrap must round-trip to original DER");
    }

    #[test]
    fn der_to_pem_wraps_lines_at_64_chars() {
        let der = decode_first_certificate(SAMPLE_CA_PEM).expect("decode");
        let pem = der_to_pem(&der);
        let text = std::str::from_utf8(&pem).expect("PEM is ASCII");
        // All body lines (between header/footer) must be ≤ 64 chars.
        for line in text.lines() {
            if line.starts_with("---") {
                continue;
            }
            assert!(
                line.len() <= 64,
                "body line longer than 64: {} chars: {line:?}",
                line.len()
            );
        }
    }

    #[test]
    fn der_to_pem_handles_empty_der() {
        // An empty DER blob yields an empty body (no panics, no UB).
        let pem = der_to_pem(b"");
        assert_eq!(
            pem,
            b"-----BEGIN CERTIFICATE-----\n-----END CERTIFICATE-----\n"
        );
    }
}
