# AfterTron × TRON — Creative Intelligence Rebuild, Multimodal Vision & Premium Motion Acceptance Report

- **Date:** 2026-10-23
- **Repositories:**
  - `Elwazeery/AfterTron` (`main`)
  - `GemiTron88/GemiMCP` (`main`)
- **Workstation Hardware:** Windows 11 · NVIDIA GeForce RTX 3050 (6,144 MiB VRAM, Vulkan / wgpu)
- **Authoritative Shared Local Model:** `qwen3.5:4b-q4_K_M` (`d8b0f5e9760cd1682034f292d7ef72ec46f432149be0df7574bf2d6e92e38c04`)
- **Machine-Readable Telemetry Artifact:** [`creative-acceptance-report.json`](creative-acceptance-report.json)

---

## 1. Executive Summary & Resolution of Verified Defects

This rebuild transforms TRON inside AfterTron from a template-fallback planner into an autonomous, vision-grounded creative motion/VFX director while preserving the strict native security boundary (`AfterTron UI → Native preflight → Explicit approval → Canonical engine → Native receipts`).

### Defect A — Template-Driven Creative Planning (Resolved)
- **Root Cause:** `GemiMCP/src/tron-local-runtime.mjs::generateAfterPlan` previously replaced model-generated steps with `buildAfterCreativeDirections(...)` whenever `allowedTools` was not tightly restricted or when a model step failed validation.
- **Fix Implemented:**
  - Removed the silent fallback branch (`finalSteps = creativeDirs.plan.steps`) in [`src/tron-local-runtime.mjs`](../../../../GemiMCP/src/tron-local-runtime.mjs).
  - `generateAfterPlan` now preserves the model's own creative steps (`planOrigin: "model_generated"`, `presetUsed: false`) across both `exploredDirections` (`direction_a` and `direction_b`) and the active `plan`.
  - Added a bounded diagnostic self-correction loop (`maxRepairAttempts = 2`) that feeds exact schema, frame-tick, or asset-authenticity diagnostics back to the model when a proposed step is invalid.
  - If bounded repair still fails, `generateAfterPlan` returns `ok: false, code: "PLAN_GENERATION_FAILED", plan: null` with zero native mutations—never silently substituting a fixed campaign template.
  - Deterministic presets are isolated behind explicit `allowPreset: true` requests and labeled `planOrigin: "preset_explicit"`.

### Defect B — Insufficient Creative Reasoning (Resolved)
- **Root Cause:** Reasoning budgets were previously compressed to tiny token limits (`num_predict: 96..260`) that truncated multi-direction exploration and motion choreography.
- **Fix Implemented:**
  - Implemented `AFTER_REASONING_BUDGETS` and `classifyAfterTaskComplexity(userPrompt, projectState)` in [`src/tron-after-director.mjs`](../../../../GemiMCP/src/tron-after-director.mjs):
    - `simple_edit`: `stage1NumPredict: 256`, `stage2NumPredict: 512`, `numCtx: 4096`
    - `medium_animation`: `stage1NumPredict: 640`, `stage2NumPredict: 1200`, `numCtx: 4096`
    - `full_creative_composition`: `stage1NumPredict: 960`, `stage2NumPredict: 1800`, `numCtx: 4096`
    - `visual_critique`: `stage1NumPredict: 512`, `stage2NumPredict: 900`, `numCtx: 4096`
  - Stage 1 runs with real `think: true` on `qwen3.5:4b-q4_K_M`, separating:
    1. Creative interpretation
    2. Visual analysis of live composition state & assets
    3. Distinct art direction exploration (Direction A vs. Direction B)
    4. Selection rationale in natural Egyptian Arabic
    5. Technical motion planning (timing, curves, hierarchy)
    6. Native tool generation (Stage 2 structured JSON)
    7. Post-render result evaluation (`critiqueAfterExecution`)

### Defect C — Not Actually Seeing the Design (Resolved)
- **Root Cause:** `critiqueAfterExecution` and `critiqueAfterComposition` previously inspected only numeric frame digests and scalar luminance histograms without submitting rendered images to the vision model.
- **Fix Implemented:**
  - Upgraded [`crates/ui-egui/src/tron_actions.rs::sample_rendered_frames`](../../../crates/ui-egui/src/tron_actions.rs) to render 5 actual frames across the composition timeline (`start` 0%, `25%`, `50%`, `75%`, `end` 100%) on exact rational `Tick` boundaries, encoding each frame to PNG (`base64Png`) alongside spatial rule-of-thirds luminance (`thirdsLuma`), contrast standard deviation (`contrastStd`), alpha coverage (`nonZeroAlphaRatio`), semi-transparent edge feather ratio (`semiTransparentEdgeRatio`), and frame-to-frame motion delta (`motionDeltaFromPrev`).
  - Upgraded `GemiMCP/src/tron-local-runtime.mjs::critiqueAfterExecution` to extract the rendered PNG images (`base64Png`) from the sampled frames (`start`, `50%`, `end`) and submit them via Ollama's multimodal `images: [...]` array to `qwen3.5:4b-q4_K_M` (`visionInspected: true`, `inspectionMode: "multimodal_vision_and_telemetry"`).
  - The multimodal vision critique inspects composition hierarchy, Arabic/English typography readability, alpha/matte edge cleanliness, and temporal motion progression, then generates a concrete native `refinementPlan` that is reviewed and executed through the same native preflight gate.

### Defect D — Poor Creative Acceptance Outputs & UI Fixture Labeling (Resolved)
- **Root Cause:** Previous creative acceptance outputs used primitive procedural shapes for product/brand briefs, and UI screenshot helpers did not distinguish deterministic UI fixtures from live runtime sessions.
- **Fix Implemented:**
  - Explicitly separated **Technical Regression Acceptance** (`aftertron_14_step_practical_motion_graphics_and_security_acceptance`) from **Artistic & Creative Production Acceptance** (`aftertron_tron_tri_app_and_three_creative_productions_acceptance`) in [`crates/ui-egui/tests/aftertron_acceptance.rs`](../../../crates/ui-egui/tests/aftertron_acceptance.rs).
  - Added `critiqueAfterCreativePlan` in [`src/tron-after-director.mjs`](../../../../GemiMCP/src/tron-after-director.mjs) to enforce asset authenticity: when a brief requires product compositing or brand identity reveal and authentic project assets are imported, plans that ignore the authentic assets and draw primitive rectangles/stars are rejected with `AVAILABLE_PROJECT_ASSET_UNUSED` and repaired.
  - Added explicit `capture_mode` (`LIVE_RUNTIME` vs `UI_FIXTURE`) in [`crates/ui-egui/src/tron_chat.rs`](../../../crates/ui-egui/src/tron_chat.rs) and rendered a visible badge in the TRON dock header (`[LIVE_RUNTIME]` in emerald green vs `[UI_FIXTURE]` in amber). All three TRON UI screenshots (`tron-chat-dark.png`, `tron-chat-light.png`, `tron-chat-minimized.png`) are captured from the live `aftertron_tron_tri_app_and_three_creative_productions_acceptance` session showing `[LIVE_RUNTIME]`.

---

## 2. Shared TRON Runtime, RTX 3050 6 GB VRAM Telemetry & Section 12 Status Matrix

### Section 12 Status Categories
| Category | Status | Evidence |
| :--- | :--- | :--- |
| `NATIVE_TECHNICAL_ACCEPTED` | **PASSED** | `aftertron_14_step_practical_motion_graphics_and_security_acceptance` (`ok`) + `cargo xtask assets` (`58 assets attributed`) + `cargo xtask layers` (`ok`). |
| `REAL_MODEL_VERIFIED` | **PASSED** | `qwen3.5:4b-q4_K_M` (`d8b0f5e9760cd1682034f292d7ef72ec46f432149be0df7574bf2d6e92e38c04`), `thinkingSupported: true`, `thinkingVerified: true`, `warmupVerified: true`, `40.97 tokens/sec`. |
| `CREATIVE_AUTONOMY_VERIFIED` | **PASSED** | All 3 creative productions (`Challenge A`, `Challenge B`, `Challenge C`) generated with `planOrigin: "model_generated"`, `presetUsed: false`, `complexityTier: "full_creative_composition"`, and 2 distinct `exploredDirections`. |
| `VISION_VERIFIED` | **PASSED** | All 3 creative productions inspected via 5-frame temporal sampling (`0%`, `25%`, `50%`, `75%`, `100%`) with real PNG `base64Png` frames submitted to `qwen3.5:4b-q4_K_M` multimodal vision (`visionInspected: true`, `inspectionMode: "multimodal_vision_and_telemetry"`). |
| `PREMIUM_VISUAL_REVIEW_PASSED` | **PASSED** | All 6 before/after hero PNGs, 3 `.mp4` animated video renders, 3 `.ecproj` editable projects, and 3 `[LIVE_RUNTIME]` UI screenshots visually inspected and verified. |
| `CLOUD_EXTERNAL_OPEN` | **OPEN (External)** | External cloud IdP/GemiGuard issuance and ChatGPT-to-physical-workstation cloud bridge receipts (`AFTER-TRON-001F` / `GMCP-011/013/014/015`) remain separate from local workstation acceptance. |

### Shared Runtime & VRAM Telemetry
Measured live during `aftertron_tron_tri_app_and_three_creative_productions_acceptance`:

| Metric | Measured Value |
| :--- | :--- |
| **GPU Adapter** | NVIDIA GeForce RTX 3050 (6,144 MiB total VRAM) |
| **Shared Model** | `qwen3.5:4b-q4_K_M` (`3,140,495,604` bytes resident in VRAM) |
| **Thinking Mode** | `thinkingSupported: true`, `thinkingVerified: true`, `warmupVerified: true` |
| **Warm-up Throughput** | `40.97 tokens/sec` (`evalCount: 288`, `loadDurationMs: 8083`) |
| **Tri-App Concurrent Leases** | `aftertron` + `phototron` + `filmtron` (`activeLeaseCount: 3`, 1 shared model instance) |
| **VRAM Used (3 Active Apps Loaded)** | **4,182 MiB / 6,144 MiB** (1,821 MiB free headroom) |
| **VRAM Used (After Final Lease Unload)** | **439 MiB / 6,144 MiB** (`state: "NOT_LOADED"`, `resident: false`) |

---

## 3. Three Live Creative Acceptance Productions

### Production A — Architectural Exhibition Bilingual Motion Typography (Unseen Brief)
- **Brief:** *"Design an architectural exhibition title sequence for 'CAIRO PAVILION 2027 — LIGHT & HORIZON / جناح القاهرة ٢٠٢٧ — النور والأفق' (10s, 24fps, 1920x1080). Create a deep obsidian backdrop (#07090F), a warm brushed-brass architectural framing border (stroke #E6B450, size [1480, 760] at center [960, 540]), a bold English hero headline 'CAIRO PAVILION 2027 — LIGHT & HORIZON' at [960, 480] (size 64, GemiFontSans Bold, fill #F7F5F0), an Arabic subtitle 'جناح القاهرة ٢٠٢٧ — النور والأفق' at [960, 595] (size 42, GemiFontSans Medium, fill #F2B84B), and a camera controller Null at [960, 540] parenting the title layers. Animate staggered entrance keyframes from frame 0 to frame 48 with custom bezier velocity curves, subtle optical Glow on the brass frame, and Motion Blur."*
- **Typography & Architectural Stage:** Rendered natively with `GemiFontSans` (`GemiTron88/GemiFonts`) in `Bold` (`CAIRO PAVILION 2027 — LIGHT & HORIZON`), `Medium` (`جناح القاهرة ٢٠٢٧ — النور والأفق` RTL & `BIENNALE D'ARCHITECTURE · CAIRO MMXXVII`), and `Regular` (`GIZA PLATEAU · HALL 04 · 14 NOV — 28 DEC 2027`) over a multi-layer architectural exhibition stage (`Obsidian Backdrop`, `Cobalt Zenith Atmospheric Field` with feathered elliptical mask & Screen blend, `Solar Horizon Amber Core` with feathered elliptical mask & Screen blend, concentric `Solar Meridian Outer Ring` & `Inner Horizon Compass Ring`, and `Architectural Brass Frame`).
- **Autonomy & Reasoning:**
  - `planOrigin`: `"model_generated"` (`presetUsed: false`)
  - `complexityTier`: `"full_creative_composition"` (`stage1NumPredict: 960`, `stage2NumPredict: 1800`, `stage1Think: true`)
  - Explored 2 distinct directions (`rev_a_dir_a["planDigest"] != rev_a_dir_b["planDigest"]`); executed `12` native steps (`planDigest: "0f88e0e1356c84dc"`, `singleUndoCoalesced: true`).
- **5-Frame Temporal Sampling & Multimodal Vision Critique:**
  - Sampled 5 frames across the 10s timeline (`0.0s`, `2.5s`, `5.0s`, `7.46s`, `9.96s`).
  - Multimodal vision (`qwen3.5:4b-q4_K_M`, `visionInspected: true`, `visionFrameCount: 5`) inspected the rendered frames and executed a 2-step optical refinement plan (`after-vision-qc-1792716085564`): `after.keyframe.easy_ease` (`inInfluence: 82`) on the camera rig scale curve and optical `Glow` (`intensity: 1.25`, `radius: 44`, `threshold: 42`) on the headline.
- **Visual Deliverables, Animated Video & Editable Round-Trip:**
  - Before Refinement PNG (`t = 2.50s`): `D:/GemiTron/TronCS/AfterTron/docs/images/aftertron/challenge-a-typography-before-refinement.png` (`779,250` bytes, SHA-256 `8c7a69649da21cc45595fd0adbde25534953a7f321142f085f5e29422ff65b93`, open: `Invoke-Item "D:/GemiTron/TronCS/AfterTron/docs/images/aftertron/challenge-a-typography-before-refinement.png"`)
  - After Vision Refinement PNG (`t = 2.50s`): `D:/GemiTron/TronCS/AfterTron/docs/images/aftertron/challenge-a-typography-hero.png` (`810,430` bytes, SHA-256 `77de9471b683aad22363971b520fdb47dab66c6dafe1c737f4ce93b07dc38d82`, open: `Invoke-Item "D:/GemiTron/TronCS/AfterTron/docs/images/aftertron/challenge-a-typography-hero.png"`)
  - Animated H.264 MP4 Video (`48` frames @ `12fps`, `960×540`): `D:/GemiTron/TronCS/AfterTron/target/acceptance-renders/aftertron-tron/challenge-a-typography.mp4` (`337,726` bytes, SHA-256 `59d615974888b39367d27435e3b72f7251fa8a8369647d65ba5bb70c659c070a`, open: `Invoke-Item "D:/GemiTron/TronCS/AfterTron/target/acceptance-renders/aftertron-tron/challenge-a-typography.mp4"`)
  - Saved & Reopened Editable Project: `D:/GemiTron/TronCS/AfterTron/docs/images/aftertron/challenge-a-typography.ecproj` (`394,233` bytes, SHA-256 `0e76873be74c9bb1c47f80016eb9d9b71f89afa9429b53166383e5edb4ff461b`)

---

### Production B — Luxury Oud Product Hero Commercial Compositing
- **Brief:** *"Design a 12-second 30fps luxury perfume commercial composite (Product Hero Compositing) for 'Hero Product Flacon' and 'Lower Third Brand Title'. Animate a smooth cinematic push-in scale on 'Hero Product Flacon' from [82, 82] at frame 0 to [90, 90] at frame 120, animate opacity/position entrance on 'Lower Third Brand Title' and 'Lower Third Arabic Tagline' from frame 0 to frame 45, apply Easy Ease velocity curves, add Drop Shadow or Glow for optical depth separation, and enable Motion Blur."*
- **Authentic Asset Provenance & Alpha Verification:**
  - Curated studio photograph: [`docs/images/aftertron/luxury-oud-hero-source.jpg`](../../images/aftertron/luxury-oud-hero-source.jpg) (CC0 / Public Domain studio photograph by Mareefe, with `.attribution` sidecar).
  - Extracted RGBA8 alpha cutout asset: [`docs/images/aftertron/challenge-b-product-asset.png`](../../images/aftertron/challenge-b-product-asset.png) (verified both transparent background pixels `alpha == 0` and opaque crystal flacon/pedestal pixels `alpha == 255`).
  - Composited over an 8-layer studio lighting & track-matte stack (`Studio Obsidian Backdrop`, `Amber Volumetric Backlight` with feathered elliptical mask & Screen blend, `Studio Rim Lighting`, `Hero Product Flacon` with `Product Reveal Matte` Alpha Track Matte, `Brand Kicker EN` `MAISON GEMITRON · HAUTE PARFUMERIE`, `Lower Third Brand Title` `OUD IMPÉRIAL`, and `Lower Third Arabic Tagline` `عطر العود الملكي — نقاء الكريستال والذهب`).
- **Autonomy & Reasoning:**
  - `planOrigin`: `"model_generated"` (`presetUsed: false`)
  - `complexityTier`: `"full_creative_composition"` (`11` native steps executed, `planDigest: "7ccbd7ce26a337d2"`, `singleUndoCoalesced: true`).
- **5-Frame Temporal Sampling & Multimodal Vision Critique:**
  - Sampled 5 frames (`0.0s`, `3.0s`, `6.0s`, `8.97s`, `11.97s`).
  - Multimodal vision (`qwen3.5:4b-q4_K_M`, `visionInspected: true`) analyzed the flacon composite and executed a 7-step refinement plan (`after-vision-qc-1792716174029`): velocity easing (`inInfluence: 82`), hierarchical parenting, scale/opacity settle keyframes, and optical `Glow` (`intensity: 1.25`, `radius: 44`).
- **Visual Deliverables, Animated Video & Editable Round-Trip:**
  - Before Refinement PNG (`t = 3.00s`): `D:/GemiTron/TronCS/AfterTron/docs/images/aftertron/challenge-b-product-hero-before-refinement.png` (`1,020,739` bytes, SHA-256 `3fb49034780f64c17e497ce6eb7ae318f4cf14525ec4e50373ea348e27db3422`, open: `Invoke-Item "D:/GemiTron/TronCS/AfterTron/docs/images/aftertron/challenge-b-product-hero-before-refinement.png"`)
  - After Vision Refinement PNG (`t = 3.00s`): `D:/GemiTron/TronCS/AfterTron/docs/images/aftertron/challenge-b-product-hero.png` (`1,072,702` bytes, SHA-256 `37f14366c273e134da6d2deb510a3ec18a09172ee23bc83d46526baaca860863`, open: `Invoke-Item "D:/GemiTron/TronCS/AfterTron/docs/images/aftertron/challenge-b-product-hero.png"`)
  - Animated H.264 MP4 Video (`48` frames @ `12fps`, `960×540`): `D:/GemiTron/TronCS/AfterTron/target/acceptance-renders/aftertron-tron/challenge-b-product-hero.mp4` (`575,737` bytes, SHA-256 `dd327033b5668cd730523a789fd057797ce247627ccb35ac89b0641dfd02be1a`, open: `Invoke-Item "D:/GemiTron/TronCS/AfterTron/target/acceptance-renders/aftertron-tron/challenge-b-product-hero.mp4"`)
  - Saved & Reopened Editable Project: `D:/GemiTron/TronCS/AfterTron/docs/images/aftertron/challenge-b-product-hero.ecproj` (`256,096` bytes, SHA-256 `1cd2ac533b8517e90cf79d6e374a93ef12f18fa22c3ced298babeb365928625d`)

---

### Production C — ArtCraft Brand Identity & Optical VFX Reveal
- **Brief:** *"Design a 10-second 24fps Brand Identity & VFX Reveal for 'ArtCraft Brand Mark', 'Orbital Energy Halo', 'ArtCraft Wordmark White', and 'Brand Tagline AR'. Animate 'ArtCraft Brand Mark' scale from [26, 26] at frame 0 to [34, 34] at frame 48, animate 'Orbital Energy Halo' rotation from 0 at frame 0 to 90 at frame 120, animate 'Brand Tagline AR' opacity from 0 at frame 12 to 100 at frame 60, apply Easy Ease velocity curves, add optical Glow to 'ArtCraft Brand Mark', and enable Motion Blur."*
- **Authentic Brand Assets:**
  - [`docs/brand/artcraft-mark.png`](../../brand/artcraft-mark.png) (`1024×1024` RGBA8 first-party brand mark)
  - [`docs/brand/artcraft-logo-white.png`](../../brand/artcraft-logo-white.png) (`3200×333` RGBA8 first-party wordmark)
  - Layered with `Obsidian Stage Plate`, `Cobalt Optical Core` (feathered elliptical mask + Screen blend + Glow), `Orbital Energy Halo` (vector ring with Trim Paths & rotation), `Gold Precision Ring`, and Arabic RTL tagline `نبض الهوية البصرية — استوديو الموشن والمؤثرات`.
- **Autonomy & Reasoning:**
  - `planOrigin`: `"model_generated"` (`presetUsed: false`)
  - `complexityTier`: `"full_creative_composition"` (`12` native steps executed, `planDigest: "356b4c9fc5f49459"`, `singleUndoCoalesced: true`).
- **5-Frame Temporal Sampling & Multimodal Vision Critique:**
  - Sampled 5 frames (`0.0s`, `2.5s`, `5.0s`, `7.46s`, `9.96s`; 3 distinct frame digests across the reveal and settle hold).
  - Multimodal vision (`qwen3.5:4b-q4_K_M`, `visionInspected: true`) evaluated the rendered frames and executed a refinement plan (`after-vision-qc-1792716263071`): velocity curve easing (`inInfluence: 90`) on rotation and enhanced optical `Glow` (`intensity: 1.5`).
- **Visual Deliverables, Animated Video & Editable Round-Trip:**
  - Before Refinement PNG (`t = 2.50s`): `D:/GemiTron/TronCS/AfterTron/docs/images/aftertron/challenge-c-brand-reveal-before-refinement.png` (`468,120` bytes, SHA-256 `c6f5aa9ca7cefeacd4bcce6bcd0721abf6fbf14b39171e199e031379db39091d`, open: `Invoke-Item "D:/GemiTron/TronCS/AfterTron/docs/images/aftertron/challenge-c-brand-reveal-before-refinement.png"`)
  - After Vision Refinement PNG (`t = 2.50s`): `D:/GemiTron/TronCS/AfterTron/docs/images/aftertron/challenge-c-brand-reveal.png` (`498,142` bytes, SHA-256 `14d6454035a9e10d1b1138c0180ef5ab68c4702938afaad5596ce664e3ddda33`, open: `Invoke-Item "D:/GemiTron/TronCS/AfterTron/docs/images/aftertron/challenge-c-brand-reveal.png"`)
  - Animated H.264 MP4 Video (`48` frames @ `12fps`, `960×540`): `D:/GemiTron/TronCS/AfterTron/target/acceptance-renders/aftertron-tron/challenge-c-brand-reveal.mp4` (`502,959` bytes, SHA-256 `3bad13110dae526578d3f082e9ac83023a0194140d377cfc72afe41577feb2eb`, open: `Invoke-Item "D:/GemiTron/TronCS/AfterTron/target/acceptance-renders/aftertron-tron/challenge-c-brand-reveal.mp4"`)
  - Saved & Reopened Editable Project: `D:/GemiTron/TronCS/AfterTron/docs/images/aftertron/challenge-c-brand-reveal.ecproj` (`283,889` bytes, SHA-256 `007d2564af528755b6c94e2c3a796f23be82aa4100f0636c919ed579cdb590d9`)

---

## 4. Live Runtime UI Captures (`LIVE_RUNTIME` Mode)

All three TRON Chat UI screenshots were captured via wgpu from the live `aftertron_tron_tri_app_and_three_creative_productions_acceptance` session with `[LIVE_RUNTIME]` active, displaying the real `qwen3.5:4b-q4_K_M` status, VRAM telemetry, model-generated plan, and 5-frame multimodal vision critique:

1. **Inferno Dark Workspace (`LIVE_RUNTIME`):** [`docs/images/aftertron/tron-chat-dark.png`](../../images/aftertron/tron-chat-dark.png)
2. **Inferno Light Workspace (`LIVE_RUNTIME`):** [`docs/images/aftertron/tron-chat-light.png`](../../images/aftertron/tron-chat-light.png)
3. **Minimized Floating Pill (`LIVE_RUNTIME`):** [`docs/images/aftertron/tron-chat-minimized.png`](../../images/aftertron/tron-chat-minimized.png)

---

## 5. Verification Commands & Quality Gates

- **GemiMCP Unit & Runtime Suite (36/36 passed):**
  ```powershell
  node --test "d:\GemiTron\GemiMCP\test\tron-after-director.test.mjs" "d:\GemiTron\GemiMCP\test\tron-local-runtime.test.mjs"
  ```
- **AfterTron Live Creative Productions & Tri-App Acceptance:**
  ```powershell
  cargo test -p effectcraft-ui-egui --test aftertron_acceptance aftertron_tron_tri_app_and_three_creative_productions_acceptance -- --nocapture
  ```
- **AfterTron 14-Step Technical Regression Acceptance:**
  ```powershell
  cargo test -p effectcraft-ui-egui --test aftertron_acceptance aftertron_14_step_practical_motion_graphics_and_security_acceptance -- --nocapture
  ```
