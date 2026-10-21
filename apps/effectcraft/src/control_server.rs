//! Local TCP control server (`effectcraft --control 9877`).
//!
//! Newline-delimited JSON on loopback:
//!   request:  `{"id": 1, "method": "engine.execute", "params": {"command": "comp.new", "params": {}}}`
//!   response: `{"id": 1, "ok": true, "result": ...}` or `{"id": 1, "ok": false, "error": "..."}`
//!
//! Security hardening (`AFTER-010` / `AFTER-003`):
//! - Loopback-only bind (`127.0.0.1` or `::1`) verified by [`validate_loopback_bind`].
//! - Mandatory per-launch 256-bit bearer token authentication (`method: "auth"`) within
//!   [`AUTH_DEADLINE`] (10 s) before any command is dispatched to the UI thread.
//! - Bounded connection slots (`MAX_CONNECTIONS`) guarded by RAII [`ConnectionSlot`] so permits
//!   are released under normal returns, thread spawn failures, and panic unwinds.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream, ToSocketAddrs};
use std::path::Path;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
    mpsc::{self, Receiver, Sender},
};
use std::time::Duration;

use effectcraft_automation::security::{ControlAuthToken, validate_loopback_bind, write_owner_only_token_file};
use effectcraft_ui_egui::control::ControlRequest;
use serde_json::{Value, json};

/// Maximum size of one JSON request line (256 KiB).
const MAX_LINE: u64 = 256 * 1024;
/// Simultaneous client connections.
const MAX_CONNECTIONS: usize = 8;
/// Maximum time allowed for a new connection to complete the `auth` handshake.
const AUTH_DEADLINE: Duration = Duration::from_secs(10);
/// Idle read timeout after authentication.
const READ_TIMEOUT: Duration = Duration::from_secs(60);
/// Time to wait for the UI thread to process a request.
const REPLY_TIMEOUT: Duration = Duration::from_secs(60);

struct ConnectionSlot {
    active: Arc<AtomicUsize>,
}

impl ConnectionSlot {
    fn acquire(active: &Arc<AtomicUsize>) -> Option<Self> {
        active
            .try_update(Ordering::AcqRel, Ordering::Acquire, |n| (n < MAX_CONNECTIONS).then_some(n + 1))
            .ok()
            .map(|_| Self { active: Arc::clone(active) })
    }
}

impl Drop for ConnectionSlot {
    fn drop(&mut self) {
        self.active.fetch_sub(1, Ordering::AcqRel);
    }
}

pub struct ControlServer {
    pub rx: Receiver<ControlRequest>,
    pub addr: SocketAddr,
    #[allow(dead_code)]
    pub token: ControlAuthToken,
    wake: Arc<Mutex<Option<egui::Context>>>,
}

impl ControlServer {
    /// Start listening on `addr_str` (`"9877"` -> `127.0.0.1:9877`, or `"127.0.0.1:9877"`).
    /// Refuse non-loopback addresses so the control port is never exposed to the LAN.
    #[allow(dead_code)]
    pub fn spawn(addr_str: &str) -> Result<Self, String> {
        let token = ControlAuthToken::from_env().unwrap_or_else(ControlAuthToken::generate);
        Self::spawn_with_token(addr_str, token, None)
    }

    /// Start listening on `addr_str` with an explicit [`ControlAuthToken`] and optional owner-only
    /// token output file (`--control-token-out <FILE>`).
    pub fn spawn_with_token(addr_str: &str, token: ControlAuthToken, token_out: Option<&Path>) -> Result<Self, String> {
        let addr_str = if addr_str.chars().all(|c| c.is_ascii_digit()) {
            format!("127.0.0.1:{addr_str}")
        } else {
            addr_str.to_string()
        };
        let addr: SocketAddr = addr_str
            .to_socket_addrs()
            .map_err(|e| format!("invalid control address `{addr_str}`: {e}"))?
            .next()
            .ok_or_else(|| format!("could not resolve `{addr_str}`"))?;
        validate_loopback_bind(&addr).map_err(|e| e.to_string())?;
        if let Some(path) = token_out {
            write_owner_only_token_file(path, &token).map_err(|e| e.to_string())?;
        }
        let listener = TcpListener::bind(addr).map_err(|e| format!("bind {addr}: {e}"))?;
        let local = listener.local_addr().unwrap_or(addr);
        let (tx, rx) = mpsc::channel();
        let wake: Arc<Mutex<Option<egui::Context>>> = Arc::new(Mutex::new(None));
        let wake2 = wake.clone();
        let active = Arc::new(AtomicUsize::new(0));
        let server_token = token.clone();
        let _ = std::thread::Builder::new().name("aftertron-control".into()).spawn(move || {
            for stream in listener.incoming().flatten() {
                let Some(slot) = ConnectionSlot::acquire(&active) else {
                    drop(stream); // busy
                    continue;
                };
                let tx = tx.clone();
                let wake = wake2.clone();
                let conn_token = server_token.clone();
                let _ = std::thread::Builder::new().name("aftertron-control-client".into()).spawn(move || {
                    let _slot = slot;
                    handle_client(stream, tx, wake, conn_token);
                });
            }
        });
        log::info!("AfterTron control server listening on {local} (auth required)");
        Ok(Self { rx, addr: local, token, wake })
    }

    pub fn set_ctx(&self, ctx: &egui::Context) {
        if let Ok(mut g) = self.wake.lock()
            && g.is_none()
        {
            *g = Some(ctx.clone());
        }
    }
}

/// Start listening on `127.0.0.1:port` and return the request receiver for [`effectcraft_ui_egui::EffectcraftApp::with_control`].
#[allow(dead_code)]
pub fn start(port: u16, ctx: egui::Context) -> Receiver<ControlRequest> {
    start_with_token_file(port, ctx, None)
}

/// Start listening on `127.0.0.1:port`, optionally writing the per-launch token to `token_out` with owner-only permissions.
pub fn start_with_token_file(port: u16, ctx: egui::Context, token_out: Option<&Path>) -> Receiver<ControlRequest> {
    let token = ControlAuthToken::from_env().unwrap_or_else(ControlAuthToken::generate);
    match ControlServer::spawn_with_token(&format!("127.0.0.1:{port}"), token, token_out) {
        Ok(srv) => {
            srv.set_ctx(&ctx);
            srv.rx
        }
        Err(e) => {
            log::error!("control server failed on 127.0.0.1:{port}: {e}");
            let (_tx, rx) = mpsc::channel();
            rx
        }
    }
}

fn handle_client(
    stream: TcpStream,
    tx: Sender<ControlRequest>,
    wake: Arc<Mutex<Option<egui::Context>>>,
    expected_token: ControlAuthToken,
) {
    let _ = stream.set_nodelay(true);
    let _ = stream.set_read_timeout(Some(AUTH_DEADLINE));
    let Ok(mut writer) = stream.try_clone() else { return };
    let mut reader = BufReader::new(stream);
    let mut authenticated = false;

    loop {
        let mut line = String::new();
        match (&mut reader).take(MAX_LINE + 1).read_line(&mut line) {
            Ok(0) | Err(_) => break,
            Ok(n) if n as u64 > MAX_LINE => {
                let _ = writeln!(writer, "{}", json!({"ok": false, "error": "request line exceeds 256 KiB"}));
                break;
            }
            Ok(_) => {}
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let msg: Value = match serde_json::from_str(trimmed) {
            Ok(v) => v,
            Err(e) => {
                let _ = writeln!(writer, "{}", json!({"ok": false, "error": format!("invalid JSON: {e}")}));
                if !authenticated {
                    break;
                }
                continue;
            }
        };
        let id = msg.get("id").cloned().unwrap_or(Value::Null);
        let Some(method) = msg.get("method").and_then(Value::as_str).map(str::to_string) else {
            let _ = writeln!(writer, "{}", json!({"id": id, "ok": false, "error": "missing `method`"}));
            if !authenticated {
                break;
            }
            continue;
        };
        let params = msg.get("params").cloned().unwrap_or(json!({}));

        // Allow explicit `auth` handshake or inline bearer token on the request.
        if method == "auth" {
            let candidate = params
                .get("token")
                .or_else(|| msg.get("token"))
                .and_then(Value::as_str)
                .unwrap_or("");
            if expected_token.verify(candidate) {
                authenticated = true;
                let _ = reader.get_ref().set_read_timeout(Some(READ_TIMEOUT));
                let resp = json!({"id": id, "ok": true, "result": {"authenticated": true, "product": "AfterTron"}});
                if writeln!(writer, "{resp}").is_err() || writer.flush().is_err() {
                    break;
                }
                continue;
            } else {
                let resp = json!({"id": id, "ok": false, "error": "unauthorized: invalid control token"});
                let _ = writeln!(writer, "{resp}");
                let _ = writer.flush();
                break;
            }
        }

        if !authenticated {
            let inline_token = msg
                .get("token")
                .or_else(|| params.get("token"))
                .and_then(Value::as_str)
                .unwrap_or("");
            if expected_token.verify(inline_token) {
                authenticated = true;
                let _ = reader.get_ref().set_read_timeout(Some(READ_TIMEOUT));
            } else {
                let resp = json!({
                    "id": id,
                    "ok": false,
                    "error": "unauthorized: authentication required before dispatching commands"
                });
                let _ = writeln!(writer, "{resp}");
                let _ = writer.flush();
                break;
            }
        }

        let (reply_tx, reply_rx) = mpsc::channel();
        if tx.send(ControlRequest { method, params, reply: reply_tx }).is_err() {
            break;
        }
        if let Ok(g) = wake.lock()
            && let Some(ctx) = g.as_ref()
        {
            ctx.request_repaint();
        }
        let mut resp = match reply_rx.recv_timeout(REPLY_TIMEOUT) {
            Ok(v) => v,
            Err(_) => json!({"ok": false, "error": "timed out waiting for UI thread"}),
        };
        if let Some(o) = resp.as_object_mut() {
            o.insert("id".into(), id);
        }
        if writeln!(writer, "{resp}").is_err() || writer.flush().is_err() {
            break;
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    fn spawn_slot_task<F>(active: &Arc<AtomicUsize>, spawn: F) -> Result<(), std::io::Error>
    where
        F: FnOnce(ConnectionSlot) -> Result<(), std::io::Error>,
    {
        let Some(slot) = ConnectionSlot::acquire(active) else {
            return Err(std::io::Error::other("busy"));
        };
        spawn(slot)
    }

    #[test]
    fn connection_slot_releases_on_drop_spawn_error_and_panic() {
        let active = Arc::new(AtomicUsize::new(0));
        let mut slots = Vec::new();
        for _ in 0..MAX_CONNECTIONS {
            slots.push(ConnectionSlot::acquire(&active).expect("slot should be available"));
        }
        assert_eq!(active.load(Ordering::Acquire), MAX_CONNECTIONS);
        assert!(ConnectionSlot::acquire(&active).is_none());

        drop(slots.pop());
        assert_eq!(active.load(Ordering::Acquire), MAX_CONNECTIONS - 1);

        let err = spawn_slot_task(&active, |_slot| Err(std::io::Error::other("spawn failed")));
        assert!(err.is_err());
        assert_eq!(active.load(Ordering::Acquire), MAX_CONNECTIONS - 1);

        let panicked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _slot = ConnectionSlot::acquire(&active).expect("slot after failed spawn");
            std::panic::resume_unwind(Box::new("handler panic"));
        }));
        assert!(panicked.is_err());
        assert_eq!(active.load(Ordering::Acquire), MAX_CONNECTIONS - 1);

        drop(slots);
        assert_eq!(active.load(Ordering::Acquire), 0);
    }

    #[test]
    fn non_loopback_bind_is_rejected() {
        let res = ControlServer::spawn_with_token("0.0.0.0:0", ControlAuthToken::generate(), None);
        assert!(res.is_err());
        let err = res.err().unwrap();
        assert!(err.contains("non-loopback"), "unexpected error: {err}");
    }

    #[test]
    fn unauthenticated_client_is_rejected_before_command_dispatch() {
        let token = ControlAuthToken::generate();
        let srv = ControlServer::spawn_with_token("127.0.0.1:0", token, None).unwrap();
        let mut stream = TcpStream::connect(srv.addr).unwrap();
        stream.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
        writeln!(stream, "{}", json!({"id": 1, "method": "engine.execute", "params": {"command": "comp.new"}})).unwrap();
        let mut reader = BufReader::new(stream);
        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
        let reply: Value = serde_json::from_str(&line).unwrap();
        assert_eq!(reply["id"], 1);
        assert_eq!(reply["ok"], false);
        assert!(reply["error"].as_str().unwrap().contains("unauthorized"));
        // Verify the command never reached the UI thread channel:
        assert!(srv.rx.try_recv().is_err());
    }

    #[test]
    fn wrong_bearer_token_is_rejected() {
        let token = ControlAuthToken::generate();
        let srv = ControlServer::spawn_with_token("127.0.0.1:0", token, None).unwrap();
        let mut stream = TcpStream::connect(srv.addr).unwrap();
        stream.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
        writeln!(
            stream,
            "{}",
            json!({"id": 1, "method": "auth", "params": {"token": "0000000000000000000000000000000000000000000000000000000000000000"}})
        )
        .unwrap();
        let mut reader = BufReader::new(stream);
        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
        let reply: Value = serde_json::from_str(&line).unwrap();
        assert_eq!(reply["ok"], false);
        assert!(reply["error"].as_str().unwrap().contains("unauthorized"));
    }

    #[test]
    fn authenticated_client_executes_ping_and_commands() {
        let token = ControlAuthToken::generate();
        let srv = ControlServer::spawn_with_token("127.0.0.1:0", token.clone(), None).unwrap();

        // Spawn a responder simulating the UI loop
        let rx = srv.rx;
        let handle = std::thread::spawn(move || {
            if let Ok(req) = rx.recv_timeout(Duration::from_secs(3)) {
                let _ = req.reply.send(json!({"ok": true, "result": {"pong": true, "method": req.method}}));
            }
        });

        let mut client = effectcraft_automation::BridgeClient::with_token(&srv.addr.to_string(), token).unwrap();
        let res = client.call("ping", json!({})).unwrap();
        assert_eq!(res["pong"], true);
        handle.join().unwrap();
    }
}
