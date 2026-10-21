# AfterTron — Native Control & Cloud MCP Security Audit

**Audit & Verification Date:** 2026-10-21 · **Status:** REMEDIATED & VERIFIED BY AUTOMATED NEGATIVE + POSITIVE TESTS (`effectcraft-automation` & `effectcraft` & `aftertron_acceptance`).

---

## 1. Remediated Vulnerability Summary

Upstream EffectCraft's `--control <port>` server previously accepted unauthenticated JSON-line commands on `127.0.0.1:<port>`, allowing any local process to execute arbitrary engine commands or file imports.

All native control and MCP bridge security requirements (`AFTER-010`, `AFTER-003`, `AFTER-005`, `AFTER-006`) are now implemented and verified:

| Security Control | Implementation Location | Verification Test(s) |
|---|---|---|
| **RAII Connection Slot Permit** | `apps/effectcraft/src/control_server.rs` (`ConnectionSlot`) | `control_server::tests::connection_slot_releases_on_drop_spawn_error_and_panic` |
| **Mandatory CSPRNG Bearer Token Handshake (`auth`)** | `crates/automation/src/security.rs` (`ControlAuthToken`), `apps/effectcraft/src/control_server.rs` (`AUTH_DEADLINE = 10s`, constant-time 256-bit hex comparison) | `control_server::tests::unauthenticated_client_is_rejected_before_command_dispatch`, `wrong_bearer_token_is_rejected`, `authenticated_client_executes_ping_and_commands` |
| **Loopback-Only Bind Guard** | `crates/automation/src/security.rs` (`validate_loopback_bind`), `apps/effectcraft/src/control_server.rs` | `control_server::tests::non_loopback_bind_is_rejected`, `security::tests::loopback_bind_rejects_public_interfaces` |
| **Owner-Only Token File ACL (`--control-token-out`)** | `crates/automation/src/security.rs` (`write_owner_only_token_file` using `icacls /inheritance:r /grant:r "<SID>:(R,W)"` on Windows and `0600` on Unix), `apps/effectcraft/src/main.rs` | `main_tests::parses_flags_and_files` |
| **Workspace Path Confinement** | `crates/automation/src/security.rs` (`AuthorizedWorkspace`), `crates/automation/src/server.rs` (`FilePolicy`) | `security::tests::workspace_confines_paths_and_blocks_traversal` |
| **Optimistic Concurrency (`expectedRevision`) & Replay Protection (`idempotencyKey`)** | `crates/ui-egui/src/control.rs`, `crates/automation/src/server.rs`, `crates/automation/src/security.rs` (`ReplayProtector`) | `security::tests::replay_protector_caches_bounded_keys`, `aftertron_acceptance` (Step 11 & 12) |
| **GemiMCP Product Manifest & Outbound Bridge Adapter** | `crates/automation/src/gemimcp.rs` (`build_aftertron_product_manifest`, `OutboundBridgeAdapter`), `contracts/gemimcp-product-aftertron.v1.json` | `gemimcp::tests::manifest_has_required_schema_and_covers_all_engine_commands`, `gemimcp::tests::contract_file_matches_generated_manifest` |
| **OIDC PKCE (`auth.gemitron.net`) & GemiGuard Ed25519 Offline Lease / Revocation Verifier** | `crates/automation/src/gemiguard.rs` (`OidcPkceRequest`, `OfflineLease`, `RevocationList`, `GemiGuardVerifier` with pure-Rust RFC 8032 Ed25519) | `gemiguard::tests::oidc_pkce_generates_valid_s256_challenge_and_auth_url`, `ed25519_signs_and_verifies_offline_lease_and_handles_expiry_grace_and_revocation`, `forged_signature_and_wrong_device_are_rejected_safely` |
