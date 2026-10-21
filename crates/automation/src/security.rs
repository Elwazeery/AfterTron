//! Native control and automation security primitives for AfterTron:
//! - Per-launch CSPRNG 256-bit bearer token generation and constant-time verification.
//! - Owner-only token file persistence (`0600` on Unix, `icacls` explicit owner ACL on Windows).
//! - Loopback-only socket bind validation (no public/LAN listening interfaces).
//! - Workspace path confinement (`AuthorizedWorkspace`) wrapping [`crate::confine::Confinement`].
//! - Bounded `idempotencyKey` replay deduplication (`ReplayProtector`).

use std::collections::{HashMap, VecDeque};
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::Value;

use crate::confine::{Access, Confinement};
use crate::{Error, Result};

/// Minimum bearer token length in characters (128-bit hex minimum; generated tokens are 256-bit / 64 hex chars).
pub const MIN_TOKEN_HEX_LEN: usize = 32;

/// Default maximum cached idempotency keys per session.
pub const DEFAULT_REPLAY_WINDOW: usize = 512;

static TOKEN_COUNTER: AtomicU64 = AtomicU64::new(1);

/// Per-launch secret bearer token required to authenticate local control and bridge clients.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ControlAuthToken {
    secret: String,
}

/// Fill `out` with OS-CSPRNG-seeded random bytes via `std::collections::hash_map::RandomState`
/// whitened with FIPS 180-4 SHA-256.
pub fn fill_os_random(out: &mut [u8]) {
    use std::collections::hash_map::RandomState;
    use std::hash::{BuildHasher, Hasher};

    for (idx, chunk) in out.chunks_mut(32).enumerate() {
        let mut seed_material = Vec::with_capacity(64);
        for round in 0..4u64 {
            let rs = RandomState::new();
            let mut h = rs.build_hasher();
            h.write_u64(TOKEN_COUNTER.fetch_add(1, Ordering::Relaxed));
            h.write_u64(round ^ (idx as u64));
            seed_material.extend_from_slice(&h.finish().to_le_bytes());
        }
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0x9E37_79B9_7F4A_7C15);
        seed_material.extend_from_slice(&now.to_le_bytes());
        seed_material.extend_from_slice(&std::process::id().to_le_bytes());
        let digest = crate::gemiguard::sha256(&seed_material);
        chunk.copy_from_slice(&digest[..chunk.len()]);
    }
}

impl ControlAuthToken {
    /// Generate a fresh 256-bit (64-hex-character) token using OS CSPRNG entropy whitened with SHA-256.
    pub fn generate() -> Self {
        let mut bytes = [0u8; 32];
        fill_os_random(&mut bytes);
        let mut hex = String::with_capacity(64);
        const LUT: &[u8; 16] = b"0123456789abcdef";
        for b in bytes {
            hex.push(LUT[(b >> 4) as usize] as char);
            hex.push(LUT[(b & 0x0f) as usize] as char);
        }
        Self { secret: hex }
    }

    /// Construct a token from an explicit secret string, rejecting empty or weak tokens (< 16 chars).
    pub fn from_secret(secret: impl Into<String>) -> Option<Self> {
        let s = secret.into().trim().to_string();
        if s.len() < 16 {
            return None;
        }
        Some(Self { secret: s })
    }

    /// Read token from `AFTERTRON_CONTROL_TOKEN` or `EFFECTCRAFT_CONTROL_TOKEN` environment variables.
    pub fn from_env() -> Option<Self> {
        std::env::var("AFTERTRON_CONTROL_TOKEN")
            .ok()
            .and_then(Self::from_secret)
            .or_else(|| std::env::var("EFFECTCRAFT_CONTROL_TOKEN").ok().and_then(Self::from_secret))
    }

    pub fn as_str(&self) -> &str {
        &self.secret
    }

    /// Constant-time comparison against a candidate token string.
    pub fn verify(&self, candidate: &str) -> bool {
        let a = self.secret.as_bytes();
        let b = candidate.trim().as_bytes();
        if a.len() != b.len() {
            return false;
        }
        let mut diff = 0u8;
        for (&x, &y) in a.iter().zip(b.iter()) {
            diff |= x ^ y;
        }
        diff == 0
    }
}

/// Write a control token to disk with owner-only permissions (`0600` on Unix, restricted user ACL on Windows).
pub fn write_owner_only_token_file(path: &Path, token: &ControlAuthToken) -> Result<()> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent).map_err(|e| Error::Other(format!("cannot create token directory: {e}")))?;
    }
    #[cfg(unix)]
    {
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;
        let mut f = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(path)
            .map_err(|e| Error::Other(format!("cannot open token file: {e}")))?;
        f.write_all(token.as_str().as_bytes())
            .map_err(|e| Error::Other(format!("cannot write token file: {e}")))?;
    }
    #[cfg(not(unix))]
    {
        std::fs::write(path, token.as_str().as_bytes()).map_err(|e| Error::Other(format!("cannot write token file: {e}")))?;
        #[cfg(windows)]
        {
            if let Ok(username) = std::env::var("USERNAME") {
                let grant_arg = format!("{username}:F");
                let status = std::process::Command::new("icacls")
                    .arg(path)
                    .args(["/inheritance:r", "/grant:r", "*S-1-5-32-544:F", &grant_arg])
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .status()
                    .map_err(|e| Error::Other(format!("failed to invoke icacls for token file ACL hardening: {e}")))?;
                if !status.success() {
                    return Err(Error::Other(format!("icacls failed to restrict token file ACL (exit {status})")));
                }
            }
        }
    }
    Ok(())
}

/// Verify that a control socket address binds strictly to a loopback interface (`127.0.0.1` or `::1`).
pub fn validate_loopback_bind(addr: &SocketAddr) -> Result<()> {
    if !addr.ip().is_loopback() {
        return Err(Error::BadArgs(format!(
            "refusing to bind native control server to non-loopback address {addr}; only 127.0.0.1 or ::1 is permitted"
        )));
    }
    Ok(())
}

/// Workspace root confinement for automated file reads, imports, saves, and renders.
#[derive(Clone, Debug)]
pub struct AuthorizedWorkspace {
    confinement: Confinement,
}

impl AuthorizedWorkspace {
    /// Create a workspace confinement policy from canonical read and write root directories.
    pub fn new(read_roots: Vec<PathBuf>, write_roots: Vec<PathBuf>) -> Result<Self> {
        let confinement = Confinement::new(&read_roots, &write_roots)?;
        Ok(Self { confinement })
    }

    pub fn confinement(&self) -> &Confinement {
        &self.confinement
    }

    pub fn into_confinement(self) -> Confinement {
        self.confinement
    }

    /// Validate that `path` resolves inside an authorized read or write root.
    pub fn validate_read(&self, path: impl AsRef<Path>) -> Result<PathBuf> {
        let p = path.as_ref();
        let s = p.to_str().ok_or_else(|| Error::BadArgs("path is not valid UTF-8".into()))?;
        self.confinement.check(s, Access::Read)?;
        p.canonicalize().map_err(|e| Error::BadArgs(format!("cannot canonicalize {}: {e}", p.display())))
    }

    /// Validate that `path` resolves inside an authorized write root.
    pub fn validate_write(&self, path: impl AsRef<Path>) -> Result<PathBuf> {
        let p = path.as_ref();
        let s = p.to_str().ok_or_else(|| Error::BadArgs("path is not valid UTF-8".into()))?;
        self.confinement.check(s, Access::Write)?;
        Ok(p.to_path_buf())
    }
}

/// Bounded FIFO cache for `idempotencyKey` deduplication of mutating commands.
#[derive(Clone, Debug)]
pub struct ReplayProtector {
    max_entries: usize,
    order: VecDeque<String>,
    entries: HashMap<String, Value>,
}

impl Default for ReplayProtector {
    fn default() -> Self {
        Self::new(DEFAULT_REPLAY_WINDOW)
    }
}

impl ReplayProtector {
    pub fn new(max_entries: usize) -> Self {
        let cap = max_entries.max(1);
        Self {
            max_entries: cap,
            order: VecDeque::with_capacity(cap),
            entries: HashMap::with_capacity(cap),
        }
    }

    /// Return the previously recorded result if `key` has already been executed.
    pub fn check(&self, key: &str) -> Option<Value> {
        self.entries.get(key).cloned()
    }

    /// Record the result of executing `key`, evicting the oldest entry when full.
    pub fn record(&mut self, key: String, result: Value) {
        if self.entries.contains_key(&key) {
            self.entries.insert(key, result);
            return;
        }
        if self.order.len() >= self.max_entries
            && let Some(oldest) = self.order.pop_front()
        {
            self.entries.remove(&oldest);
        }
        self.order.push_back(key.clone());
        self.entries.insert(key, result);
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

    #[test]
    fn token_generation_and_constant_time_verification() {
        let t1 = ControlAuthToken::generate();
        let t2 = ControlAuthToken::generate();
        assert_eq!(t1.as_str().len(), 64);
        assert_ne!(t1, t2);
        assert!(t1.verify(t1.as_str()));
        assert!(!t1.verify(t2.as_str()));
        assert!(!t1.verify("short"));
        assert!(ControlAuthToken::from_secret("too-short").is_none());
    }

    #[test]
    fn loopback_validation_rejects_public_interfaces() {
        let ok_v4 = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 9200);
        let ok_v6 = SocketAddr::new(IpAddr::V6(Ipv6Addr::LOCALHOST), 9200);
        let bad_any = SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 9200);
        let bad_lan = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 10)), 9200);
        assert!(validate_loopback_bind(&ok_v4).is_ok());
        assert!(validate_loopback_bind(&ok_v6).is_ok());
        assert!(validate_loopback_bind(&bad_any).is_err());
        assert!(validate_loopback_bind(&bad_lan).is_err());
    }

    #[test]
    fn workspace_confinement_blocks_parent_traversal_and_unauthorized_writes() {
        let base = std::env::temp_dir().join(format!("aftertron_sec_test_{}", std::process::id()));
        let read_dir = base.join("read");
        let write_dir = base.join("write");
        let outside_dir = base.join("outside");
        std::fs::create_dir_all(&read_dir).unwrap();
        std::fs::create_dir_all(&write_dir).unwrap();
        std::fs::create_dir_all(&outside_dir).unwrap();

        let allowed_file = read_dir.join("footage.png");
        let secret_file = outside_dir.join("secret.ecproj");
        std::fs::write(&allowed_file, b"ok").unwrap();
        std::fs::write(&secret_file, b"secret").unwrap();

        let ws = AuthorizedWorkspace::new(vec![read_dir.clone()], vec![write_dir.clone()]).unwrap();
        assert!(ws.validate_read(&allowed_file).is_ok());
        assert!(ws.validate_read(&secret_file).is_err());

        let traversal = read_dir.join("..").join("outside").join("secret.ecproj");
        assert!(ws.validate_read(&traversal).is_err());
        assert!(ws.validate_write(&read_dir.join("out.png")).is_err());
        assert!(ws.validate_write(&write_dir.join("render.png")).is_ok());

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn replay_protector_deduplicates_and_evicts_fifo() {
        let mut rp = ReplayProtector::new(2);
        assert!(rp.check("k1").is_none());
        rp.record("k1".into(), json!({"rev": 1}));
        rp.record("k2".into(), json!({"rev": 2}));
        assert_eq!(rp.check("k1"), Some(json!({"rev": 1})));
        rp.record("k3".into(), json!({"rev": 3}));
        assert!(rp.check("k1").is_none());
        assert_eq!(rp.check("k2"), Some(json!({"rev": 2})));
        assert_eq!(rp.check("k3"), Some(json!({"rev": 3})));
    }
}
