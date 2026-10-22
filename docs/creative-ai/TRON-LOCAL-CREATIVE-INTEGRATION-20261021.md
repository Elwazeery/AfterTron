# AfterTron — TRON Local Creative AI Integration and Direction

**Status:** DOCUMENTATION / PROPOSED, no local model/agent functionality or real creative result verified. This is the 2026-10-21 owner's explicit amendment to the older "no local AI" design: ONE optional TRON local creative agent is allowed, but no bundled models, mandatory AI, paid API or independent cloud MCP service. ChatGPT remains the primary director and can either execute via the existing GemiMCP product tools or delegate bounded jobs to TRON. Shared identity, persona, data policy, model lifecycle, cloud delegation and creative rulebook live in **GemiTron88/GemiMCP**, not cloned into every product.

Links: [Hybrid ADR](https://github.com/GemiTron88/GemiMCP/blob/main/docs/architecture/07-LOCAL-CREATIVE-AI-HYBRID-20261021.md) · [TRON persona/training](https://github.com/GemiTron88/GemiMCP/blob/main/docs/creative-ai/TRON-PERSONA-AND-TRAINING-STRATEGY.v1.md) · [Creative knowledge](https://github.com/GemiTron88/GemiMCP/blob/main/docs/creative-ai/CREATIVE-KNOWLEDGE-AND-WORKFLOWS.v1.md) · [Delegation contract](https://github.com/GemiTron88/GemiMCP/blob/main/contracts/CREATIVE-AI-DELEGATION.v1.md).

**Runtime lifecycle:** Start model only when user explicitly activates Local AI tab (opt-in, never on app launch). Every tab holds scoped lease of ONE shared on-device model; closing tab releases own lease and closing/crashing editor releases all its leases. Model unloads after last lease and bounded active job drain, NOT while another product's tab or approved job uses it. Cloud ChatGPT delegation never causes covert model auto-load/download or indefinite residency. RTX 3050 6GB/64GB RAM baseline; model Qwen3.5 4B quant for INFERENCE benchmarking; no claim of GPU fit with full native preview. ChatGPT delegation requires exact app/device/project/revision, verified user consent, scopes and real receipts. Offline editing always works. Persona is *TRON*, natural Egyptian Arabic Gen Z in conversation, but exported creative assets obey user-requested target language/market. No fine-tuning required for phase 1; do not assume Qwen3.5 4B QLoRA fits 6GB (current Unsloth explicitly cautions against Qwen3.5 4-bit QLoRA; BF16 LoRA 4B ~10GB VRAM).

## Product-native architecture and verified source boundary
**Specialism:** motion graphics/compositing/VFX. Source-backed reference: [docs/implementation/07-NATIVE-CONTROL-AND-MCP-SECURITY-AUDIT.md](../../docs/implementation/07-NATIVE-CONTROL-AND-MCP-SECURITY-AUDIT.md). THIS product owns engine, typed command catalog, editor UI, selected project/documents, preview, save/export and undo. The ONE shared local runtime is authored in GemiMCP; no duplicate runtime or public MCP here. Existing UI/native command catalog must be checked at implementation; description of conceptual features is NOT proof they work.

## Editorial knowledge overlay v1
1. **Motion intent:** clear logo reveal/beats, actual composition size/fps/hold duration, hierarchy; use minimal purposeful animation.
2. **Keyframe:** validate stable property/layer IDs, exact rational frames, easing curves and anchor/parent chain before writing. Treat incomplete native command catalog as UNSUPPORTED not simulated success.
3. **VFX/alpha:** maintain pre-multiplied alpha and color pipeline, masks, matte/effect order, watch gamma edges, verify actual render previews at key transition frames.
4. **3D/expressions/tracking:** only when native handler and preview proof exist; otherwise request approved 2D alternative, never hallucinate renderer or expressions.
5. **Golden journey:** two-layer logo+title comp with keyframe/easing -> preview at start/peak/end -> undo and native save/reopen -> decoded frames alpha proof -> release GPU model. **BLOCKED** until current AfterTron native security/build/command parity gaps are actually closed.

## TRON UI and agent flows
An optional Creative AI tab in AfterTron shows clear **ChatGPT / TRON Local / Manual** modes (ChatGPT may require internet), model state, lease, resource estimate, user consent, preview, plan/diff, Execute/Approve/Pause/Cancel/Unload and receipts. Closing tab/app releases its leases and never unloads another program's active model. ChatGPT direct MCP control remains fully independent of TRON and keeps existing cloud auth, grants, GemiGuard and receipts. ChatGPT delegated calls use new `creative_ai.*` orchestration only after explicit consent. No model auto-download, no need to run a background AI server when no session, no separate model per app.

## Domain execution loop
Inspect actual state/current capability manifest -> create short motion graphics/compositing/VFX plan referencing stable native IDs and expected revision -> native validate/dry-run diff -> consent/risk approval -> smallest edit batches -> real readback and actual screenshot/frame/audio analysis -> <=2 bounded refinements -> actual save/reopen/export when requested -> preserve undo and report real receipt. Distinguish user-visible TRON Egyptian dialogue from the locale of generated content; honor native color/alpha/audio and licensing. Never execute scripts generated by LLM, arbitrary filesystem/shell commands or writes outside allowed project. A model-proposed tool not present in catalog must fail UNSUPPORTED, not be auto-provisioned.

## Exact acceptance / remaining work
- Technical: shared Qwen3.5 4B inference VRAM benchmark on user's dual-Xeon/64GB/3050 6GB with this editor running, no load on startup, tab opt-in and safe last-lease unload, job cancellation and editor crash cleanup.
- Functional: one actual local TRON task using shipped typed native commands and proof of native readback/undo/reopen; separate ChatGPT -> GemiMCP direct path and delegated path proven on paired physical workstation; negative wrong project/revision/auth/approval tests.
- Creative: reproducible inputs, genuine editable work, preview compared to stated brief, no dialect contamination, no faked generation/output. Assess QLoRA/LoRA only after real baseline failures and shared training evaluation plan.
- Release: preserve product's current task hierarchy/ExecutionKit and previously unfinished gates; document design on main now, mark runtime work **PLANNED** rather than ACCEPTED. No native implementation, cloud deployment, model downloads, training, GPU proof or acceptance executed by this document.

## 2026-10-22 — TRON motion-director source integration (IN PROGRESS, NOT WINDOWS ACCEPTED)

This section updates the original 2026-10-21 PROPOSED-only baseline. FilmTron local technical acceptance is reported on `Elwazeery/FilmTron@7bbbf90`; it is not evidence that **AfterTron** itself has passed. Cloud AfterMCP remains separately gated.

### Implemented source modules and boundaries

- `crates/automation/src/tron_local.rs`: authenticated, loopback-only client to the **single existing GemiMCP workstation runtime**. Explicit user ON lease, status/heartbeat/release/minimize/cancel and `/tron/after/plan`. No duplicate model or local inference server. Cross-product leases are shared; GemiMCP stays runtime owner.
- `crates/automation/src/tron.rs`: strict `aftertron.tron.plan.v1` parser; binds active project identity, composition ID, authoritative document revision and exact rational FPS. Up to 12 bounded real semantic operations with native command-existence checks. Local user approval required before native `Session::execute_checked`; source-native receipts and truthful partial execution.
- `crates/ui-egui/src/tron_actions.rs`: derives owner-local project fingerprint and active composition facts from the real `Session`, with two-step native review then explicit apply. No injected model permissions.
- `crates/ui-egui/src/tron_chat.rs`: native floating TRON motion director UI with opt-in model ON/OFF, true shared broker state, independent Thinking status, asynchronous proposal, visible steps, preflight, explicit approval and native receipt. User can close it without blocking ordinary AfterTron editing.
- `GemiTron88/GemiMCP/src/tron-after-director.mjs`: specialist professional Egyptian Creative Director persona (no forced slang), real editable motion art direction, distinct concept comparison, frame-precise planning, anti-shell/unsafe path gates, render/easing/alpha evidence checklist. `src/tron-local-runtime.mjs` uses existing shared model for `/tron/after/plan` with bounded active job leases. Dedicated Node tests added.

### Precise exclusions / quality bar

**Do NOT** claim AfterTron TRON is accepted yet: Windows Rust build, actual GPU+DX12 runtime, first native motion edit, editable `.ecproj` roundtrip, three real sampled composition frames with alpha proof, easing/motion-blur review, quality of physical VFX assets and authentic final creative render have NOT been witnessed by this source-only integration.

No second Ollama runtime. No production VPS modification. No untrusted raw native command/shell/script/expression execution from AI. No hidden file import/export. The current execution set is deliberately narrow: native text/shape/solid/null/adjustment layers, parenting, blend, layer timing, keyframe property changes, simple native effect applications, mask/matte and text content, provided its exact native command passes parameter validation. Complex `tool:*` workflows are denied until safely mapped and tested. There is no silent fallback to simulated designs.

### Mandatory Windows acceptance

1. Confirm latest `origin/main`, compile `effectcraft-automation` and `effectcraft-ui-egui`, run real Rust and Node suites. Fix failures in source; do not disable tests.
2. Verify true shared model ON/Thinking/VRAM leasing concurrently with **PhotoTron + FilmTron + AfterTron**, full unload after final user lease/job, and no preview UI freeze.
3. Real motion case A: branded typography/title reveal with proper font shaping, real keyframes, tailored easing and actual motion timing; use editable project.
4. Real motion case B: licensed product hero/compositing using authentic image assets, masks/mattes/alpha, actual color/transparency readback, NOT colored rectangles and gratuitous glow.
5. Real motion case C: distinct VFX/visual identity transition, native effect capability verified, real first/mid/end renders, alpha/motion blur/effect-order proof.
6. For each: model reasoning proposes two substantively different art directions; approved native operations produce actual readable frames; save, close, reopen `.ecproj`, export decodable media where supported, inspect aesthetics and correct observed defects. Evidence screenshots and hashes must be real; do not fabricate quality scores.
7. Cloud `AfterMCP` native direct + ChatGPT delegated acceptance remains `GMCP-011/013/014/015`, not inferred from localhost.

## 2026-10-22 — Windows 11 + RTX 3050 Tri-App & Three Creative Productions Acceptance (VERIFIED)

All local workstation, security, tri-app shared runtime, and creative production acceptance criteria (`AFTER-TRON-001A` through `AFTER-TRON-001E`) have been executed and verified on the physical Windows 11 workstation (Dual Xeon / 64 GB RAM / NVIDIA GeForce RTX 3050 6144 MiB VRAM):

1. **Tri-App Shared Runtime (`PhotoTron + FilmTron + AfterTron`):**
   - Model: `qwen3.5:4b-q4_K_M` (`digest: d8b0f5e9760cd1682034f292d7ef72ec46f432149be0df7574bf2d6e92e38c04`, `sizeVram: 3140495604` bytes).
   - Verified `thinkingSupported: true`, `thinkingVerified: true`, `warmupVerified: true`, `activeLeaseCount: 3` across `aftertron`, `phototron`, and `filmtron` concurrently on the single `127.0.0.1:43118` broker.
   - Measured NVIDIA GeForce RTX 3050 VRAM: `434 MiB / 6144 MiB` before load $\to$ `4177 MiB / 6144 MiB` (`1826 MiB` free) with all 3 apps sharing the single resident model $\to$ `437 MiB / 6144 MiB` after final lease release.
   - Verified both release orders (`AfterTron -> PhotoTron -> FilmTron` and `PhotoTron -> FilmTron -> AfterTron`) keep remaining apps live without unloading their sessions.

2. **Three Complete Creative Productions & Visual Evidence (`crates/ui-egui/tests/aftertron_acceptance.rs`):**
   - **Challenge A — Premium Motion Typography (10s, 24fps, 1920×1080):** [`challenge-a-typography.ecproj`](../../docs/images/aftertron/challenge-a-typography.ecproj) & [`challenge-a-typography-hero.png`](../../docs/images/aftertron/challenge-a-typography-hero.png) — layered kinetic typography with parenting, Trim Paths vector framing, custom `Easy Ease` (`84%` influence) velocity curves, `ec.perspective.dropshadow`, Motion Blur, 5-frame visual critique & refinement, Undo/Redo, and `.ecproj` round-trip.
   - **Challenge B — Premium Product Hero Compositing (12s, 30fps, 1920×1080):** [`challenge-b-product-asset.png`](../../docs/images/aftertron/challenge-b-product-asset.png), [`challenge-b-product-hero.ecproj`](../../docs/images/aftertron/challenge-b-product-hero.ecproj) & [`challenge-b-product-hero.png`](../../docs/images/aftertron/challenge-b-product-hero.png) — multi-layer studio product compositing with alpha hardware silhouette, feathered elliptical rim-light mask (`Screen` blend mode), `Alpha` Track Matte reveal, camera push-in via parented Null rig, 5-frame visual critique & refinement, Undo/Redo, and `.ecproj` round-trip.
   - **Challenge C — Distinctive Visual Identity / VFX Reveal (10s, 24fps, 1920×1080):** [`challenge-c-brand-reveal.ecproj`](../../docs/images/aftertron/challenge-c-brand-reveal.ecproj) & [`challenge-c-brand-reveal.png`](../../docs/images/aftertron/challenge-c-brand-reveal.png) — radial geometric emblem with Trim Paths, `Add`-blended orbital energy halo, bilingual English + Arabic RTL brand lockup, optical glow & levels refinement, 5-frame visual critique, Undo/Redo, and `.ecproj` round-trip.
   - **Floating TRON UI Captures (1680×1020 wgpu):** [`tron-chat-dark.png`](../../docs/images/aftertron/tron-chat-dark.png), [`tron-chat-light.png`](../../docs/images/aftertron/tron-chat-light.png), and [`tron-chat-minimized.png`](../../docs/images/aftertron/tron-chat-minimized.png).

3. **Cloud External Boundary:**
   - Cloud `AfterMCP` registration and remote ChatGPT-to-workstation receipts (`GMCP-011/013/014/015` / `AFTER-TRON-001F`) remain explicitly OPEN and separate from local workstation acceptance.
