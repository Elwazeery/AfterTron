# Product charter — fork, native editing, cloud MCP, SSO and licensing

**Owner-approved 2026-10-20. Documentation-only.** See centralized runtime owner [GemiMCP](https://github.com/GemiTron88/GemiMCP) and its [architecture](https://github.com/GemiTron88/GemiMCP/blob/main/docs/architecture/01-CLOUD-MCP-PLATFORM.md).

## Common binding product/architecture rules

1. One independent desktop editor, its own project schema, undo stack, asset storage, renderer and native command engine. **Never merge into or depend on GemiMotion**, and never substitute a web mockup for a professional workstation binary. First-class Windows/macOS, measured support of any extra platforms.
2. **Real fork:** inspect/pin upstream version + current license; import upstream Git history and establish upstream remote for periodic audited sync. Preserve LICENSE/NOTICE, dependencies' terms and author credits; rebrand original marks only where rights permit; record added code separately. Avoid force push destroying documentation/history.
3. **Full native/MCP parity:** inventory ALL implemented menus/toolbar/keyboard shortcuts/inspector and nested object properties, session/UI state, dialogs, batch ops, exporter features. Every meaningful UI operation maps to typed native command/query with stable object IDs, deterministic readback, expected revision, scope, undo/redo or explicit non-reversible status and real preview. Runtime MCP transport/server implementations **only in GemiMCP repo**, not an extra competing gateway in this repo. This repo owns native command adapters/manifest/tests.
4. **Cloud execution:** protected `mcp.gemitron.net` routes in central repo, direct standalone product MCP AND federated GemiMCP availability. Outbound-only device agent on Windows/macOS, no public open ports, no shell/SQL, allowlisted media roots. Offline native edit remains available subject to GemiGuard local lease; ChatGPT remote control requires a connected device.
5. **Unified account/license:** only OIDC `auth.gemitron.net` + Studio-embedded Gemi Access grants. Only GemiGuard product entitlement/signed offline leases; never write local identity/seat DB or duplicate centralized grant writer. Desktop uses system browser PKCE, secure OS keychain, explicit sign-out/revoke, per-device capability grants. No mandatory live login just to open owned local projects.
6. **Design:** consume pinned generated GemiTokens and GemiFontSans (with tested platform shaping), high-performance original **Inferno Pixel Glass** aesthetic, product-specific accents, dark/light, keyboard/pro dock layout, Arabic/English mixed-direction text, real screenshots and accessibility. Accurate video/image color never tinted by app chrome. Restrict costly blur/animation near preview/canvas.
7. **Quality:** functionally real 2–3 minute editing journey + 100% command parity *target*, actual physical workstation input/preview/save/reopen/undo/export; negative tests for wrong project/device, grant, token, lease, replay, unavailable provider, corrupt source, crash after apply and graceful recovery. Report unsupported separately, never false ACCEPTED status.
8. **AI & cost:** ChatGPT connected via cloud MCP is the directing brain; no local chatbot, no paid LLM/media API bundled, no assertion that a server can perform ongoing agent reasoning unaided; optional neural tools clearly require approved provider availability/cost.
9. **Release:** source-based tasks follow installed ExecutionKit, native OS CI + GPU tests as available, versioned signed release and safe rollback; every completed claim needs commit SHA, tests, screenshot/export and native readback evidence. First verify real upstream feature gaps, then implement only demonstrably missing/high-value features.

## Native MCP baseline command families

Required semantic coverage: `session.*` `project.*` `assets.*` `document.inspect` `object.list/get/create/update/delete` `property.enumerate/get/set` `selection.*` `history.undo/redo` `preview.frame/capture` `render/export.*` `jobs.status/cancel` `ui.layout/panels/selection` and domain-specific namespaces documented below. **These are planned families** — discover real native handler names before implementing; don't register placeholders as active tools.

Required call pattern: inspect → prepare non-mutating diff & expected revision → authorize/confirm if needed → native apply/receipt → preview/readback/verify → optional undo. Long jobs report progress and cancellation with a durable status. Batch operations must be atomic only where native transaction is actually supported; otherwise report partial and compensating edits.

## Upstream/standalone acceptance boundary

Standalone MCP means direct cloud tool access scoped to THIS product even if federated GemiMCP discovery is unavailable, while sharing the same one GemiMCP-hosted protocol/security stack. It does NOT mean standalone account, independent license, new GitHub MCP repo, or new AI model.

## AfterTron-specific scope and missing-feature priority

**Upstream:** `storytold/effectcraft`, Rust compositor. Its feature counts are NOT proof of professional parity with Adobe; verify each actual effect outcome and stability.

**Professional feature coverage to audit against actual implementation:**
- Composition/precomp nesting, duration/frame rate, layer types, parenting, transformations, track mattes, masks/roto, motion blur, live typography/RTL and shape editing.
- Every property keyframe interpolation/tangents/expressions, easing graph, time remapping, marker/track interaction and scoped undo.
- Effect plugin inventory and parameters (read/write via introspection), effect stack order, shader parity and edge cases; color/HDR/premultiplied-alpha and gamma tests.
- 2.5D/3D camera/lights/materials/render passes where real, tracking/warp stabilization where available, hardware GPU/CPU preview comparability and robust render queue.
- Interchange: Lottie and other advertised import/export with failure/loss evidence; no blanket claim to open/edit/export every After Effects .aep feature.

**Additional typed MCP domains:** `after.composition.*`, `after.layer.*`, `after.transform.*`, `after.property.*`, `after.keyframe.*`, `after.graph.*`, `after.expression.*`, `after.mask.*`, `after.matte.*`, `after.effect.*`, `after.camera.*`, `after.render.*`; nested effect parameters must be enumerable and addressable by stable IDs and frame/time.

**P0 vertical acceptance:** construct a real comp with source image, text and logo; modify one layer's position at exact frame+easing, add actual effect parameters, inspect numerical graph, preview rendered frames, undo/redo scoped operation, save/reopen and render real video; compare frame visuals and alpha without altered unrelated layers.
