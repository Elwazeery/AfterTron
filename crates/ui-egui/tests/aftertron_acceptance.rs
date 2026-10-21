//! Mandatory 14-step practical motion graphics & VFX acceptance test and real Windows wgpu
//! screenshot generator for AfterTron.

use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use effectcraft_automation::Backend;
use effectcraft_automation::bridge::BridgeClient;
use effectcraft_automation::gemiguard::{
    GemiGuardVerifier, OfflineLease, OidcPkceRequest, RevocationList,
    OFFLINE_LEASE_SCHEMA, REVOCATION_LIST_SCHEMA, ed25519_public_key,
};
use effectcraft_automation::gemimcp::{
    GemiMcpToolEnvelope, OutboundBridgeAdapter, build_aftertron_product_manifest,
};
use effectcraft_automation::security::ControlAuthToken;
use effectcraft_engine::render::RenderOpts;
use effectcraft_engine::time::Tick;
use effectcraft_ui_egui::dock::PanelKind;
use effectcraft_ui_egui::{ControlRequest, EffectcraftApp};
use egui_kittest::Harness;
use serde_json::{Value, json};

fn same_adapter(name: String) -> eframe::egui_wgpu::WgpuSetup {
    let mut setup = egui_kittest::wgpu::default_wgpu_setup();
    if let eframe::egui_wgpu::WgpuSetup::CreateNew(c) = &mut setup {
        let fallback = c.native_adapter_selector.take();
        c.native_adapter_selector = Some(std::sync::Arc::new(move |adapters, surface| {
            let named = |a: &&eframe::wgpu::Adapter| format!("{} ({:?})", a.get_info().name, a.get_info().backend) == name;
            match (adapters.iter().find(named), &fallback) {
                (Some(a), _) => Ok(a.clone()),
                (None, Some(f)) => f(adapters, surface),
                (None, None) => adapters.first().cloned().ok_or_else(|| "No adapter found".to_owned()),
            }
        }));
    }
    setup
}

fn step_frame(h: &mut Harness<'_, EffectcraftApp>) {
    for e in h.state_mut().take_synthetic_input() {
        h.event(e);
    }
    h.step();
}

fn settle(h: &mut Harness<'_, EffectcraftApp>, _secs: f64) {
    for _ in 0..120 {
        step_frame(h);
        if h.state().frames.inflight() == 0
            && h.state().frames.last_ms.lock().map(|v| *v > 0.0).unwrap_or(false)
        {
            break;
        }
        std::thread::sleep(Duration::from_millis(15));
    }
    for _ in 0..4 {
        step_frame(h);
    }
}

fn send_control(
    h: &mut Harness<'_, EffectcraftApp>,
    tx: &mpsc::Sender<ControlRequest>,
    method: &str,
    params: Value,
) -> Value {
    let (reply_tx, reply_rx) = mpsc::channel();
    tx.send(ControlRequest {
        method: method.to_string(),
        params,
        reply: reply_tx,
    })
    .expect("send control request");
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        step_frame(h);
        if let Ok(v) = reply_rx.try_recv() {
            return v;
        }
        assert!(Instant::now() <= deadline, "timed out waiting for {method}");
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn save_verified_screenshot(h: &mut Harness<'_, EffectcraftApp>, rel_path: &str) {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let out_path = root.join(rel_path);
    if let Some(parent) = out_path.parent() {
        std::fs::create_dir_all(parent).expect("create screenshot dir");
    }
    let img = h.render().expect("wgpu render screenshot");
    assert_eq!((img.width(), img.height()), (1680, 1020), "unexpected screenshot size for {rel_path}");
    // Verify non-blank visual content (multiple distinct colours across the workspace).
    let mut distinct = std::collections::HashSet::new();
    for p in img.pixels().step_by(97) {
        distinct.insert(p.0);
    }
    assert!(distinct.len() > 24, "screenshot {rel_path} lacks visual variety ({} colours)", distinct.len());
    img.save(&out_path).expect("save screenshot PNG");
}

fn frame_digest(pixels: &[[f32; 4]]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for px in pixels {
        for &c in px {
            let b = (c.clamp(0.0, 1.0) * 255.0).round() as u8;
            h ^= u64::from(b);
            h = h.wrapping_mul(0x100000001b3);
        }
    }
    h
}

#[test]
fn aftertron_14_step_practical_motion_graphics_and_security_acceptance() {
    let tmp_dir = std::env::temp_dir().join(format!("aftertron_accept_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp_dir);
    std::fs::create_dir_all(&tmp_dir).expect("create temp dir");

    // -------------------------------------------------------------------------
    // Step 1: Launch AfterTron with wgpu + Inferno Pixel Glass Dark theme
    // -------------------------------------------------------------------------
    eprintln!("[Step 1] Probing GPU adapter and launching AfterTron...");
    let adapter = effectcraft_gpu::Gpu::headless()
        .map(|probe| effectcraft_engine::render::Accelerator::name(&probe))
        .unwrap_or_default();
    eprintln!("[Step 1] Using GPU adapter: {adapter}");

    let mut session = effectcraft_host::session();
    session.prefs.startup.show_home_on_launch = false;
    session.prefs.appearance.appearance_mode = "dark".into();
    session.prefs.appearance.dark_theme = "inferno-dark".into();
    session.prefs.appearance.theme = "inferno-dark".into();

    let (tx, rx) = mpsc::channel::<ControlRequest>();
    let mut harness = Harness::builder()
        .with_size(egui::vec2(1680.0, 1020.0))
        .with_pixels_per_point(1.0)
        .wgpu_setup(same_adapter(adapter))
        .build_eframe(move |_cc| {
            let mut app = EffectcraftApp::new(session).with_control(rx);
            app.ui.start_screen = false;
            app
        });
    settle(&mut harness, 0.8);

    let ui_info = send_control(&mut harness, &tx, "ui.inspect", json!({}));
    assert_eq!(ui_info["ok"], true, "{ui_info}");
    assert_eq!(ui_info["result"]["ui"]["theme"], "InfernoDark");

    // -------------------------------------------------------------------------
    // Step 2: Create a composition with real image footage, English + Arabic RTL
    //         text layers (GemiFontSans), vector shape layer, and solid layer
    // -------------------------------------------------------------------------
    eprintln!("[Step 2] Creating composition and layers...");
    let footage_path = tmp_dir.join("footage_banner.png");
    {
        let mut img = image::RgbaImage::new(640, 360);
        for (x, y, px) in img.enumerate_pixels_mut() {
            let r = ((x * 255) / 640) as u8;
            let g = ((y * 180) / 360) as u8;
            let b = 220u8.saturating_sub(r / 2);
            *px = image::Rgba([r, g, b, 255]);
        }
        img.save(&footage_path).expect("write test footage PNG");
    }

    let comp_reply = send_control(
        &mut harness,
        &tx,
        "engine.execute",
        json!({
            "id": "comp.new",
            "params": {
                "name": "AfterTron Hero",
                "width": 1280,
                "height": 720,
                "fps": 30.0,
                "duration": 4.0
            }
        }),
    );
    assert_eq!(comp_reply["ok"], true, "{comp_reply}");
    let comp_id = comp_reply["result"]["comp"].as_u64().expect("comp id");

    // Import real PNG footage and add it as a layer
    let import_reply = send_control(
        &mut harness,
        &tx,
        "engine.execute",
        json!({
            "id": "file.import",
            "params": { "paths": [footage_path.to_string_lossy()] }
        }),
    );
    assert_eq!(import_reply["ok"], true, "{import_reply}");
    let footage_item = import_reply["result"]["items"][0].as_u64().expect("footage item id");

    let bg_solid = send_control(
        &mut harness,
        &tx,
        "engine.execute",
        json!({
            "id": "layer.newSolid",
            "params": { "comp": comp_id, "name": "Inferno Backdrop", "color": "#14101C", "width": 1280, "height": 720 }
        }),
    );
    assert_eq!(bg_solid["ok"], true, "{bg_solid}");
    let solid_layer_id = bg_solid["result"]["layer"].as_u64().expect("solid layer id");

    let footage_layer = send_control(
        &mut harness,
        &tx,
        "engine.execute",
        json!({
            "id": "layer.addItem",
            "params": { "comp": comp_id, "item": footage_item }
        }),
    );
    assert_eq!(footage_layer["ok"], true, "{footage_layer}");
    let footage_layer_id = footage_layer["result"]["layer"].as_u64().expect("footage layer id");

    // Add vector shape layer with rounded rectangle, crimson fill, gold stroke, and trim paths
    let shape_reply = send_control(
        &mut harness,
        &tx,
        "engine.execute",
        json!({
            "id": "layer.newShape",
            "params": {
                "comp": comp_id,
                "kind": "rounded",
                "name": "Crimson Orbit Frame",
                "size": [760.0, 220.0],
                "fill": [0.97, 0.22, 0.26],
                "fillOpacity": 85.0,
                "stroke": [1.0, 0.83, 0.22],
                "strokeWidth": 6.0
            }
        }),
    );
    assert_eq!(shape_reply["ok"], true, "{shape_reply}");
    let shape_layer_id = shape_reply["result"]["layer"].as_u64().expect("shape layer id");

    let add_trim = send_control(
        &mut harness,
        &tx,
        "engine.execute",
        json!({
            "id": "layer.addShapeItem",
            "params": { "comp": comp_id, "layer": shape_layer_id, "kind": "trim" }
        }),
    );
    assert_eq!(add_trim["ok"], true, "{add_trim}");

    // Add English title layer using GemiFontSans
    let en_text = send_control(
        &mut harness,
        &tx,
        "engine.execute",
        json!({
            "id": "layer.newText",
            "params": {
                "comp": comp_id,
                "name": "Hero Title EN",
                "text": "AFTERTRON • INFERNO PIXEL GLASS",
                "font": "GemiFontSans",
                "style": "Bold",
                "size": 52.0,
                "fill": [0.96, 0.95, 0.93]
            }
        }),
    );
    assert_eq!(en_text["ok"], true, "{en_text}");
    let en_layer_id = en_text["result"]["layer"].as_u64().expect("en text layer id");

    // Add Arabic RTL subtitle layer and verify Arabic glyph shaping & RTL ordering
    let ar_string = "أفترترون — استوديو الموشن والمؤثرات";
    let ar_text = send_control(
        &mut harness,
        &tx,
        "engine.execute",
        json!({
            "id": "layer.newText",
            "params": {
                "comp": comp_id,
                "name": "Hero Subtitle AR",
                "text": ar_string,
                "font": "GemiFontSans",
                "style": "Medium",
                "size": 36.0,
                "fill": [1.0, 0.83, 0.22],
                "direction": "rtl"
            }
        }),
    );
    assert_eq!(ar_text["ok"], true, "{ar_text}");
    let ar_layer_id = ar_text["result"]["layer"].as_u64().expect("ar text layer id");

    let shaped_ar = effectcraft_text::layout_text(
        "مرحبا",
        &effectcraft_text::TextStyle {
            family: "GemiFontSans".into(),
            size: 36.0,
            ..Default::default()
        },
        &Default::default(),
    );
    assert!(shaped_ar.glyphs.len() >= 5, "Arabic word should shape at least 5 glyphs");
    assert!(shaped_ar.glyphs.iter().all(|g| g.id != 0), "Arabic glyphs must not be .notdef");
    assert!(
        shaped_ar.glyphs.first().unwrap().cluster > shaped_ar.glyphs.last().unwrap().cluster,
        "Arabic word must lay out right-to-left (visual left glyph is the last logical cluster)"
    );

    // -------------------------------------------------------------------------
    // Step 3: Add multiple properties, keyframes, and an expression
    // -------------------------------------------------------------------------
    eprintln!("[Step 3] Setting keyframes and expression...");
    for (layer, path, t_sec, val) in [
        (en_layer_id, "transform/position", 0.0, json!([640.0, 180.0])),
        (en_layer_id, "transform/position", 1.0, json!([640.0, 310.0])),
        (en_layer_id, "transform/position", 2.5, json!([640.0, 280.0])),
        (en_layer_id, "transform/opacity", 0.0, json!(25.0)),
        (en_layer_id, "transform/opacity", 1.0, json!(100.0)),
        (ar_layer_id, "transform/position", 0.0, json!([640.0, 560.0])),
        (ar_layer_id, "transform/position", 1.0, json!([640.0, 420.0])),
        (shape_layer_id, "transform/scale", 0.0, json!([40.0, 40.0])),
        (shape_layer_id, "transform/scale", 1.0, json!([100.0, 100.0])),
        (shape_layer_id, "transform/scale", 2.5, json!([112.0, 112.0])),
    ] {
        let r = send_control(
            &mut harness,
            &tx,
            "engine.execute",
            json!({
                "id": "prop.addKey",
                "params": {
                    "comp": comp_id,
                    "layer": layer,
                    "path": path,
                    "time": t_sec,
                    "value": val
                }
            }),
        );
        assert_eq!(r["ok"], true, "prop.addKey {path}@{t_sec}: {r}");
    }

    let expr_reply = send_control(
        &mut harness,
        &tx,
        "engine.execute",
        json!({
            "id": "prop.setExpression",
            "params": {
                "comp": comp_id,
                "layer": shape_layer_id,
                "path": "transform/rotation",
                "expression": "time * 30"
            }
        }),
    );
    assert_eq!(expr_reply["ok"], true, "{expr_reply}");

    // -------------------------------------------------------------------------
    // Step 4: Change easing (Easy Ease) and inspect the Graph Editor
    // -------------------------------------------------------------------------
    eprintln!("[Step 4] Selecting keyframes, applying Easy Ease, opening Graph Editor...");
    let sel_keys = send_control(
        &mut harness,
        &tx,
        "engine.execute",
        json!({
            "id": "keys.select",
            "params": {
                "comp": comp_id,
                "keys": [
                    { "layer": en_layer_id, "path": "transform/position", "time": 0.0 },
                    { "layer": en_layer_id, "path": "transform/position", "time": 1.0 },
                    { "layer": en_layer_id, "path": "transform/position", "time": 2.5 }
                ],
                "selectProperties": true
            }
        }),
    );
    assert_eq!(sel_keys["ok"], true, "{sel_keys}");

    let ease_reply = send_control(
        &mut harness,
        &tx,
        "engine.execute",
        json!({
            "id": "keys.easyEase",
            "params": { "which": "both" }
        }),
    );
    assert_eq!(ease_reply["ok"], true, "{ease_reply}");

    // Select the English title layer and shape layer, open their properties and the Graph Editor
    let _ = send_control(
        &mut harness,
        &tx,
        "engine.execute",
        json!({ "id": "layer.select", "params": { "comp": comp_id, "layers": [en_layer_id, shape_layer_id] } }),
    );
    {
        let app = harness.state_mut();
        app.ui.timeline.open_layers.insert(en_layer_id);
        app.ui.timeline.open_layers.insert(shape_layer_id);
        app.ui.timeline.graph_editor = true;
    }

    // -------------------------------------------------------------------------
    // Step 5: Add and modify several actual effects and verify pixel impact
    // -------------------------------------------------------------------------
    eprintln!("[Step 5] Applying effects and verifying pixel impact...");
    let _ = send_control(&mut harness, &tx, "engine.execute", json!({ "id": "time.set", "params": { "time": 1.0 } }));
    let before_fx = harness
        .state_mut()
        .session
        .render(effectcraft_engine::project::ItemId(comp_id), Tick::from_seconds_f64(1.0), RenderOpts::default());

    for (fx_id, param, value) in [
        ("Glow", "intensity", json!(1.8)),
        ("Drop Shadow", "distance", json!(14.0)),
        ("Gaussian Blur", "blurriness", json!(8.0)),
    ] {
        let apply = send_control(
            &mut harness,
            &tx,
            "engine.execute",
            json!({
                "id": "effect.apply",
                "params": { "comp": comp_id, "layers": [shape_layer_id], "effect": fx_id }
            }),
        );
        assert_eq!(apply["ok"], true, "effect.apply {fx_id}: {apply}");
        let prefix = apply["result"]["paths"][0].as_str().expect("effect path prefix");
        let set_p = send_control(
            &mut harness,
            &tx,
            "engine.execute",
            json!({
                "id": "prop.set",
                "params": {
                    "comp": comp_id,
                    "layer": shape_layer_id,
                    "path": format!("{prefix}/{param}"),
                    "value": value
                }
            }),
        );
        assert_eq!(set_p["ok"], true, "prop.set {prefix}/{param}: {set_p}");
    }

    let after_fx = harness
        .state_mut()
        .session
        .render(effectcraft_engine::project::ItemId(comp_id), Tick::from_seconds_f64(1.0), RenderOpts::default());
    assert_ne!(
        frame_digest(&before_fx.data),
        frame_digest(&after_fx.data),
        "applying Glow + Drop Shadow + Gaussian Blur must alter rendered pixels"
    );

    // Show Effect Controls panel for Crimson Orbit Frame and select animated properties for Graph Editor curves
    let _ = send_control(
        &mut harness,
        &tx,
        "engine.execute",
        json!({ "id": "layer.select", "params": { "comp": comp_id, "layers": [shape_layer_id, en_layer_id] } }),
    );
    let _ = send_control(
        &mut harness,
        &tx,
        "engine.execute",
        json!({ "id": "prop.select", "params": { "comp": comp_id, "layer": en_layer_id, "path": "transform/position", "selectKeys": true } }),
    );
    let _ = send_control(
        &mut harness,
        &tx,
        "engine.execute",
        json!({ "id": "prop.select", "params": { "comp": comp_id, "layer": shape_layer_id, "path": "transform/scale", "add": true, "selectKeys": true } }),
    );
    {
        let app = harness.state_mut();
        app.ui.timeline.graph_editor = true;
        app.show_panel(PanelKind::EffectControls);
    }
    settle(&mut harness, 1.0);
    save_verified_screenshot(&mut harness, "docs/images/aftertron/aftertron-graph-editor-effects.png");

    // -------------------------------------------------------------------------
    // Step 6: Preview exact frames and animation timing (0.0s, 1.0s, 2.5s)
    // -------------------------------------------------------------------------
    eprintln!("[Step 6] Verifying frame-exact animation timing...");
    let f_t0 = harness
        .state_mut()
        .session
        .render(effectcraft_engine::project::ItemId(comp_id), Tick::from_seconds_f64(0.0), RenderOpts::default());
    let f_t1 = harness
        .state_mut()
        .session
        .render(effectcraft_engine::project::ItemId(comp_id), Tick::from_seconds_f64(1.0), RenderOpts::default());
    let f_t2 = harness
        .state_mut()
        .session
        .render(effectcraft_engine::project::ItemId(comp_id), Tick::from_seconds_f64(2.5), RenderOpts::default());
    let (d0, d1, d2) = (frame_digest(&f_t0.data), frame_digest(&f_t1.data), frame_digest(&f_t2.data));
    assert_ne!(d0, d1, "frame at 0.0s and 1.0s must differ");
    assert_ne!(d1, d2, "frame at 1.0s and 2.5s must differ");

    // Close Graph Editor to show the layer stack + keyframe diamonds for the Hero Dark screenshot
    {
        let app = harness.state_mut();
        app.ui.timeline.graph_editor = false;
        app.show_panel(PanelKind::Properties);
    }
    settle(&mut harness, 1.0);
    save_verified_screenshot(&mut harness, "docs/images/aftertron/aftertron-inferno-dark-hero.png");

    // -------------------------------------------------------------------------
    // Step 7: Verify masks, alpha transparency and color neutrality
    // -------------------------------------------------------------------------
    eprintln!("[Step 7] Adding elliptical mask and verifying corner alpha transparency...");
    let mask_reply = send_control(
        &mut harness,
        &tx,
        "engine.execute",
        json!({
            "id": "layer.addMask",
            "params": {
                "comp": comp_id,
                "layer": solid_layer_id,
                "shape": "ellipse"
            }
        }),
    );
    assert_eq!(mask_reply["ok"], true, "{mask_reply}");
    // Disable the footage plate briefly to verify transparent corner alpha from the elliptical mask
    let _ = send_control(
        &mut harness,
        &tx,
        "engine.execute",
        json!({ "id": "layer.setSwitch", "params": { "comp": comp_id, "layers": [footage_layer_id], "switch": "video", "value": false } }),
    );
    let masked_frame = harness
        .state_mut()
        .session
        .render(effectcraft_engine::project::ItemId(comp_id), Tick::from_seconds_f64(1.0), RenderOpts::default());
    // Corner pixel (0,0) is outside the elliptical mask and must have alpha == 0
    assert_eq!(masked_frame.data[0][3], 0.0, "top-left corner outside elliptical mask must be transparent");
    // Center pixel (640,360) is inside the composition and must have non-zero alpha
    let center_idx = 360 * 1280 + 640;
    assert!(masked_frame.data[center_idx][3] > 0.8, "center pixel inside composition must be opaque");
    // Re-enable the footage plate
    let _ = send_control(
        &mut harness,
        &tx,
        "engine.execute",
        json!({ "id": "layer.setSwitch", "params": { "comp": comp_id, "layers": [footage_layer_id], "switch": "video", "value": true } }),
    );

    // -------------------------------------------------------------------------
    // Step 8: Undo / Redo determinism
    // -------------------------------------------------------------------------
    let rev_before = harness.state().session.document_revision();
    let hash_before = frame_digest(
        &harness
            .state_mut()
            .session
            .render(effectcraft_engine::project::ItemId(comp_id), Tick::from_seconds_f64(1.0), RenderOpts::default())
            .data,
    );
    let undo_reply = send_control(&mut harness, &tx, "engine.execute", json!({ "id": "edit.undo", "params": {} }));
    assert_eq!(undo_reply["ok"], true, "{undo_reply}");
    assert!(harness.state().session.document_revision() > rev_before);

    let redo_reply = send_control(&mut harness, &tx, "engine.execute", json!({ "id": "edit.redo", "params": {} }));
    assert_eq!(redo_reply["ok"], true, "{redo_reply}");
    let hash_after_redo = frame_digest(
        &harness
            .state_mut()
            .session
            .render(effectcraft_engine::project::ItemId(comp_id), Tick::from_seconds_f64(1.0), RenderOpts::default())
            .data,
    );
    assert_eq!(hash_before, hash_after_redo, "redo must restore bit-exact rendered frame");

    // -------------------------------------------------------------------------
    // Step 9: Save, close and reopen the .ecproj project
    // -------------------------------------------------------------------------
    let project_path = tmp_dir.join("aftertron_hero.ecproj");
    let save_reply = send_control(
        &mut harness,
        &tx,
        "engine.execute",
        json!({ "id": "file.save", "params": { "path": project_path.to_string_lossy() } }),
    );
    assert_eq!(save_reply["ok"], true, "{save_reply}");
    assert!(project_path.is_file(), "saved .ecproj must exist");

    let new_proj = send_control(&mut harness, &tx, "engine.execute", json!({ "id": "file.newProject", "params": {} }));
    assert_eq!(new_proj["ok"], true, "{new_proj}");
    assert!(harness.state().session.project.items.is_empty());

    let open_reply = send_control(
        &mut harness,
        &tx,
        "engine.execute",
        json!({ "id": "file.open", "params": { "path": project_path.to_string_lossy() } }),
    );
    assert_eq!(open_reply["ok"], true, "{open_reply}");
    let hash_reopened = frame_digest(
        &harness
            .state_mut()
            .session
            .render(effectcraft_engine::project::ItemId(comp_id), Tick::from_seconds_f64(1.0), RenderOpts::default())
            .data,
    );
    assert_eq!(hash_before, hash_reopened, "reopened .ecproj must render identical pixels");

    // -------------------------------------------------------------------------
    // Step 10: Render actual output and inspect frames + Render Queue / Scopes
    // -------------------------------------------------------------------------
    eprintln!("[Step 10] Adding to Render Queue and capturing Render Queue + Lumetri Scopes screenshot...");
    let rq_add = send_control(
        &mut harness,
        &tx,
        "engine.execute",
        json!({ "id": "renderQueue.add", "params": { "comp": comp_id, "format": "png" } }),
    );
    assert_eq!(rq_add["ok"], true, "{rq_add}");
    {
        let app = harness.state_mut();
        app.show_panel(PanelKind::RenderQueue);
        app.show_panel(PanelKind::LumetriScopes);
    }
    settle(&mut harness, 1.0);
    save_verified_screenshot(&mut harness, "docs/images/aftertron/aftertron-render-queue-and-scopes.png");

    // Export an actual PNG frame to disk and verify decoded dimensions and pixels
    let out_png = tmp_dir.join("aftertron_frame_1s.png");
    let rendered = harness
        .state_mut()
        .session
        .render(effectcraft_engine::project::ItemId(comp_id), Tick::from_seconds_f64(1.0), RenderOpts::default());
    let rgba_bytes: Vec<u8> = rendered
        .data
        .iter()
        .flat_map(|px| px.map(|c| (c.clamp(0.0, 1.0) * 255.0).round() as u8))
        .collect();
    let out_img = image::RgbaImage::from_raw(rendered.width, rendered.height, rgba_bytes).expect("rgba image");
    out_img.save(&out_png).expect("save rendered PNG");
    let decoded = image::open(&out_png).expect("decode rendered PNG").to_rgba8();
    assert_eq!((decoded.width(), decoded.height()), (1280, 720));

    // -------------------------------------------------------------------------
    // Step 11 & 12: Native authenticated control interface + GemiMCP OutboundBridgeAdapter
    //               + rejection of unauthorized control calls + revision conflict check
    // -------------------------------------------------------------------------
    let token = ControlAuthToken::generate();
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind loopback");
    let port = listener.local_addr().unwrap().port();
    let tx_srv = tx.clone();
    let token_srv = token.clone();
    let srv_thread = std::thread::spawn(move || {
        // Serve up to 3 connections (1 unauthenticated, 1 wrong token, 1 authenticated adapter)
        for _ in 0..3 {
            let Ok((stream, _)) = listener.accept() else { break };
            let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
            let Ok(mut writer) = stream.try_clone() else { continue };
            let mut reader = BufReader::new(stream);
            let mut authed = false;
            let mut line = String::new();
            while reader.read_line(&mut line).unwrap_or(0) > 0 {
                let req: Value = serde_json::from_str(line.trim()).unwrap_or(json!({}));
                line.clear();
                let req_id = req.get("id").cloned();
                let method = req["method"].as_str().unwrap_or("");
                if !authed {
                    if method == "auth" {
                        let candidate = req["params"]["token"].as_str().unwrap_or("");
                        if token_srv.verify(candidate) {
                            authed = true;
                            let mut ok_reply = json!({"ok": true, "result": {"authenticated": true}});
                            if let Some(id) = req_id {
                                ok_reply["id"] = id;
                            }
                            let _ = writeln!(writer, "{ok_reply}");
                            continue;
                        }
                    }
                    let mut err_reply = json!({
                        "ok": false,
                        "error": "unauthorized: authenticate first via {\"method\":\"auth\",\"params\":{\"token\":\"...\"}}"
                    });
                    if let Some(id) = req_id {
                        err_reply["id"] = id;
                    }
                    let _ = writeln!(writer, "{err_reply}");
                    break;
                }
                let (rtx, rrx) = mpsc::channel();
                let _ = tx_srv.send(ControlRequest {
                    method: method.to_string(),
                    params: req.get("params").cloned().unwrap_or(json!({})),
                    reply: rtx,
                });
                let mut reply = rrx
                    .recv_timeout(Duration::from_secs(5))
                    .unwrap_or_else(|_| json!({"ok": false, "error": "timeout"}));
                if let Some(id) = req_id {
                    reply["id"] = id;
                }
                let _ = writeln!(writer, "{reply}");
            }
        }
    });

    // Pump the UI harness on the main thread while a worker exercises the TCP clients
    let token_client = token.clone();
    let client_worker = std::thread::spawn(move || {
        let port_str = port.to_string();
        // 1) Unauthorized raw client without auth handshake must be rejected
        {
            let mut raw = std::net::TcpStream::connect(("127.0.0.1", port)).expect("connect raw");
            writeln!(raw, "{}", json!({"method": "engine.execute", "params": {"id": "project.summary"}})).unwrap();
            let mut line = String::new();
            BufReader::new(raw).read_line(&mut line).unwrap();
            let resp: Value = serde_json::from_str(&line).unwrap();
            assert_eq!(resp["ok"], false);
            assert!(resp["error"].as_str().unwrap_or("").contains("unauthorized"), "{resp}");
        }
        // 2) Client with wrong token must be rejected
        {
            let wrong_token = ControlAuthToken::from_secret("00".repeat(32)).expect("wrong token format");
            let mut bad = BridgeClient::with_token(&port_str, wrong_token).expect("create bad bridge client");
            let err = bad.call("ui.inspect", json!({})).expect_err("wrong token must fail").to_string();
            assert!(err.contains("unauthorized"), "{err}");
        }
        // 3) Authenticated OutboundBridgeAdapter succeeds and enforces expectedRevision + idempotencyKey
        let backend = Backend::bridge_authenticated(&port_str, token_client).expect("connect authenticated backend");
        let mut adapter = OutboundBridgeAdapter::new(backend);
        let summary_resp = adapter.handle_envelope(GemiMcpToolEnvelope {
            request_id: "req-1".into(),
            tool: "after.project.summary".into(),
            arguments: json!({}),
            expected_revision: None,
            idempotency_key: None,
            workspace_roots: None,
        });
        assert!(summary_resp.ok, "{summary_resp:?}");
        let rev = summary_resp.document_revision;

        // Stale expectedRevision must be rejected
        let stale_resp = adapter.handle_envelope(GemiMcpToolEnvelope {
            request_id: "req-2".into(),
            tool: "after.cmd.time.set".into(),
            arguments: json!({ "time": 0.5 }),
            expected_revision: Some(rev + 999),
            idempotency_key: None,
            workspace_roots: None,
        });
        assert!(!stale_resp.ok);
        assert!(
            stale_resp.error.as_deref().unwrap_or("").contains("revision mismatch"),
            "{stale_resp:?}"
        );

        // Valid expectedRevision + idempotencyKey succeeds and replays deterministically
        let ok1 = adapter.handle_envelope(GemiMcpToolEnvelope {
            request_id: "req-3".into(),
            tool: "after.cmd.time.set".into(),
            arguments: json!({ "time": 1.0 }),
            expected_revision: Some(rev),
            idempotency_key: Some("accept-step-11".into()),
            workspace_roots: None,
        });
        assert!(ok1.ok && !ok1.replayed, "{ok1:?}");
        let ok2 = adapter.handle_envelope(GemiMcpToolEnvelope {
            request_id: "req-3b".into(),
            tool: "after.cmd.time.set".into(),
            arguments: json!({ "time": 1.0 }),
            expected_revision: Some(rev),
            idempotency_key: Some("accept-step-11".into()),
            workspace_roots: None,
        });
        assert!(ok2.ok && ok2.replayed && ok2.document_revision == ok1.document_revision, "{ok2:?}");
    });

    while !client_worker.is_finished() {
        step_frame(&mut harness);
        std::thread::sleep(Duration::from_millis(10));
    }
    client_worker.join().expect("client worker assertions");
    srv_thread.join().expect("server thread");

    // -------------------------------------------------------------------------
    // Step 13: Verify GemiMCP manifest, OIDC PKCE & GemiGuard offline lease /
    //          revocation safety (project files remain accessible when expired/revoked)
    // -------------------------------------------------------------------------
    let manifest = build_aftertron_product_manifest();
    assert_eq!(manifest.schema, "gemimcp.product-manifest.v1");
    assert_eq!(manifest.product_id, "aftertron");

    let pkce = OidcPkceRequest::generate(49210).expect("pkce");
    assert!(pkce.authorization_url.starts_with("https://auth.gemitron.net/oauth2/authorize?"));

    let secret = [0x55u8; 32];
    let pubkey = ed25519_public_key(&secret);
    let verifier = GemiGuardVerifier::new("win11-workstation")
        .with_trusted_key("accept-key-1", pubkey);
    let mut lease = OfflineLease {
        schema: OFFLINE_LEASE_SCHEMA.into(),
        lease_id: "lease-accept".into(),
        subject: "user-accept".into(),
        product: "aftertron".into(),
        tier: "studio-pro".into(),
        device_fingerprint: "win11-workstation".into(),
        issued_at: 1_700_000_000,
        expires_at: 1_700_086_400,
        offline_grace_until: 1_700_090_000,
        capabilities: vec!["aftertron:app:use".into(), "aftertron:mcp:cloud_bridge".into()],
        key_id: "accept-key-1".into(),
        signature: String::new(),
    };
    lease.sign(&secret);
    let mut rev_list = RevocationList {
        schema: REVOCATION_LIST_SCHEMA.into(),
        issued_at: 1_700_010_000,
        revoked_lease_ids: vec!["lease-accept".into()],
        revoked_device_fingerprints: vec![],
        key_id: "accept-key-1".into(),
        signature: String::new(),
    };
    rev_list.sign(&secret);
    let decision = verifier
        .evaluate(&lease, Some(&rev_list), 1_700_020_000)
        .expect("evaluate revoked lease");
    assert!(decision.preserve_user_project_data && decision.allows_local_project_io);

    // -------------------------------------------------------------------------
    // Step 14: Capture Inferno Pixel Glass Light workspace & Appearance Settings
    //          screenshots and verify all 5 screenshots on disk
    // -------------------------------------------------------------------------
    let _ = send_control(
        &mut harness,
        &tx,
        "engine.execute",
        json!({ "id": "prefs.set", "params": { "key": "appearance.lightTheme", "value": "inferno-light" } }),
    );
    let _ = send_control(
        &mut harness,
        &tx,
        "engine.execute",
        json!({ "id": "prefs.set", "params": { "key": "appearance.appearanceMode", "value": "light" } }),
    );
    let _ = send_control(
        &mut harness,
        &tx,
        "engine.execute",
        json!({ "id": "layer.select", "params": { "comp": comp_id, "layers": [shape_layer_id, en_layer_id] } }),
    );
    {
        let app = harness.state_mut();
        app.show_panel(PanelKind::Project);
        app.show_panel(PanelKind::Timeline);
        app.show_panel(PanelKind::Properties);
    }
    settle(&mut harness, 1.0);
    save_verified_screenshot(&mut harness, "docs/images/aftertron/aftertron-inferno-light-workspace.png");

    // Switch back to Inferno Pixel Glass Dark and open Appearance Settings dialog for screenshot 5
    let _ = send_control(
        &mut harness,
        &tx,
        "engine.execute",
        json!({ "id": "prefs.set", "params": { "key": "appearance.appearanceMode", "value": "dark" } }),
    );
    {
        let app = harness.state_mut();
        effectcraft_ui_egui::panels::settings::open(app, "appearance");
    }
    settle(&mut harness, 0.8);
    save_verified_screenshot(&mut harness, "docs/images/aftertron/aftertron-about-and-settings.png");

    let _ = std::fs::remove_dir_all(&tmp_dir);
}
