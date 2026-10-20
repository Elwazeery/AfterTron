# AfterTron — Native Control / Cloud MCP Gap Audit

**Source inspection:** 2026-10-20 · **Status:** SOURCE-INSPECTED, CLOUD EXECUTION NOT ENABLED / NO REAL CLIENT ACCEPTANCE.

## Existing actual upstream native control

EffectCraft control protocol starts at 127.0.0.1, also accepts unauthenticated JSON lines; official documentation states 'The port has no authentication'. Input is framed and length-bounded (4 MiB) and connection count capped (16) but NOT principal authenticated. A separate bug was confirmed and fixed this round: connection quota slots could leak on worker panic/spawn failure; RAII ConnectionSlot now releases permits, with two source tests.

Source locations: `apps/effectcraft/src/control_server.rs`, `docs/control-protocol.md`, native command registry/automation sources; all source owned by this product. This control is NOT GemiMCP's cloud server.

## Native commands already advertised

`engine.commands`, `command.list`, `command.describe`, `engine.execute`, `ui.inspect`, `ui.elements`, `ui.set`, `render.frame`, `ui.screenshot`, `ui.timeline.*`, `jobs.list`, comp/layer/prop inspections. All published feature claims need native/remote parity proof.

Do not replace engine commands with synthetic mouse clicks. Keep UI manipulation for dialog/window operations and verified source state. Inventory actual capabilities/parameters and dynamic properties, typed input/output schemas, stable IDs, safe apply/verify/undo and real screenshot/export evidence to satisfy the future **100% MCP parity acceptance goal**, measured against implemented GUI actions.

## Mandatory next remediation

P0 BLOCKER BEFORE ANY CLOUD DEVICE BRIDGE: implement native short-lived authenticated command channel with per-launch secret or OS-protected IPC, auth deadline, denied unknown/untrusted commands, project-root and resource isolation, replay mitigation, native revision fences and offline safe defaults. Include wrong-user/wrong-device/rogue-loopback-client tests and preserve compatibility intentionally.

Required secure path: `ChatGPT → GemiTron88/GemiMCP cloud MCP (OIDC + Gemi Access + GemiGuard where relevant) → scoped outbound Desktop Agent → authenticated local command/IPC → native editor readback/receipt`. Never open the local port to a public interface. The cloud MCP implementation is maintained centrally in `GemiTron88/GemiMCP`, native editor/API corrections only here.

**Acceptance evidence needed:** independent wrong-token/bypass/rogue-process test, UI↔native command parity manifest and tests, physical Windows/macOS native save/undo/readback, device disconnect/recovery, authoritative job receipt. Do not declare Cloud MCP 'done' from the presence of the local control method names.

## Source-only changes and verified limits

- Source fork ancestry verified via GitHub compare against pinned original Git SHA, original license files preserved.
- First visible Tron title/Windows metadata/CLI rebrand committed.
- ConnectionSlot RAII fix committed with unit tests (Rust suite has NOT been executed or verified on Windows/macOS).
- Existing executables/source crate names/APP_ID and original project file extensions retained until safe migration, icons and full visual redesign remain open.
