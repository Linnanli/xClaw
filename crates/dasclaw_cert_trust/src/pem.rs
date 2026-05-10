//! PEM ↔ DER decoding and SHA-256 fingerprinting helpers.
//!
//! Shared by all platform backends (macOS / Windows / Linux). Kept
//! deliberately small: only the public functions actually used by the
//! `TrustStore` impls are exposed.

use sha2::Digest;
use x509_parser::prelude::FromDer as _;

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

/// Validate that a DER-encoded X.509 certificate is fit to be installed
/// as a per-user **root** CA.
///
/// Implements the three RFC 5280 sanity checks called out in issue #375 /
/// ADR-139 §3.7 (fail-safe error handling). The OS keychain APIs do not
/// perform these checks themselves: they accept any well-formed DER and
/// will then trust it as a root, so without this gate a caller could
/// promote an arbitrary leaf cert into the user's trust store.
///
/// Rejected cases (each returns [`Error::InvalidCa`] with a specific
/// reason string so the CLI can give an actionable message):
///
/// 1. **Not a CA** — `BasicConstraints` extension is absent or has
///    `cA: false`. RFC 5280 §4.2.1.9 requires `cA: TRUE` for any cert
///    that issues other certificates.
/// 2. **Expired** — `notAfter` is already in the past. We only check
///    `notAfter` (not `notBefore`); a not-yet-valid CA is unusual but
///    legitimate (e.g. pre-staged rotation), while an already-expired CA
///    cannot validate any TLS handshake.
/// 3. **Not self-signed** — issuer DN ≠ subject DN. Only self-signed
///    roots belong in a root store; an intermediate placed there would
///    silently fail chain validation against any legitimate root.
///
/// Comparison of issuer and subject uses the **DER-encoded** name bytes
/// per RFC 5280 §4.1.2.4 — string-form comparison would normalize whitespace
/// and case in ways that don't match what the validator does at handshake
/// time.
///
/// We deliberately do **not** verify the self-signature here. That would
/// require a parsed public key + signature algorithm ladder, which
/// `dasclaw_net_proxy` will already have done when generating the CA;
/// re-doing it on the consumer side adds attack surface (parsing new
/// algorithm OIDs) for no extra safety beyond the three checks above.
pub(crate) fn validate_root_ca(der: &[u8]) -> Result<()> {
    let (_rest, cert) = x509_parser::certificate::X509Certificate::from_der(der)
        .map_err(|err| Error::InvalidCa(format!("X.509 parse failed: {err}")))?;

    // Check 1: BasicConstraints CA:TRUE.
    match cert
        .basic_constraints()
        .map_err(|err| Error::InvalidCa(format!("BasicConstraints parse failed: {err}")))?
    {
        Some(ext) if ext.value.ca => {}
        Some(_) => {
            return Err(Error::InvalidCa(
                "BasicConstraints extension present but cA:FALSE (leaf cert, not a CA)".into(),
            ));
        }
        None => {
            return Err(Error::InvalidCa(
                "BasicConstraints extension absent (cannot confirm cA:TRUE)".into(),
            ));
        }
    }

    // Check 2: notAfter not in the past (no clock-skew tolerance — the OS
    // will reject expired certs at handshake time, and a CA that is "barely
    // valid right now" is itself a problem we want to surface early).
    let now = x509_parser::time::ASN1Time::now();
    if !cert.validity().is_valid_at(now) {
        return Err(Error::InvalidCa(format!(
            "certificate not valid at current time (notBefore={}, notAfter={})",
            cert.validity().not_before,
            cert.validity().not_after,
        )));
    }

    // Check 3: self-signed (issuer DN == subject DN, byte-equal in DER).
    if cert.subject().as_raw() != cert.issuer().as_raw() {
        return Err(Error::InvalidCa(
            "issuer DN ≠ subject DN (not a self-signed root)".into(),
        ));
    }

    Ok(())
}

/// Decode + validate a PEM-encoded root CA in one step.
///
/// Equivalent to `decode_first_certificate` followed by `validate_root_ca`.
/// This is the entry point every `TrustStore::install` impl must use, so
/// that platform backends never see an unvalidated PEM. The two-step
/// composition is exposed so unit tests can target each stage in
/// isolation.
pub(crate) fn decode_and_validate_root_ca(ca_pem: &[u8]) -> Result<Vec<u8>> {
    let der = decode_first_certificate(ca_pem)?;
    validate_root_ca(&der)?;
    Ok(der)
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

    // ------------------------------------------------------------------
    // validate_root_ca / decode_and_validate_root_ca — issue #375.
    //
    // Each fixture builder produces a freshly-generated cert via `rcgen`
    // so the assertions don't drift when system time crosses any
    // hard-coded `notAfter`. Builders return the DER bytes (the input
    // shape of `validate_root_ca`).
    // ------------------------------------------------------------------

    /// Build a self-signed CA whose `notBefore`/`notAfter` are set
    /// relative to "now" by the supplied offsets in days. Negative values
    /// move the window into the past.
    fn build_self_signed_ca_with_validity(days_before: i64, days_after: i64) -> Vec<u8> {
        use rcgen::{
            BasicConstraints, CertificateParams, DistinguishedName, DnType, IsCa, KeyPair,
        };
        use time::{Duration, OffsetDateTime};

        let mut params = CertificateParams::new(Vec::<String>::new()).expect("CertificateParams");
        let mut dn = DistinguishedName::new();
        dn.push(DnType::CommonName, "dasclaw-test-root");
        params.distinguished_name = dn;
        params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
        let now = OffsetDateTime::now_utc();
        params.not_before = now + Duration::days(days_before);
        params.not_after = now + Duration::days(days_after);

        let key = KeyPair::generate().expect("keypair");
        let cert = params.self_signed(&key).expect("self-sign");
        cert.der().to_vec()
    }

    /// Build a self-signed leaf certificate (`BasicConstraints CA:FALSE`).
    fn build_self_signed_leaf() -> Vec<u8> {
        use rcgen::{CertificateParams, DistinguishedName, DnType, IsCa, KeyPair};
        use time::{Duration, OffsetDateTime};

        let mut params = CertificateParams::new(Vec::<String>::new()).expect("CertificateParams");
        let mut dn = DistinguishedName::new();
        dn.push(DnType::CommonName, "dasclaw-test-leaf");
        params.distinguished_name = dn;
        params.is_ca = IsCa::ExplicitNoCa;
        let now = OffsetDateTime::now_utc();
        params.not_before = now - Duration::days(1);
        params.not_after = now + Duration::days(30);

        let key = KeyPair::generate().expect("keypair");
        let cert = params.self_signed(&key).expect("self-sign");
        cert.der().to_vec()
    }

    /// Build an intermediate CA signed by a separate root, so issuer DN ≠
    /// subject DN.
    fn build_non_self_signed_intermediate() -> Vec<u8> {
        use rcgen::{
            BasicConstraints, CertificateParams, DistinguishedName, DnType, IsCa, Issuer, KeyPair,
            SignatureAlgorithm,
        };
        use time::{Duration, OffsetDateTime};

        // Root.
        let mut root_params =
            CertificateParams::new(Vec::<String>::new()).expect("CertificateParams");
        let mut root_dn = DistinguishedName::new();
        root_dn.push(DnType::CommonName, "dasclaw-test-root");
        root_params.distinguished_name = root_dn;
        root_params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
        let now = OffsetDateTime::now_utc();
        root_params.not_before = now - Duration::days(1);
        root_params.not_after = now + Duration::days(365);
        let root_key = KeyPair::generate().expect("root keypair");
        let root_alg: &'static SignatureAlgorithm = root_key.algorithm();
        let issuer = Issuer::new(root_params, root_key);

        // Intermediate signed by root.
        let mut int_params =
            CertificateParams::new(Vec::<String>::new()).expect("CertificateParams");
        let mut int_dn = DistinguishedName::new();
        int_dn.push(DnType::CommonName, "dasclaw-test-intermediate");
        int_params.distinguished_name = int_dn;
        int_params.is_ca = IsCa::Ca(BasicConstraints::Constrained(0));
        int_params.not_before = now - Duration::days(1);
        int_params.not_after = now + Duration::days(180);
        let int_key = KeyPair::generate_for(root_alg).expect("intermediate keypair");
        let int_cert = int_params
            .signed_by(&int_key, &issuer)
            .expect("intermediate sign");
        int_cert.der().to_vec()
    }

    #[test]
    fn validate_root_ca_accepts_self_signed_ca() {
        let der = build_self_signed_ca_with_validity(-1, 30);
        validate_root_ca(&der).expect("freshly-minted self-signed CA must validate");
    }

    #[test]
    fn validate_root_ca_rejects_leaf_cert_without_ca_true() {
        let der = build_self_signed_leaf();
        match validate_root_ca(&der).expect_err("leaf cert must be rejected") {
            Error::InvalidCa(msg) => {
                assert!(
                    msg.contains("cA:FALSE") || msg.contains("BasicConstraints"),
                    "unexpected reason: {msg}"
                );
            }
            other => panic!("expected InvalidCa, got {other:?}"),
        }
    }

    #[test]
    fn validate_root_ca_rejects_expired_ca() {
        // notAfter 1 day ago.
        let der = build_self_signed_ca_with_validity(-30, -1);
        match validate_root_ca(&der).expect_err("expired CA must be rejected") {
            Error::InvalidCa(msg) => {
                assert!(
                    msg.contains("not valid"),
                    "expected validity message, got: {msg}"
                );
            }
            other => panic!("expected InvalidCa, got {other:?}"),
        }
    }

    #[test]
    fn validate_root_ca_rejects_non_self_signed_intermediate() {
        let der = build_non_self_signed_intermediate();
        match validate_root_ca(&der).expect_err("intermediate must be rejected") {
            Error::InvalidCa(msg) => {
                assert!(msg.contains("self-signed"), "unexpected reason: {msg}");
            }
            other => panic!("expected InvalidCa, got {other:?}"),
        }
    }

    #[test]
    fn validate_root_ca_rejects_garbage_der() {
        let err = validate_root_ca(b"not der at all").expect_err("garbage DER must be rejected");
        assert!(matches!(err, Error::InvalidCa(_)));
    }

    #[test]
    fn decode_and_validate_root_ca_accepts_valid_pem() {
        // The fixture `sample-ca.pem` is a valid self-signed CA whose
        // notAfter is far in the future (`-days 36500` when generated).
        // If this assertion ever fires in CI it means the fixture has
        // genuinely expired and needs regeneration.
        decode_and_validate_root_ca(SAMPLE_CA_PEM)
            .expect("sample-ca.pem must validate as a root CA");
    }

    #[test]
    fn decode_and_validate_root_ca_surfaces_pem_errors_first() {
        // PEM-syntax error should produce InvalidPem, not InvalidCa, so
        // the CLI can give the right diagnosis.
        let err =
            decode_and_validate_root_ca(b"not a pem at all").expect_err("garbage input must fail");
        assert!(
            matches!(err, Error::InvalidPem(_)),
            "expected InvalidPem, got {err:?}"
        );
    }
}
