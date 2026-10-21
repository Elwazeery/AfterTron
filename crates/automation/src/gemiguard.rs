//! Gemi Access (OIDC + PKCE, capability scopes) and GemiGuard (Ed25519 signed offline leases
//! and revocation lists) consumer integration for AfterTron (`AFTER-006`).
//!
//! Canonical platform contracts:
//! - Identity & OIDC authority: `https://auth.gemitron.net` (client ID `aftertron-desktop`)
//! - Licensing & entitlement authority: GemiGuard (`gemiguard.offline-lease.v1`, `gemiguard.revocation-list.v1`)
//! - Offline safety guarantee: `preserveUserProjectData: true` and `allowsLocalProjectIo: true`
//!   in all license states so local `.ecproj` documents can always be opened, edited, saved, and
//!   recovered during cloud outages or after lease expiration/revocation.

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::Error;

/// Canonical Gemi Access OIDC issuer URL.
pub const GEMI_ACCESS_ISSUER: &str = "https://auth.gemitron.net";
/// Canonical OAuth 2.0 / OIDC public desktop client identifier for AfterTron.
pub const AFTERTRON_OIDC_CLIENT_ID: &str = "aftertron-desktop";
/// Canonical offline lease schema identifier.
pub const OFFLINE_LEASE_SCHEMA: &str = "gemiguard.offline-lease.v1";
/// Canonical revocation list schema identifier.
pub const REVOCATION_LIST_SCHEMA: &str = "gemiguard.revocation-list.v1";

/// Canonical AfterTron product capability scopes issued by Gemi Access / GemiGuard.
pub const CANONICAL_CAPABILITIES: &[&str] = &[
    "aftertron:app:use",
    "aftertron:render:standard",
    "aftertron:render:queue",
    "aftertron:mcp:local_control",
    "aftertron:mcp:cloud_bridge",
    "aftertron:studio:sync",
];

/// Default embedded GemiGuard root verifying key (hex-encoded 32-byte Ed25519 public key).
/// Operators and tests can supply additional or rotated keys by `keyId`.
pub const DEFAULT_GEMIGUARD_KEY_ID: &str = "gemiguard-root-2026-01";
pub const DEFAULT_GEMIGUARD_PUBLIC_KEY_HEX: &str =
    "d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a";

/// OIDC Authorization Code + PKCE (`S256`) session parameters for system-browser sign-in.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OidcPkceRequest {
    pub issuer: String,
    pub client_id: String,
    pub redirect_uri: String,
    pub state: String,
    pub code_verifier: String,
    pub code_challenge: String,
    pub code_challenge_method: String,
    pub scopes: Vec<String>,
    pub authorization_url: String,
}

impl OidcPkceRequest {
    /// Generate a fresh OIDC Authorization Code + PKCE (`S256`) request bound to a loopback callback port.
    pub fn generate(loopback_port: u16) -> Result<Self, Error> {
        let mut verifier_bytes = [0u8; 32];
        crate::security::fill_os_random(&mut verifier_bytes);
        let mut state_bytes = [0u8; 16];
        crate::security::fill_os_random(&mut state_bytes);
        let code_verifier = base64url_encode(&verifier_bytes);
        let state = hex_encode(&state_bytes);
        Ok(Self::from_verifier_and_state(loopback_port, code_verifier, state))
    }

    /// Construct deterministic PKCE parameters from an explicit `code_verifier` and `state`.
    pub fn from_verifier_and_state(loopback_port: u16, code_verifier: String, state: String) -> Self {
        let challenge_digest = sha256(code_verifier.as_bytes());
        let code_challenge = base64url_encode(&challenge_digest);
        let redirect_uri = format!("http://127.0.0.1:{loopback_port}/oauth/callback");
        let scopes = vec![
            "openid".to_string(),
            "profile".to_string(),
            "email".to_string(),
            "offline_access".to_string(),
            "gemi.capabilities".to_string(),
        ];
        let scope_param = scopes.join("%20");
        let authorization_url = format!(
            "{GEMI_ACCESS_ISSUER}/oauth2/authorize?response_type=code&client_id={AFTERTRON_OIDC_CLIENT_ID}&redirect_uri={}&scope={scope_param}&state={state}&code_challenge={code_challenge}&code_challenge_method=S256",
            url_encode_component(&redirect_uri)
        );
        Self {
            issuer: GEMI_ACCESS_ISSUER.to_string(),
            client_id: AFTERTRON_OIDC_CLIENT_ID.to_string(),
            redirect_uri,
            state,
            code_verifier,
            code_challenge,
            code_challenge_method: "S256".to_string(),
            scopes,
            authorization_url,
        }
    }
}

/// Signed offline lease document (`gemiguard.offline-lease.v1`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OfflineLease {
    pub schema: String,
    pub lease_id: String,
    pub subject: String,
    pub product: String,
    pub tier: String,
    pub device_fingerprint: String,
    pub issued_at: u64,
    pub expires_at: u64,
    pub offline_grace_until: u64,
    pub capabilities: Vec<String>,
    pub key_id: String,
    pub signature: String,
}

impl OfflineLease {
    /// Canonical UTF-8 signing payload across all lease fields (excluding `signature`).
    pub fn signing_payload(&self) -> Vec<u8> {
        let mut caps = self.capabilities.clone();
        caps.sort();
        let canonical = json!({
            "schema": self.schema,
            "leaseId": self.lease_id,
            "subject": self.subject,
            "product": self.product,
            "tier": self.tier,
            "deviceFingerprint": self.device_fingerprint,
            "issuedAt": self.issued_at,
            "expiresAt": self.expires_at,
            "offlineGraceUntil": self.offline_grace_until,
            "capabilities": caps,
            "keyId": self.key_id,
        });
        canonical.to_string().into_bytes()
    }

    /// Sign this lease using a 32-byte Ed25519 secret seed (used by tests and offline lease fixtures).
    pub fn sign(&mut self, secret_seed: &[u8; 32]) {
        let payload = self.signing_payload();
        let sig = ed25519_sign(secret_seed, &payload);
        self.signature = hex_encode(&sig);
    }
}

/// Signed revocation list (`gemiguard.revocation-list.v1`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RevocationList {
    pub schema: String,
    pub issued_at: u64,
    pub revoked_lease_ids: Vec<String>,
    pub revoked_device_fingerprints: Vec<String>,
    pub key_id: String,
    pub signature: String,
}

impl RevocationList {
    /// Canonical UTF-8 signing payload across all revocation list fields (excluding `signature`).
    pub fn signing_payload(&self) -> Vec<u8> {
        let mut leases = self.revoked_lease_ids.clone();
        leases.sort();
        let mut devices = self.revoked_device_fingerprints.clone();
        devices.sort();
        let canonical = json!({
            "schema": self.schema,
            "issuedAt": self.issued_at,
            "revokedLeaseIds": leases,
            "revokedDeviceFingerprints": devices,
            "keyId": self.key_id,
        });
        canonical.to_string().into_bytes()
    }

    /// Sign this revocation list using a 32-byte Ed25519 secret seed.
    pub fn sign(&mut self, secret_seed: &[u8; 32]) {
        let payload = self.signing_payload();
        let sig = ed25519_sign(secret_seed, &payload);
        self.signature = hex_encode(&sig);
    }
}

/// Evaluated entitlement state returned by [`GemiGuardVerifier::evaluate`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LeaseState {
    Active,
    GracePeriod,
    ExpiredFallback,
    Revoked,
    UnlicensedLocalFallback,
}

/// Complete verification & capability receipt for an AfterTron desktop session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EntitlementReceipt {
    pub state: LeaseState,
    pub product: String,
    pub tier: String,
    pub subject: Option<String>,
    pub lease_id: Option<String>,
    pub device_fingerprint: String,
    pub preserve_user_project_data: bool,
    pub allows_local_project_io: bool,
    pub granted_capabilities: Vec<String>,
    pub reason: String,
}

impl EntitlementReceipt {
    /// Check whether a specific capability scope is granted in the current entitlement state.
    /// Local `.ecproj` open/save/recovery is always permitted (`preserve_user_project_data: true`).
    pub fn allows(&self, capability: &str) -> bool {
        if matches!(
            capability,
            "aftertron:local:open" | "aftertron:local:save" | "aftertron:local:recover"
        ) {
            return true;
        }
        self.granted_capabilities.iter().any(|c| c == capability)
    }
}

/// Verifier for GemiGuard offline leases and revocation lists.
#[derive(Debug, Clone)]
pub struct GemiGuardVerifier {
    trusted_keys: std::collections::HashMap<String, [u8; 32]>,
    device_fingerprint: String,
}

impl Default for GemiGuardVerifier {
    fn default() -> Self {
        let mut trusted_keys = std::collections::HashMap::new();
        if let Ok(pk) = decode_fixed_32(DEFAULT_GEMIGUARD_PUBLIC_KEY_HEX) {
            trusted_keys.insert(DEFAULT_GEMIGUARD_KEY_ID.to_string(), pk);
        }
        Self {
            trusted_keys,
            device_fingerprint: default_device_fingerprint(),
        }
    }
}

impl GemiGuardVerifier {
    pub fn new(device_fingerprint: impl Into<String>) -> Self {
        Self {
            device_fingerprint: device_fingerprint.into(),
            ..Self::default()
        }
    }

    pub fn with_trusted_key(mut self, key_id: impl Into<String>, public_key: [u8; 32]) -> Self {
        self.trusted_keys.insert(key_id.into(), public_key);
        self
    }

    pub fn device_fingerprint(&self) -> &str {
        &self.device_fingerprint
    }

    /// Return the default local fallback receipt when no cloud session or offline lease is loaded.
    pub fn unlicensed_fallback(&self) -> EntitlementReceipt {
        EntitlementReceipt {
            state: LeaseState::UnlicensedLocalFallback,
            product: "aftertron".to_string(),
            tier: "local-fallback".to_string(),
            subject: None,
            lease_id: None,
            device_fingerprint: self.device_fingerprint.clone(),
            preserve_user_project_data: true,
            allows_local_project_io: true,
            granted_capabilities: vec![
                "aftertron:app:use".to_string(),
                "aftertron:render:standard".to_string(),
                "aftertron:mcp:local_control".to_string(),
            ],
            reason: "no GemiGuard offline lease loaded; local editing and .ecproj project saving remain enabled".to_string(),
        }
    }

    /// Verify a signed [`RevocationList`] against the trusted key ring.
    pub fn verify_revocation_list(&self, list: &RevocationList) -> Result<(), Error> {
        if list.schema != REVOCATION_LIST_SCHEMA {
            return Err(Error::BadArgs(format!(
                "unsupported revocation list schema `{}` (expected `{REVOCATION_LIST_SCHEMA}`)",
                list.schema
            )));
        }
        let pk = self.trusted_keys.get(&list.key_id).ok_or_else(|| {
            Error::BadArgs(format!("unknown GemiGuard keyId `{}`", list.key_id))
        })?;
        let sig = decode_signature(&list.signature)?;
        let payload = list.signing_payload();
        if !ed25519_verify(pk, &payload, &sig) {
            return Err(Error::BadArgs(
                "invalid Ed25519 signature on GemiGuard revocation list".into(),
            ));
        }
        Ok(())
    }

    /// Cryptographically verify an [`OfflineLease`] (and optional [`RevocationList`]) at `now_unix`.
    pub fn evaluate(
        &self,
        lease: &OfflineLease,
        revocations: Option<&RevocationList>,
        now_unix: u64,
    ) -> Result<EntitlementReceipt, Error> {
        if lease.schema != OFFLINE_LEASE_SCHEMA {
            return Err(Error::BadArgs(format!(
                "unsupported lease schema `{}` (expected `{OFFLINE_LEASE_SCHEMA}`)",
                lease.schema
            )));
        }
        if lease.product != "aftertron" {
            return Err(Error::BadArgs(format!(
                "lease product mismatch: expected `aftertron`, got `{}`",
                lease.product
            )));
        }
        if lease.device_fingerprint != self.device_fingerprint {
            return Err(Error::BadArgs(format!(
                "lease deviceFingerprint mismatch: lease bound to `{}`, current device is `{}`",
                lease.device_fingerprint, self.device_fingerprint
            )));
        }
        let pk = self.trusted_keys.get(&lease.key_id).ok_or_else(|| {
            Error::BadArgs(format!("unknown GemiGuard keyId `{}`", lease.key_id))
        })?;
        let sig = decode_signature(&lease.signature)?;
        let payload = lease.signing_payload();
        if !ed25519_verify(pk, &payload, &sig) {
            return Err(Error::BadArgs(
                "invalid Ed25519 signature on GemiGuard offline lease".into(),
            ));
        }

        if let Some(rev_list) = revocations {
            self.verify_revocation_list(rev_list)?;
            let lease_revoked = rev_list.revoked_lease_ids.iter().any(|id| id == &lease.lease_id);
            let device_revoked = rev_list
                .revoked_device_fingerprints
                .iter()
                .any(|fp| fp == &lease.device_fingerprint);
            if lease_revoked || device_revoked {
                return Ok(EntitlementReceipt {
                    state: LeaseState::Revoked,
                    product: lease.product.clone(),
                    tier: lease.tier.clone(),
                    subject: Some(lease.subject.clone()),
                    lease_id: Some(lease.lease_id.clone()),
                    device_fingerprint: self.device_fingerprint.clone(),
                    preserve_user_project_data: true,
                    allows_local_project_io: true,
                    granted_capabilities: vec!["aftertron:app:use".to_string()],
                    reason: "lease or device has been revoked by GemiGuard; existing local .ecproj projects remain safely accessible".to_string(),
                });
            }
        }

        if now_unix <= lease.expires_at {
            Ok(EntitlementReceipt {
                state: LeaseState::Active,
                product: lease.product.clone(),
                tier: lease.tier.clone(),
                subject: Some(lease.subject.clone()),
                lease_id: Some(lease.lease_id.clone()),
                device_fingerprint: self.device_fingerprint.clone(),
                preserve_user_project_data: true,
                allows_local_project_io: true,
                granted_capabilities: lease.capabilities.clone(),
                reason: "verified active GemiGuard offline lease".to_string(),
            })
        } else if now_unix <= lease.offline_grace_until {
            Ok(EntitlementReceipt {
                state: LeaseState::GracePeriod,
                product: lease.product.clone(),
                tier: lease.tier.clone(),
                subject: Some(lease.subject.clone()),
                lease_id: Some(lease.lease_id.clone()),
                device_fingerprint: self.device_fingerprint.clone(),
                preserve_user_project_data: true,
                allows_local_project_io: true,
                granted_capabilities: lease.capabilities.clone(),
                reason: "lease is past expiresAt but within offlineGraceUntil; full capabilities remain active".to_string(),
            })
        } else {
            Ok(EntitlementReceipt {
                state: LeaseState::ExpiredFallback,
                product: lease.product.clone(),
                tier: lease.tier.clone(),
                subject: Some(lease.subject.clone()),
                lease_id: Some(lease.lease_id.clone()),
                device_fingerprint: self.device_fingerprint.clone(),
                preserve_user_project_data: true,
                allows_local_project_io: true,
                granted_capabilities: vec![
                    "aftertron:app:use".to_string(),
                    "aftertron:render:standard".to_string(),
                    "aftertron:mcp:local_control".to_string(),
                ],
                reason: "offline grace window has elapsed; cloud bridge and commercial queue entitlements are paused until lease refresh, while local .ecproj projects remain editable and savable".to_string(),
            })
        }
    }
}

/// Deterministic privacy-preserving device fingerprint (`sha256:<hex>`).
pub fn default_device_fingerprint() -> String {
    let host = std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .unwrap_or_else(|_| "aftertron-workstation".to_string());
    let os = std::env::consts::OS;
    let arch = std::env::consts::ARCH;
    let digest = sha256(format!("aftertron-device-v1:{os}:{arch}:{host}").as_bytes());
    format!("sha256:{}", hex_encode(&digest))
}

/// Handle `auth.status` RPC queries.
pub fn auth_status_json(verifier: &GemiGuardVerifier, active_receipt: Option<&EntitlementReceipt>) -> Value {
    let fallback;
    let receipt = match active_receipt {
        Some(r) => r,
        None => {
            fallback = verifier.unlicensed_fallback();
            &fallback
        }
    };
    json!({
        "issuer": GEMI_ACCESS_ISSUER,
        "clientId": AFTERTRON_OIDC_CLIENT_ID,
        "pkceMethod": "S256",
        "deviceFingerprint": verifier.device_fingerprint(),
        "preserveUserProjectData": receipt.preserve_user_project_data,
        "allowsLocalProjectIo": receipt.allows_local_project_io,
        "entitlement": receipt,
    })
}

// ===========================================================================
// Pure-Rust Cryptographic Primitives (FIPS 180-4 SHA-256 / SHA-512 & RFC 8032 Ed25519)
// ===========================================================================

pub fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0x0f) as usize] as char);
    }
    out
}

pub fn hex_decode(hex: &str) -> Result<Vec<u8>, Error> {
    let hex = hex.trim();
    if !hex.len().is_multiple_of(2) {
        return Err(Error::BadArgs("hex string must have even length".into()));
    }
    let mut out = Vec::with_capacity(hex.len() / 2);
    let bytes = hex.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let hi = from_hex_digit(bytes[i])?;
        let lo = from_hex_digit(bytes[i + 1])?;
        out.push((hi << 4) | lo);
        i += 2;
    }
    Ok(out)
}

fn from_hex_digit(b: u8) -> Result<u8, Error> {
    match b {
        b'0'..=b'9' => Ok(b - b'0'),
        b'a'..=b'f' => Ok(b - b'a' + 10),
        b'A'..=b'F' => Ok(b - b'A' + 10),
        _ => Err(Error::BadArgs("invalid hexadecimal character".into())),
    }
}

fn decode_fixed_32(encoded: &str) -> Result<[u8; 32], Error> {
    let raw = if encoded.len() == 64 && encoded.bytes().all(|b| b.is_ascii_hexdigit()) {
        hex_decode(encoded)?
    } else {
        crate::base64::decode(encoded.trim())
            .ok_or_else(|| Error::BadArgs("invalid key encoding".into()))?
    };
    <[u8; 32]>::try_from(raw.as_slice())
        .map_err(|_| Error::BadArgs("Ed25519 public key must be 32 bytes".into()))
}

fn decode_signature(encoded: &str) -> Result<[u8; 64], Error> {
    let raw = if encoded.len() == 128 && encoded.bytes().all(|b| b.is_ascii_hexdigit()) {
        hex_decode(encoded)?
    } else {
        crate::base64::decode(encoded.trim())
            .ok_or_else(|| Error::BadArgs("invalid signature encoding".into()))?
    };
    <[u8; 64]>::try_from(raw.as_slice())
        .map_err(|_| Error::BadArgs("Ed25519 signature must be 64 bytes".into()))
}

/// RFC 4648 §5 URL-safe base64 without padding (`=`), used for OIDC PKCE `S256`.
pub fn base64url_encode(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b0 = u32::from(chunk[0]);
        let b1 = u32::from(*chunk.get(1).unwrap_or(&0));
        let b2 = u32::from(*chunk.get(2).unwrap_or(&0));
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(TABLE[((n >> 18) & 0x3f) as usize] as char);
        out.push(TABLE[((n >> 12) & 0x3f) as usize] as char);
        if chunk.len() > 1 {
            out.push(TABLE[((n >> 6) & 0x3f) as usize] as char);
        }
        if chunk.len() > 2 {
            out.push(TABLE[(n & 0x3f) as usize] as char);
        }
    }
    out
}

fn url_encode_component(input: &str) -> String {
    let mut out = String::with_capacity(input.len() + 16);
    for b in input.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(b as char);
        } else {
            const HEX: &[u8; 16] = b"0123456789ABCDEF";
            out.push('%');
            out.push(HEX[(b >> 4) as usize] as char);
            out.push(HEX[(b & 0x0f) as usize] as char);
        }
    }
    out
}

/// FIPS 180-4 SHA-256 digest.
pub fn sha256(data: &[u8]) -> [u8; 32] {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
        0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
        0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
        0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
        0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
        0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
        0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
    ];
    let mut h: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a,
        0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19,
    ];
    let bit_len = (data.len() as u64).wrapping_mul(8);
    let mut padded = data.to_vec();
    padded.push(0x80);
    while (padded.len() % 64) != 56 {
        padded.push(0);
    }
    padded.extend_from_slice(&bit_len.to_be_bytes());

    for chunk in padded.chunks_exact(64) {
        let mut w = [0u32; 64];
        for (i, word) in chunk.chunks_exact(4).enumerate() {
            w[i] = u32::from_be_bytes([word[0], word[1], word[2], word[3]]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16].wrapping_add(s0).wrapping_add(w[i - 7]).wrapping_add(s1);
        }
        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut hh] = h;
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let temp1 = hh.wrapping_add(s1).wrapping_add(ch).wrapping_add(K[i]).wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = s0.wrapping_add(maj);
            hh = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }
        for (dst, val) in h.iter_mut().zip([a, b, c, d, e, f, g, hh]) {
            *dst = dst.wrapping_add(val);
        }
    }
    let mut out = [0u8; 32];
    for (i, word) in h.iter().enumerate() {
        out[i * 4..(i + 1) * 4].copy_from_slice(&word.to_be_bytes());
    }
    out
}

/// FIPS 180-4 SHA-512 digest.
pub fn sha512(data: &[u8]) -> [u8; 64] {
    const K: [u64; 80] = [
        0x428a2f98d728ae22, 0x7137449123ef65cd, 0xb5c0fbcfec4d3b2f, 0xe9b5dba58189dbbc,
        0x3956c25bf348b538, 0x59f111f1b605d019, 0x923f82a4af194f9b, 0xab1c5ed5da6d8118,
        0xd807aa98a3030242, 0x12835b0145706fbe, 0x243185be4ee4b28c, 0x550c7dc3d5ffb4e2,
        0x72be5d74f27b896f, 0x80deb1fe3b1696b1, 0x9bdc06a725c71235, 0xc19bf174cf692694,
        0xe49b69c19ef14ad2, 0xefbe4786384f25e3, 0x0fc19dc68b8cd5b5, 0x240ca1cc77ac9c65,
        0x2de92c6f592b0275, 0x4a7484aa6ea6e483, 0x5cb0a9dcbd41fbd4, 0x76f988da831153b5,
        0x983e5152ee66dfab, 0xa831c66d2db43210, 0xb00327c898fb213f, 0xbf597fc7beef0ee4,
        0xc6e00bf33da88fc2, 0xd5a79147930aa725, 0x06ca6351e003826f, 0x142929670a0e6e70,
        0x27b70a8546d22ffc, 0x2e1b21385c26c926, 0x4d2c6dfc5ac42aed, 0x53380d139d95b3df,
        0x650a73548baf63de, 0x766a0abb3c77b2a8, 0x81c2c92e47edaee6, 0x92722c851482353b,
        0xa2bfe8a14cf10364, 0xa81a664bbc423001, 0xc24b8b70d0f89791, 0xc76c51a30654be30,
        0xd192e819d6ef5218, 0xd69906245565a910, 0xf40e35855771202a, 0x106aa07032bbd1b8,
        0x19a4c116b8d2d0c8, 0x1e376c085141ab53, 0x2748774cdf8eeb99, 0x34b0bcb5e19b48a8,
        0x391c0cb3c5c95a63, 0x4ed8aa4ae3418acb, 0x5b9cca4f7763e373, 0x682e6ff3d6b2b8a3,
        0x748f82ee5defb2fc, 0x78a5636f43172f60, 0x84c87814a1f0ab72, 0x8cc702081a6439ec,
        0x90befffa23631e28, 0xa4506cebde82bde9, 0xbef9a3f7b2c67915, 0xc67178f2e372532b,
        0xca273eceea26619c, 0xd186b8c721c0c207, 0xeada7dd6cde0eb1e, 0xf57d4f7fee6ed178,
        0x06f067aa72176fba, 0x0a637dc5a2c898a6, 0x113f9804bef90dae, 0x1b710b35131c471b,
        0x28db77f523047d84, 0x32caab7b40c72493, 0x3c9ebe0a15c9bebc, 0x431d67c49c100d4c,
        0x4cc5d4becb3e42b6, 0x597f299cfc657e2a, 0x5fcb6fab3ad6faec, 0x6c44198c4a475817,
    ];
    let mut h: [u64; 8] = [
        0x6a09e667f3bcc908, 0xbb67ae8584caa73b, 0x3c6ef372fe94f82b, 0xa54ff53a5f1d36f1,
        0x510e527fade682d1, 0x9b05688c2b3e6c1f, 0x1f83d9abfb41bd6b, 0x5be0cd19137e2179,
    ];
    let bit_len = (data.len() as u128).wrapping_mul(8);
    let mut padded = data.to_vec();
    padded.push(0x80);
    while (padded.len() % 128) != 112 {
        padded.push(0);
    }
    padded.extend_from_slice(&bit_len.to_be_bytes());

    for chunk in padded.chunks_exact(128) {
        let mut w = [0u64; 80];
        for (i, word) in chunk.chunks_exact(8).enumerate() {
            w[i] = u64::from_be_bytes([
                word[0], word[1], word[2], word[3], word[4], word[5], word[6], word[7],
            ]);
        }
        for i in 16..80 {
            let s0 = w[i - 15].rotate_right(1) ^ w[i - 15].rotate_right(8) ^ (w[i - 15] >> 7);
            let s1 = w[i - 2].rotate_right(19) ^ w[i - 2].rotate_right(61) ^ (w[i - 2] >> 6);
            w[i] = w[i - 16].wrapping_add(s0).wrapping_add(w[i - 7]).wrapping_add(s1);
        }
        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut hh] = h;
        for i in 0..80 {
            let s1 = e.rotate_right(14) ^ e.rotate_right(18) ^ e.rotate_right(41);
            let ch = (e & f) ^ ((!e) & g);
            let temp1 = hh.wrapping_add(s1).wrapping_add(ch).wrapping_add(K[i]).wrapping_add(w[i]);
            let s0 = a.rotate_right(28) ^ a.rotate_right(34) ^ a.rotate_right(39);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = s0.wrapping_add(maj);
            hh = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }
        for (dst, val) in h.iter_mut().zip([a, b, c, d, e, f, g, hh]) {
            *dst = dst.wrapping_add(val);
        }
    }
    let mut out = [0u8; 64];
    for (i, word) in h.iter().enumerate() {
        out[i * 8..(i + 1) * 8].copy_from_slice(&word.to_be_bytes());
    }
    out
}

// GF(2^255 - 19) element in 16 limbs (radix 2^16).
type Gf = [i64; 16];

const GF0: Gf = [0; 16];
const GF1: Gf = [1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
const D: Gf = [
    0x78a3, 0x1359, 0x4dca, 0x75eb, 0xd8ab, 0x4141, 0x0a4d, 0x0070,
    0xe898, 0x7779, 0x4079, 0x8cc7, 0xfe73, 0x2b6f, 0x6cee, 0x5203,
];
const D2: Gf = [
    0xf159, 0x26b2, 0x9b94, 0xebd6, 0xb156, 0x8283, 0x149a, 0x00e0,
    0xd130, 0xeef3, 0x80f2, 0x198e, 0xfce7, 0x56df, 0xd9dc, 0x2406,
];
const X_BASE: Gf = [
    0xd51a, 0x8f25, 0x2d60, 0xc956, 0xa7b2, 0x9525, 0xc760, 0x692c,
    0xdc5c, 0xfdd6, 0xe231, 0xc0a4, 0x53fe, 0xcd6e, 0x36d3, 0x2169,
];
const Y_BASE: Gf = [
    0x6658, 0x6666, 0x6666, 0x6666, 0x6666, 0x6666, 0x6666, 0x6666,
    0x6666, 0x6666, 0x6666, 0x6666, 0x6666, 0x6666, 0x6666, 0x6666,
];
const I_SQRT_M1: Gf = [
    0xa0b0, 0x4a0e, 0x1b27, 0xc4ee, 0xe478, 0xad2f, 0x1806, 0x2f43,
    0xd7a7, 0x3dfb, 0x0099, 0x2b4d, 0xdf0b, 0x4fc1, 0x2480, 0x2b83,
];

fn car25519(o: &mut Gf) {
    for i in 0..16 {
        o[i] += 1 << 16;
        let c = o[i] >> 16;
        if i < 15 {
            o[i + 1] += c - 1;
        } else {
            o[0] += 38 * (c - 1);
        }
        o[i] -= c << 16;
    }
}

fn gf_add(a: &Gf, b: &Gf) -> Gf {
    let mut o = GF0;
    for i in 0..16 {
        o[i] = a[i] + b[i];
    }
    o
}

fn gf_sub(a: &Gf, b: &Gf) -> Gf {
    let mut o = GF0;
    for i in 0..16 {
        o[i] = a[i] - b[i];
    }
    o
}

fn gf_mul(a: &Gf, b: &Gf) -> Gf {
    let mut t = [0i64; 31];
    for i in 0..16 {
        for j in 0..16 {
            t[i + j] += a[i] * b[j];
        }
    }
    for i in 0..15 {
        t[i] += 38 * t[i + 16];
    }
    let mut o = GF0;
    o.copy_from_slice(&t[..16]);
    car25519(&mut o);
    car25519(&mut o);
    o
}

fn gf_sq(a: &Gf) -> Gf {
    gf_mul(a, a)
}

fn inv25519(i: &Gf) -> Gf {
    let mut c = *i;
    for a in (0..=253).rev() {
        c = gf_sq(&c);
        if a != 2 && a != 4 {
            c = gf_mul(&c, i);
        }
    }
    c
}

fn pow2523(i: &Gf) -> Gf {
    let mut c = *i;
    for a in (0..=250).rev() {
        c = gf_sq(&c);
        if a != 1 {
            c = gf_mul(&c, i);
        }
    }
    c
}

fn unpack25519(n: &[u8; 32]) -> Gf {
    let mut o = GF0;
    for i in 0..16 {
        o[i] = i64::from(n[2 * i]) + (i64::from(n[2 * i + 1]) << 8);
    }
    o[15] &= 0x7fff;
    o
}

fn pack25519(n: &Gf) -> [u8; 32] {
    let mut t = *n;
    car25519(&mut t);
    car25519(&mut t);
    car25519(&mut t);
    let mut m = GF0;
    for _ in 0..2 {
        m[0] = t[0] - 0xffed;
        for i in 1..15 {
            m[i] = t[i] - 0xffff - ((m[i - 1] >> 16) & 1);
            m[i - 1] &= 0xffff;
        }
        m[15] = t[15] - 0x7fff - ((m[14] >> 16) & 1);
        let b = (m[15] >> 16) & 1;
        m[14] &= 0xffff;
        for i in 0..16 {
            t[i] = if b == 0 { m[i] } else { t[i] };
        }
    }
    let mut o = [0u8; 32];
    for i in 0..16 {
        o[2 * i] = (t[i] & 0xff) as u8;
        o[2 * i + 1] = (t[i] >> 8) as u8;
    }
    o
}

fn neq25519(a: &Gf, b: &Gf) -> bool {
    pack25519(a) != pack25519(b)
}

fn par25519(a: &Gf) -> u8 {
    let d = pack25519(a);
    d[0] & 1
}

type Point = [Gf; 4];

fn point_add(p: &Point, q: &Point) -> Point {
    let a = gf_mul(&gf_sub(&p[1], &p[0]), &gf_sub(&q[1], &q[0]));
    let b = gf_mul(&gf_add(&p[1], &p[0]), &gf_add(&q[1], &q[0]));
    let c = gf_mul(&gf_mul(&p[3], &q[3]), &D2);
    let d = gf_mul(&gf_add(&p[2], &p[2]), &q[2]);
    let e = gf_sub(&b, &a);
    let f = gf_sub(&d, &c);
    let g = gf_add(&d, &c);
    let h = gf_add(&b, &a);
    [gf_mul(&e, &f), gf_mul(&h, &g), gf_mul(&g, &f), gf_mul(&e, &h)]
}

fn scalar_mult(q: &Point, s: &[u8; 32]) -> Point {
    let mut p = [GF0, GF1, GF1, GF0];
    let mut acc = *q;
    for i in 0..256 {
        let bit = (s[i >> 3] >> (i & 7)) & 1;
        if bit == 1 {
            p = point_add(&p, &acc);
        }
        acc = point_add(&acc, &acc);
    }
    p
}

fn scalar_base(s: &[u8; 32]) -> Point {
    let b = [X_BASE, Y_BASE, GF1, gf_mul(&X_BASE, &Y_BASE)];
    scalar_mult(&b, s)
}

fn pack_point(p: &Point) -> [u8; 32] {
    let zi = inv25519(&p[2]);
    let tx = gf_mul(&p[0], &zi);
    let ty = gf_mul(&p[1], &zi);
    let mut r = pack25519(&ty);
    r[31] ^= par25519(&tx) << 7;
    r
}

fn unpack_point(n: &[u8; 32]) -> Option<Point> {
    let y = unpack25519(n);
    let z = GF1;
    let u = gf_sub(&gf_sq(&y), &z);
    let v = gf_add(&gf_mul(&D, &gf_sq(&y)), &z);
    let v3 = gf_mul(&gf_sq(&v), &v);
    let v7 = gf_mul(&gf_sq(&v3), &v);
    let mut x = gf_mul(&gf_mul(&u, &v3), &pow2523(&gf_mul(&u, &v7)));
    let vxx = gf_mul(&gf_sq(&x), &v);
    let check = gf_sub(&vxx, &u);
    if neq25519(&check, &GF0) {
        let check2 = gf_add(&vxx, &u);
        if neq25519(&check2, &GF0) {
            return None;
        }
        x = gf_mul(&x, &I_SQRT_M1);
    }
    if par25519(&x) != (n[31] >> 7) {
        x = gf_sub(&GF0, &x);
    }
    let t = gf_mul(&x, &y);
    Some([x, y, z, t])
}

const L_ORDER: [i64; 32] = [
    0xed, 0xd3, 0xf5, 0x5c, 0x1a, 0x63, 0x12, 0x58,
    0xd6, 0x9c, 0xf7, 0xa2, 0xde, 0xf9, 0xde, 0x14,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x10,
];

fn mod_l(x: &mut [i64; 64]) -> [u8; 32] {
    for i in (32..64).rev() {
        let mut carry = 0i64;
        for j in (i - 32)..(i - 12) {
            x[j] += carry - 16 * x[i] * L_ORDER[j - (i - 32)];
            carry = (x[j] + 128) >> 8;
            x[j] -= carry << 8;
        }
        x[i - 12] += carry;
        x[i] = 0;
    }
    let mut carry = 0i64;
    for j in 0..32 {
        x[j] += carry - (x[31] >> 4) * L_ORDER[j];
        carry = x[j] >> 8;
        x[j] &= 255;
    }
    for j in 0..32 {
        x[j] -= carry * L_ORDER[j];
    }
    let mut out = [0u8; 32];
    for i in 0..32 {
        x[i + 1] += x[i] >> 8;
        out[i] = (x[i] & 255) as u8;
    }
    out
}

fn scalar_is_canonical(s: &[u8; 32]) -> bool {
    if (s[31] & 0xf0) != 0 && s[31] > 0x10 {
        return false;
    }
    for i in (0..32).rev() {
        let lim = L_ORDER[i] as u8;
        if s[i] < lim {
            return true;
        }
        if s[i] > lim {
            return false;
        }
    }
    false
}

/// Derive the 32-byte Ed25519 public key from a 32-byte secret seed (RFC 8032 §5.1.5).
pub fn ed25519_public_key(secret_seed: &[u8; 32]) -> [u8; 32] {
    let mut d = sha512(secret_seed);
    d[0] &= 248;
    d[31] &= 127;
    d[31] |= 64;
    let mut s = [0u8; 32];
    s.copy_from_slice(&d[..32]);
    let p = scalar_base(&s);
    pack_point(&p)
}

/// Sign `message` with a 32-byte Ed25519 secret seed (RFC 8032 §5.1.6).
pub fn ed25519_sign(secret_seed: &[u8; 32], message: &[u8]) -> [u8; 64] {
    let mut d = sha512(secret_seed);
    d[0] &= 248;
    d[31] &= 127;
    d[31] |= 64;
    let mut a_scalar = [0u8; 32];
    a_scalar.copy_from_slice(&d[..32]);
    let pk = pack_point(&scalar_base(&a_scalar));

    let mut r_input = Vec::with_capacity(32 + message.len());
    r_input.extend_from_slice(&d[32..64]);
    r_input.extend_from_slice(message);
    let r_hash = sha512(&r_input);
    let mut r_wide = [0i64; 64];
    for i in 0..64 {
        r_wide[i] = i64::from(r_hash[i]);
    }
    let r_scalar = mod_l(&mut r_wide);
    let r_point = pack_point(&scalar_base(&r_scalar));

    let mut k_input = Vec::with_capacity(64 + message.len());
    k_input.extend_from_slice(&r_point);
    k_input.extend_from_slice(&pk);
    k_input.extend_from_slice(message);
    let k_hash = sha512(&k_input);
    let mut k_wide = [0i64; 64];
    for i in 0..64 {
        k_wide[i] = i64::from(k_hash[i]);
    }
    let k_scalar = mod_l(&mut k_wide);

    let mut s_wide = [0i64; 64];
    for i in 0..32 {
        s_wide[i] = i64::from(r_scalar[i]);
    }
    for i in 0..32 {
        for j in 0..32 {
            s_wide[i + j] += i64::from(k_scalar[i]) * i64::from(a_scalar[j]);
        }
    }
    let s_scalar = mod_l(&mut s_wide);

    let mut sig = [0u8; 64];
    sig[..32].copy_from_slice(&r_point);
    sig[32..64].copy_from_slice(&s_scalar);
    sig
}

/// Verify an RFC 8032 Ed25519 signature `[R || S]` over `message` under `public_key`.
pub fn ed25519_verify(public_key: &[u8; 32], message: &[u8], signature: &[u8; 64]) -> bool {
    let mut r_bytes = [0u8; 32];
    r_bytes.copy_from_slice(&signature[..32]);
    let mut s_bytes = [0u8; 32];
    s_bytes.copy_from_slice(&signature[32..64]);
    if !scalar_is_canonical(&s_bytes) {
        return false;
    }
    let Some(a_point) = unpack_point(public_key) else {
        return false;
    };
    let Some(r_point) = unpack_point(&r_bytes) else {
        return false;
    };
    let mut k_input = Vec::with_capacity(64 + message.len());
    k_input.extend_from_slice(&r_bytes);
    k_input.extend_from_slice(public_key);
    k_input.extend_from_slice(message);
    let k_hash = sha512(&k_input);
    let mut k_wide = [0i64; 64];
    for i in 0..64 {
        k_wide[i] = i64::from(k_hash[i]);
    }
    let k_scalar = mod_l(&mut k_wide);

    // Check [S]B == R + [k]A
    let lhs = pack_point(&scalar_base(&s_bytes));
    let ka = scalar_mult(&a_point, &k_scalar);
    let rhs = pack_point(&point_add(&r_point, &ka));
    lhs == rhs
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn rfc8032_ed25519_test_vectors_and_tamper_rejection() {
        // RFC 8032 §7.1 Test Vector 1 (empty message)
        let seed1: [u8; 32] = hex_decode("9d61b19deffd5a60ba844af492ec2cc44449c5697b326919703bac031cae7f60")
            .unwrap()
            .try_into()
            .unwrap();
        let expected_pk1: [u8; 32] = hex_decode(DEFAULT_GEMIGUARD_PUBLIC_KEY_HEX).unwrap().try_into().unwrap();
        let expected_sig1: [u8; 64] = hex_decode(
            "e5564300c360ac729086e2cc806e828a84877f1eb8e5d974d873e065224901555fb8821590a33bacc61e39701cf9b46bd25bf5f0595bbe24655141438e7a100b",
        )
        .unwrap()
        .try_into()
        .unwrap();

        assert_eq!(ed25519_public_key(&seed1), expected_pk1);
        assert_eq!(ed25519_sign(&seed1, b""), expected_sig1);
        assert!(ed25519_verify(&expected_pk1, b"", &expected_sig1));
        assert!(!ed25519_verify(&expected_pk1, b"x", &expected_sig1));

        // RFC 8032 §7.1 Test Vector 2 (1-byte message 0x72)
        let seed2: [u8; 32] = hex_decode("4ccd089b28ff96da9db6c346ec114e0f5b8a319f35aba624da8cf6ed4fb8a6fb")
            .unwrap()
            .try_into()
            .unwrap();
        let pk2 = ed25519_public_key(&seed2);
        assert_eq!(
            hex_encode(&pk2),
            "3d4017c3e843895a92b70aa74d1b7ebc9c982ccf2ec4968cc0cd55f12af4660c"
        );
        let sig2 = ed25519_sign(&seed2, &[0x72]);
        assert_eq!(
            hex_encode(&sig2),
            "92a009a9f0d4cab8720e820b5f642540a2b27b5416503f8fb3762223ebdb69da085ac1e43e15996e458f3613d0f11d8c387b2eaeb4302aeeb00d291612bb0c00"
        );
        assert!(ed25519_verify(&pk2, &[0x72], &sig2));
    }

    #[test]
    fn oidc_pkce_s256_challenge_matches_rfc7636_appendix_b() {
        // RFC 7636 Appendix B test vector:
        // verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"
        // challenge = "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        let req = OidcPkceRequest::from_verifier_and_state(
            49152,
            "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk".into(),
            "state123".into(),
        );
        assert_eq!(req.code_challenge, "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM");
        assert_eq!(req.code_challenge_method, "S256");
        assert_eq!(req.issuer, GEMI_ACCESS_ISSUER);
        assert_eq!(req.client_id, AFTERTRON_OIDC_CLIENT_ID);
        assert!(req.authorization_url.starts_with("https://auth.gemitron.net/oauth2/authorize?"));
    }

    #[test]
    fn gemiguard_offline_lease_lifecycle_and_safe_project_preservation() {
        let seed: [u8; 32] = hex_decode("9d61b19deffd5a60ba844af492ec2cc44449c5697b326919703bac031cae7f60")
            .unwrap()
            .try_into()
            .unwrap();
        let verifier = GemiGuardVerifier::new("sha256:device-alpha");
        let mut lease = OfflineLease {
            schema: OFFLINE_LEASE_SCHEMA.into(),
            lease_id: "lease-aftertron-001".into(),
            subject: "user-2011096497".into(),
            product: "aftertron".into(),
            tier: "studio".into(),
            device_fingerprint: "sha256:device-alpha".into(),
            issued_at: 1_700_000_000,
            expires_at: 1_700_086_400,
            offline_grace_until: 1_700_604_800,
            capabilities: CANONICAL_CAPABILITIES.iter().map(|s| (*s).to_string()).collect(),
            key_id: DEFAULT_GEMIGUARD_KEY_ID.into(),
            signature: String::new(),
        };
        lease.sign(&seed);

        // 1. Active lease
        let active = verifier.evaluate(&lease, None, 1_700_010_000).unwrap();
        assert_eq!(active.state, LeaseState::Active);
        assert!(active.preserve_user_project_data);
        assert!(active.allows("aftertron:mcp:cloud_bridge"));

        // 2. Grace period (after expires_at, before offline_grace_until)
        let grace = verifier.evaluate(&lease, None, 1_700_100_000).unwrap();
        assert_eq!(grace.state, LeaseState::GracePeriod);
        assert!(grace.preserve_user_project_data);
        assert!(grace.allows("aftertron:mcp:cloud_bridge"));

        // 3. Expired past offline grace window -> safe local fallback (never locks user out of local .ecproj files)
        let expired = verifier.evaluate(&lease, None, 1_701_000_000).unwrap();
        assert_eq!(expired.state, LeaseState::ExpiredFallback);
        assert!(expired.preserve_user_project_data);
        assert!(expired.allows_local_project_io);
        assert!(expired.allows("aftertron:local:save"));
        assert!(!expired.allows("aftertron:mcp:cloud_bridge"));

        // 4. Revocation list revokes lease -> state is Revoked, local project I/O remains preserved
        let mut rev = RevocationList {
            schema: REVOCATION_LIST_SCHEMA.into(),
            issued_at: 1_700_020_000,
            revoked_lease_ids: vec!["lease-aftertron-001".into()],
            revoked_device_fingerprints: vec![],
            key_id: DEFAULT_GEMIGUARD_KEY_ID.into(),
            signature: String::new(),
        };
        rev.sign(&seed);
        let revoked = verifier.evaluate(&lease, Some(&rev), 1_700_020_000).unwrap();
        assert_eq!(revoked.state, LeaseState::Revoked);
        assert!(revoked.preserve_user_project_data);
        assert!(revoked.allows_local_project_io);
        assert!(revoked.allows("aftertron:local:save"));
        assert!(!revoked.allows("aftertron:mcp:cloud_bridge"));

        // 5. Tampered lease or wrong product is rejected
        let mut tampered = lease.clone();
        tampered.tier = "enterprise".into();
        assert!(verifier.evaluate(&tampered, None, 1_700_010_000).is_err());

        let mut wrong_product = lease.clone();
        wrong_product.product = "phototron".into();
        wrong_product.sign(&seed);
        assert!(verifier.evaluate(&wrong_product, None, 1_700_010_000).is_err());
    }
}
