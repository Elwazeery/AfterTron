> **2026-10-20 source import verified:** Rust workspace `Cargo.toml`, upstream license files, and `docs/upstream/IMPORT-RECORD.txt` are present on `main`. Upstream ancestry imported by a two-parent merge. Native build, Rebrand, GPU/PSD/VFX performance and complete cloud MCP remain OPEN. See [original upstream README](docs/upstream/ORIGINAL-README.md) and [source receipt](docs/upstream/IMPORT-RECORD.txt).

# AfterTron — Independent Tron Creative Desktop Application

**Source status (2026-10-20): UPSTREAM SOURCE IMPORTED / NATIVE BUILD UNVERIFIED.** Original `storytold/effectcraft` Git commit `de3fec28ed7a3d7a700a9d8ea307779dcda99a8e` merged with ancestry and licenses; see `docs/upstream/IMPORT-RECORD.txt`. Initial window title/Windows metadata rebrand landed; full app/installer/icon rebrand, tests, native MCP and cloud deployment remain OPEN.

**Product:** Professional After Effects-class compositor, motion graphics and visual effects editor.  
**Canonical repo:** `Elwazeery/AfterTron`  
**Approved upstream:** [`storytold/effectcraft`](https://github.com/storytold/effectcraft) — preserve full upstream history, remote, licensing, NOTICE and ability to sync.  
**Creative product independence:** not part of GemiMotion. No shared project engine, no merging repos, no dependency on GemiMotion to function.  
**Unified Cloud MCP:** [GemiTron88/GemiMCP](https://github.com/GemiTron88/GemiMCP) is SOLE platform cloud MCP runtime owner. This product is sole owner of its native commands, file formats, editing project and adapter. Its individually addressable MCP resource will be `https://mcp.gemitron.net/products/after/mcp` **(PROPOSED, not deployed)**.

## Required user outcome

Preserve and substantially improve the existing open-source application: native Windows/macOS editing, project stability, performance, broad pro functionality (compositions, precomps, layer/matte/masks, effects, expressions, per-property keyframes, easing graphs, shape/text animation, 3D camera/lights, tracking, render queue, format exports), real saved editable assets, predictable undo, trustworthy rendering, error recovery and premium usable UI. Do NOT replace the native application with a thin browser dashboard or general AI chat client.

**Brand:** AfterTron, built with versioned GemiTokens **Inferno Pixel Glass** + GemiFontSans/appropriate licensed icon assets; original elegant app chrome, restrained glass/pixel motifs, real accessibility/Arabic/English support and shortcuts, ergonomic pro layouts. Preserve neutral/correct preview color.

**Identity:** ONLY central `auth.gemitron.net` and **Gemi Access within Studio** for project/role/capability grants. Browser PKCE for desktop. No standalone password store, no local login requirement to open owned offline projects.

**Licensing:** ONLY GemiGuard (product/edition/features/seat/device/offline Ed25519 lease); preserve existing editable user projects during cloud outage/expiry, respecting licensed feature policy. Do not invent a second licensing DB.

## AI-assisted operation

The assistant's intelligence is in the connected ChatGPT session; this application never requires a separately built AI chatbot or paid external AI API. Full CRUD of each meaningful implemented UI action/property must be backed by the **same native command engine** and be available through the cloud MCP resource. A shared outbound-only Desktop Agent bridges authenticated commands to the local app with exact project/device/user grants, readback, preview, revision, safe staged writes, and undo.

**100% native UI ↔ command ↔ cloud MCP parity is a future acceptance criterion, NOT a completed claim.** No fake tool success or mock render.

**First actual acceptance scene:** Modify an exact property of a layer at a specified composition frame, alter easing and effect parameter, preview frame, undo and render/reopen reliably.

## Canonical documentation

- [Product and complete MCP parity implementation charter](docs/01-FORK-PRODUCT-AND-CLOUD-MCP-CHARTER.md)
- [Execution task backlog](TODO.md)
- [Shared cloud/auth/device/security contracts](https://github.com/GemiTron88/GemiMCP/blob/main/docs/architecture/02-CONTRACTS-SECURITY-AND-DELIVERY.md)

### Source import guard

This repository began with planning documents before a full-history source import. Upstream source `storytold/effectcraft@de3fec28ed7a3d7a700a9d8ea307779dcda99a8e` is now merged on main; review `docs/upstream/IMPORT-RECORD.txt`. This is a **source fork only**, not proof of correct native builds, complete brand migration, production-readiness, or MCP parity. Upstream CI workflow definitions are held outside active `.github/workflows/` pending review. Use actual upstream/target commit references and acceptance checks.
