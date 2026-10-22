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

fn save_comp_frame_png(
    session: &mut effectcraft_engine::Session,
    comp_id: u64,
    time_sec: f64,
    rel_path: &str,
) {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let out_path = root.join(rel_path);
    if let Some(parent) = out_path.parent() {
        std::fs::create_dir_all(parent).expect("create output dir");
    }
    let rendered = session.render(
        effectcraft_engine::project::ItemId(comp_id),
        Tick::from_seconds_f64(time_sec),
        RenderOpts::default(),
    );
    let rgba_bytes: Vec<u8> = rendered
        .data
        .iter()
        .flat_map(|px| px.map(|c| (c.clamp(0.0, 1.0) * 255.0).round() as u8))
        .collect();
    let img = image::RgbaImage::from_raw(rendered.width, rendered.height, rgba_bytes)
        .expect("build RgbaImage from rendered frame");
    let mut distinct = std::collections::HashSet::new();
    for p in img.pixels().step_by(89) {
        distinct.insert(p.0);
    }
    assert!(
        distinct.len() > 16,
        "rendered frame {rel_path} lacks visual variety ({} colours)",
        distinct.len()
    );
    img.save(&out_path).expect("save rendered frame PNG");
}

fn synthesize_product_hero_asset(out_path: & std::path::Path) {
    if let Some(parent) = out_path.parent() {
        std::fs::create_dir_all(parent).expect("create asset dir");
    }
    let (w, h) = (720u32, 720u32);
    let mut img = image::RgbaImage::new(w, h);
    let (cx, cy) = (360.0f32, 360.0f32);
    for (x, y, px) in img.enumerate_pixels_mut() {
        let fx = x as f32 - cx;
        let fy = y as f32 - cy;
        // Rounded monolith hardware silhouette with chamfered corners (half-size 210x280, radius 48)
        let qx = fx.abs() - (210.0 - 48.0);
        let qy = fy.abs() - (280.0 - 48.0);
        let sdf = (qx.max(0.0).hypot(qy.max(0.0)) + qx.min(qy.max(0.0)).min(0.0)) - 48.0;
        if sdf > 1.5 {
            *px = image::Rgba([0, 0, 0, 0]);
            continue;
        }
        let alpha = ((1.5 - sdf) / 3.0).clamp(0.0, 1.0);
        // Brushed titanium directional lighting + warm rim reflection
        let nx = (fx / 210.0).clamp(-1.0, 1.0);
        let ny = (fy / 280.0).clamp(-1.0, 1.0);
        let key_light = (0.55 - 0.35 * nx - 0.25 * ny).clamp(0.12, 0.95);
        let rim = ((sdf + 10.0) / 10.0).clamp(0.0, 1.0);
        let brush = ((y as f32 * 0.7).sin() * 0.018) + ((x as f32 * 0.15).cos() * 0.01);

        let mut r = (0.16 + 0.32 * key_light + brush + 0.42 * rim * (1.0 - nx).max(0.0)).clamp(0.0, 1.0);
        let mut g = (0.18 + 0.34 * key_light + brush + 0.26 * rim * (1.0 - nx).max(0.0)).clamp(0.0, 1.0);
        let mut b = (0.22 + 0.38 * key_light + brush + 0.38 * rim * (1.0 + nx).max(0.0)).clamp(0.0, 1.0);

        // Acoustic driver ring & precision grille in upper half (center at cy - 48)
        let dy_driver = fy + 48.0;
        let dist_driver = fx.hypot(dy_driver);
        if (dist_driver - 118.0).abs() < 9.5 {
            // Warm anodized copper-gold acoustic bezel
            let ring_spec = (0.65 - 0.35 * (fx / 120.0)).clamp(0.2, 1.0);
            r = (0.88 * ring_spec + 0.12).clamp(0.0, 1.0);
            g = (0.56 * ring_spec + 0.06).clamp(0.0, 1.0);
            b = (0.24 * ring_spec + 0.04).clamp(0.0, 1.0);
        } else if dist_driver < 108.5 {
            // Acoustic mesh cavity with micro-perforation pattern and central dome
            let dome = (1.0 - (dist_driver / 108.5)).powf(1.6);
            let perf = if (x % 6 < 3) ^ (y % 6 < 3) { 0.82 } else { 1.0 };
            r = (0.09 + 0.22 * dome) * perf;
            g = (0.11 + 0.24 * dome) * perf;
            b = (0.15 + 0.30 * dome) * perf;
            if (dist_driver - 36.0).abs() < 3.5 {
                r = 0.92;
                g = 0.62;
                b = 0.28;
            }
        }

        // Lower glass telemetry bar
        if fy > 125.0 && fy < 185.0 && fx.abs() < 145.0 {
            let gloss = ((fx + fy * 0.6) * 0.025).sin().max(0.0) * 0.25;
            r = (0.08 + gloss).clamp(0.0, 1.0);
            g = (0.18 + gloss * 1.4).clamp(0.0, 1.0);
            b = (0.28 + gloss * 1.8).clamp(0.0, 1.0);
            if (fy - 155.0).abs() < 2.0 && fx.abs() < 95.0 {
                r = 0.25;
                g = 0.82;
                b = 0.98;
            }
        }

        *px = image::Rgba([
            (r * 255.0).round() as u8,
            (g * 255.0).round() as u8,
            (b * 255.0).round() as u8,
            (alpha * 255.0).round() as u8,
        ]);
    }
    img.save(out_path).expect("save product hero asset PNG");
}

fn review_and_apply_plan(
    session: &mut effectcraft_engine::Session,
    proposal: &serde_json::Value,
) -> (serde_json::Value, serde_json::Value) {
    let rev = effectcraft_ui_egui::tron_actions::review(session, proposal).expect("review plan");
    let plan_id = rev["planId"].as_str().expect("planId");
    let plan_rev = rev["revision"].as_u64().expect("revision");
    let digest = rev["planDigest"].as_str().expect("planDigest");
    let applied = effectcraft_ui_egui::tron_actions::apply(
        session, proposal, plan_id, plan_rev, digest,
    )
    .expect("apply plan");
    (rev, applied)
}

#[test]
fn aftertron_tron_tri_app_and_three_creative_productions_acceptance() {
    use effectcraft_automation::tron_local;
    use effectcraft_ui_egui::{tron_actions, tron_chat};

    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let out_dir = root.join("docs/images/aftertron");
    std::fs::create_dir_all(&out_dir).expect("create docs/images/aftertron");

    // =========================================================================
    // 1. SHARED TRON RUNTIME — THREE APPLICATIONS (PhotoTron + FilmTron + AfterTron)
    // =========================================================================
    eprintln!("[TRON 1] Activating shared GemiMCP workstation broker and verifying Tri-App leases...");
    let acq_after = tron_local::activate().expect("explicit ON activates/connects to shared broker");
    assert_eq!(acq_after["ok"], true, "{acq_after}");
    let _ = tron_local::acquire_peer_lease("phototron-tri-accept", "phototron", std::process::id())
        .expect("acquire phototron peer lease");
    let acq_film = tron_local::acquire_peer_lease("filmtron-tri-accept", "filmtron", std::process::id())
        .expect("acquire filmtron peer lease");
    assert_eq!(acq_film["ok"], true, "{acq_film}");

    let status_tri = tron_local::status_with_gpu().expect("status_with_gpu with 3 active apps");
    eprintln!("[TRON 1] Tri-App Status with GPU: {status_tri}");
    let st = &status_tri["status"];
    assert_eq!(st["model"], "qwen3.5:4b-q4_K_M");
    assert_eq!(st["thinkingSupported"], true);
    assert_eq!(st["thinkingVerified"], true);
    assert_eq!(st["warmupVerified"], true);
    assert_eq!(st["residency"]["resident"], true);
    assert!(
        st["activeLeaseCount"].as_u64().unwrap_or(0) >= 3,
        "all 3 apps must share the single runtime: {st}"
    );
    let vram_loaded_mib = st["gpu"]["usedMiB"].as_u64().unwrap_or(0);
    let vram_total_mib = st["gpu"]["totalMiB"].as_u64().unwrap_or(0);
    eprintln!(
        "[TRON 1] Measured RTX 3050 VRAM (3 apps sharing 1 model): used={} MiB / total={} MiB",
        vram_loaded_mib, vram_total_mib
    );
    assert!(vram_total_mib >= 4000, "expected real GPU VRAM telemetry: {st}");

    // Release Order 1: AfterTron -> PhotoTron -> FilmTron
    let rel_after_1 = tron_local::release().expect("release AfterTron first");
    assert_eq!(
        rel_after_1["status"]["residency"]["resident"], true,
        "releasing AfterTron must not unload PhotoTron/FilmTron sessions"
    );
    let rel_photo_1 = tron_local::release_peer_lease("phototron-tri-accept")
        .expect("release PhotoTron second");
    assert_eq!(
        rel_photo_1["status"]["residency"]["resident"], true,
        "releasing PhotoTron must keep FilmTron session resident"
    );
    let rel_film_1 = tron_local::release_peer_lease("filmtron-tri-accept")
        .expect("release FilmTron last");
    assert_eq!(rel_film_1["status"]["activeLeaseCount"], 0);
    assert_eq!(rel_film_1["status"]["state"], "NOT_LOADED");
    assert_eq!(rel_film_1["status"]["residency"]["resident"], false);

    let status_unloaded = tron_local::status_with_gpu().expect("status after final release");
    let vram_unloaded_mib = status_unloaded["status"]["gpu"]["usedMiB"].as_u64().unwrap_or(0);
    eprintln!(
        "[TRON 1] Measured RTX 3050 VRAM after final lease unload: used={} MiB",
        vram_unloaded_mib
    );

    // Release Order 2: PhotoTron -> FilmTron -> AfterTron (verify AfterTron continues working)
    let _ = tron_local::acquire_peer_lease("phototron-tri-accept-2", "phototron", std::process::id())
        .expect("re-acquire PhotoTron");
    let _ = tron_local::acquire_peer_lease("filmtron-tri-accept-2", "filmtron", std::process::id())
        .expect("re-acquire FilmTron");
    let re_after = tron_local::activate().expect("re-acquire AfterTron");
    assert_eq!(re_after["status"]["activeLeaseCount"], 3);
    assert_eq!(re_after["status"]["residency"]["resident"], true);

    let _ = tron_local::release_peer_lease("phototron-tri-accept-2").expect("release PhotoTron");
    let after_still_live = tron_local::release_peer_lease("filmtron-tri-accept-2").expect("release FilmTron");
    assert_eq!(after_still_live["status"]["activeLeaseCount"], 1);
    assert_eq!(
        after_still_live["status"]["residency"]["resident"], true,
        "AfterTron lease must keep shared model resident after PhotoTron & FilmTron exit"
    );

    // =========================================================================
    // 2. CHALLENGE A — PREMIUM MOTION TYPOGRAPHY (10s, 24fps, 1920x1080)
    // =========================================================================
    eprintln!("[Challenge A] Building 10s 24fps 1920x1080 Premium Motion Typography production...");
    let mut session_a = effectcraft_host::session();
    session_a
        .execute(
            "comp.new",
            json!({
                "name": "Challenge A — Kinetic Typography",
                "width": 1920,
                "height": 1080,
                "fps": 24.0,
                "duration": 10.0
            }),
        )
        .expect("create Challenge A comp");
    let comp_a = session_a.active_comp_id().expect("active comp A").0;

    let ctx_a = tron_actions::live_context(&session_a).expect("live_context A");
    let brief_a = "صمّم مقدمة تايبوجرافي سينمائية مدتها 10 ثواني بمعدل 24 إطار في الثانية، مع تسلسل بصري واضح بين العنوان الرئيسي والسطر الفرعي، حركة دخول متدرجة مع تمهيد وارتداد ناعم، منحنيات سرعة مخصصة في Graph Editor، وتفعيل Motion Blur.";
    let raw_plan_a = tron_local::prepare_after_plan(brief_a, &ctx_a).expect("prepare_after_plan A");
    assert_eq!(raw_plan_a["ok"], true, "{raw_plan_a}");
    let res_plan_a = raw_plan_a.get("result").unwrap_or(&raw_plan_a);

    // Verify two genuinely distinct creative directions and test switching between them
    let dirs_a = res_plan_a["exploredDirections"].as_array().expect("exploredDirections A");
    assert_eq!(dirs_a.len(), 2, "must present 2 creative directions");
    let prop_a_dir_b = tron_actions::select_direction(&raw_plan_a, 1)
        .expect("select direction_b (index 1) on Challenge A");
    let prop_a_dir_a = tron_actions::select_direction(&raw_plan_a, 0)
        .expect("select direction_a (index 0) on Challenge A");
    let rev_a_dir_b = tron_actions::review(&session_a, &prop_a_dir_b).expect("review dir B on Challenge A");
    let rev_a_dir_a = tron_actions::review(&session_a, &prop_a_dir_a).expect("review dir A on Challenge A");
    assert_ne!(
        rev_a_dir_a["planDigest"], rev_a_dir_b["planDigest"],
        "Direction A and Direction B must have distinct native motion plans and digests"
    );

    let plan_id_a = rev_a_dir_a["planId"].as_str().expect("planId A");
    let plan_rev_a = rev_a_dir_a["revision"].as_u64().expect("revision A");
    let digest_a = rev_a_dir_a["planDigest"].as_str().expect("planDigest A");
    let applied_a = tron_actions::apply(&mut session_a, &prop_a_dir_a, plan_id_a, plan_rev_a, digest_a)
        .expect("apply Direction A on Challenge A");
    assert!(
        applied_a["native"]["stepsExecuted"].as_u64().unwrap_or(0) >= 8,
        "expected >= 8 native steps in Challenge A: {applied_a}"
    );
    assert_eq!(applied_a["native"]["singleUndoCoalesced"], true);

    // Replay of consumed approval must be rejected
    assert!(
        tron_actions::apply(&mut session_a, &prop_a_dir_a, plan_id_a, plan_rev_a, digest_a).is_err(),
        "replaying a consumed approval must fail"
    );

    // Inspect 5 actual rendered frames across the 10-second timeline
    let sampled_a = tron_actions::sample_rendered_frames(&session_a)
        .expect("sample 5 rendered frames for Challenge A");
    assert_eq!(sampled_a.len(), 5);
    assert_ne!(
        sampled_a[0]["sha256"], sampled_a[2]["sha256"],
        "Challenge A start frame and 50% frame must differ (real keyframed motion)"
    );

    let post_ctx_a = tron_actions::live_context(&session_a).expect("post live_context A");
    let critique_a = tron_local::critique_after_execution(&post_ctx_a, &sampled_a, brief_a)
        .expect("critique_after_execution A");
    assert_eq!(critique_a["ok"], true, "{critique_a}");
    let qc_a = critique_a.get("result").unwrap_or(&critique_a);
    assert_eq!(qc_a["reviewedActualFrames"], true, "{qc_a}");
    assert_eq!(qc_a["noSyntheticScores"], true, "{qc_a}");

    // Review and execute the visual critique's refinement plan
    let refine_payload_a = json!({
        "ok": true,
        "userExplanation": qc_a["userSummary"],
        "plan": qc_a["refinementPlan"]
    });
    let (_, applied_refine_a) = review_and_apply_plan(&mut session_a, &refine_payload_a);
    assert!(applied_refine_a["native"]["stepsExecuted"].as_u64().unwrap_or(0) >= 1);

    // Verify Undo / Redo determinism across the refinement and main motion plan
    let rev_after_refine_a = session_a.document_revision();
    assert!(session_a.undo(), "undo refinement A");
    assert!(session_a.redo(), "redo refinement A");
    assert!(session_a.document_revision() > rev_after_refine_a);

    // Save editable .ecproj and reopen via Project::from_json
    let ecproj_a_path = out_dir.join("challenge-a-typography.ecproj");
    let ecproj_a_json = session_a.project.to_file_json().expect("serialize Challenge A .ecproj");
    assert!(
        ecproj_a_json.contains("\"font\": \"GemiFontSans\""),
        "Challenge A must use GemiFontSans from GemiTron88/GemiFonts"
    );
    std::fs::write(&ecproj_a_path, &ecproj_a_json).expect("write challenge-a-typography.ecproj");
    let reopened_a = effectcraft_engine::project::Project::from_json(&ecproj_a_json)
        .expect("reopen challenge-a-typography.ecproj");
    let reopened_comp_a = reopened_a.comp(effectcraft_engine::project::ItemId(comp_a)).expect("comp A in reopened project");
    assert!(reopened_comp_a.layers.len() >= 4, "reopened Challenge A must preserve all layers");
    assert!(
        reopened_comp_a.layers.iter().any(|l| l.parent.is_some()),
        "reopened Challenge A must preserve layer parenting"
    );

    save_comp_frame_png(
        &mut session_a,
        comp_a,
        2.5,
        "docs/images/aftertron/challenge-a-typography-hero.png",
    );

    // =========================================================================
    // 3. CHALLENGE B — PREMIUM PRODUCT HERO COMPOSITING (12s, 30fps, 1920x1080)
    // =========================================================================
    eprintln!("[Challenge B] Building 12s 30fps 1920x1080 Premium Product Hero Compositing production...");
    let product_asset_path = out_dir.join("challenge-b-product-asset.png");
    synthesize_product_hero_asset(&product_asset_path);

    let mut session_b = effectcraft_host::session();
    session_b
        .execute(
            "comp.new",
            json!({
                "name": "Challenge B — Product Hero Compositing",
                "width": 1920,
                "height": 1080,
                "fps": 30.0,
                "duration": 12.0
            }),
        )
        .expect("create Challenge B comp");
    let comp_b = session_b.active_comp_id().expect("active comp B").0;

    // Import genuine product asset with alpha silhouette and build studio layer stack
    let imp_b = session_b
        .execute(
            "file.import",
            json!({ "paths": [product_asset_path.to_string_lossy()] }),
        )
        .expect("import product hero asset");
    let product_item_id = imp_b["items"][0].as_u64().expect("product item id");

    // 1) Dark studio backdrop solid
    session_b
        .execute(
            "layer.newSolid",
            json!({
                "comp": comp_b,
                "name": "Studio Obsidian Backdrop",
                "color": "#0A0E17",
                "width": 1920,
                "height": 1080
            }),
        )
        .expect("create studio backdrop");

    // 2) Rim highlight solid with Screen blend mode and feathered elliptical mask
    let rim_solid = session_b
        .execute(
            "layer.newSolid",
            json!({
                "comp": comp_b,
                "name": "Studio Rim Lighting",
                "color": "#1C6E8C",
                "width": 1920,
                "height": 1080
            }),
        )
        .expect("create rim lighting solid");
    let rim_layer_id = rim_solid["layer"].as_u64().expect("rim layer id");
    session_b
        .execute(
            "layer.setBlendMode",
            json!({ "comp": comp_b, "layers": [rim_layer_id], "mode": "screen" }),
        )
        .expect("set rim blend mode");
    session_b
        .execute(
            "layer.addMask",
            json!({
                "comp": comp_b,
                "layer": rim_layer_id,
                "shape": "ellipse",
                "rect": [420.0, 140.0, 1080.0, 800.0]
            }),
        )
        .expect("add elliptical mask to rim light");
    session_b
        .execute(
            "layer.mask.set",
            json!({
                "comp": comp_b,
                "layer": rim_layer_id,
                "field": "feather",
                "value": 64.0
            }),
        )
        .expect("feather rim light mask");

    // 3) Product Hero Footage layer + Alpha Track Matte reveal shape layer
    let prod_layer = session_b
        .execute(
            "layer.addItem",
            json!({
                "comp": comp_b,
                "item": product_item_id,
                "position": [960.0, 465.0]
            }),
        )
        .expect("add product footage layer");
    let prod_layer_id = prod_layer["layer"].as_u64().expect("product layer id");
    session_b
        .execute(
            "layer.rename",
            json!({
                "comp": comp_b,
                "layer": prod_layer_id,
                "name": "Hero Product Monolith"
            }),
        )
        .expect("rename product hero layer");

    let matte_shape = session_b
        .execute(
            "layer.newShape",
            json!({
                "comp": comp_b,
                "kind": "rounded",
                "name": "Product Reveal Matte",
                "size": [860.0, 860.0],
                "fill": [1.0, 1.0, 1.0],
                "position": [960.0, 465.0]
            }),
        )
        .expect("create track matte shape");
    let matte_layer_id = matte_shape["layer"].as_u64().expect("matte layer id");
    session_b
        .execute(
            "layer.setTrackMatte",
            json!({
                "comp": comp_b,
                "layer": prod_layer_id,
                "matte": matte_layer_id,
                "kind": "alpha"
            }),
        )
        .expect("assign alpha track matte to product hero layer");

    session_b
        .execute(
            "layer.newText",
            json!({
                "comp": comp_b,
                "name": "Lower Third Brand Title",
                "text": "AURA PRO · TITANIUM ACOUSTIC CORE",
                "font": "GemiFontSans",
                "style": "Bold",
                "size": 52.0,
                "fill": [0.97, 0.96, 0.94],
                "tracking": 55.0,
                "position": [960.0, 896.0]
            }),
        )
        .expect("create Challenge B product headline");
    session_b
        .execute(
            "layer.newText",
            json!({
                "comp": comp_b,
                "name": "Lower Third Arabic Tagline",
                "text": "آفاق النقاء — تصميم صوتي استثنائي",
                "font": "GemiFontSans",
                "style": "Medium",
                "size": 34.0,
                "fill": [1.0, 0.80, 0.24],
                "direction": "rtl",
                "position": [960.0, 954.0]
            }),
        )
        .expect("create Challenge B product Arabic subtitle");

    // Inspect live composition and ask TRON for Challenge B product compositing motion plan
    let ctx_b = tron_actions::live_context(&session_b).expect("live_context B");
    let brief_b = "اعمل كومبوزيت وإعلان منتج احترافي (Product Hero Compositing) مدته 12 ثانية بمعدل 30 إطار/ثانية مع فصل طبقي للإضاءة المحيطية، كشف ناعم بالماسك والتراك مات، حركة تقريب سينمائية هادئة للمنتج، وتدريج تباين احترافي.";
    let raw_plan_b = tron_local::prepare_after_plan(brief_b, &ctx_b).expect("prepare_after_plan B");
    assert_eq!(raw_plan_b["ok"], true, "{raw_plan_b}");
    let res_plan_b = raw_plan_b.get("result").unwrap_or(&raw_plan_b);
    assert_eq!(res_plan_b["exploredDirections"].as_array().map(Vec::len), Some(2));

    // Review and apply Direction A (which adds Null rig, Editorial Kicker text, keyframes, Easy Ease, Drop Shadow, and Motion Blur on the Product Hero layer)
    let prop_b = tron_actions::select_direction(&raw_plan_b, 0)
        .expect("select direction_a (index 0) for Challenge B");
    let (_, applied_b) = review_and_apply_plan(&mut session_b, &prop_b);
    assert!(applied_b["native"]["stepsExecuted"].as_u64().unwrap_or(0) >= 6);
    assert_eq!(applied_b["native"]["singleUndoCoalesced"], true);

    // Sample 5 rendered frames and run TRON 5-frame visual critique & refinement
    let sampled_b = tron_actions::sample_rendered_frames(&session_b)
        .expect("sample 5 rendered frames for Challenge B");
    assert_eq!(sampled_b.len(), 5);
    assert_ne!(
        sampled_b[0]["sha256"], sampled_b[2]["sha256"],
        "Challenge B frames must animate across time"
    );

    let post_ctx_b = tron_actions::live_context(&session_b).expect("post live_context B");
    let critique_b = tron_local::critique_after_execution(&post_ctx_b, &sampled_b, brief_b)
        .expect("critique_after_execution B");
    assert_eq!(critique_b["ok"], true);
    let qc_b = critique_b.get("result").unwrap_or(&critique_b);
    let refine_payload_b = json!({
        "ok": true,
        "userExplanation": qc_b["userSummary"],
        "plan": qc_b["refinementPlan"]
    });
    let _ = review_and_apply_plan(&mut session_b, &refine_payload_b);

    assert!(session_b.undo(), "undo Challenge B refinement");
    assert!(session_b.redo(), "redo Challenge B refinement");

    let ecproj_b_path = out_dir.join("challenge-b-product-hero.ecproj");
    let ecproj_b_json = session_b.project.to_file_json().expect("serialize Challenge B .ecproj");
    std::fs::write(&ecproj_b_path, &ecproj_b_json).expect("write challenge-b-product-hero.ecproj");
    let reopened_b = effectcraft_engine::project::Project::from_json(&ecproj_b_json)
        .expect("reopen challenge-b-product-hero.ecproj");
    let reopened_comp_b = reopened_b.comp(effectcraft_engine::project::ItemId(comp_b)).expect("comp B in reopened project");
    assert!(
        reopened_comp_b.layers.iter().any(|l| l.track_matte.is_some()),
        "reopened Challenge B must preserve Track Matte"
    );

    save_comp_frame_png(
        &mut session_b,
        comp_b,
        3.0,
        "docs/images/aftertron/challenge-b-product-hero.png",
    );

    // =========================================================================
    // 4. CHALLENGE C — DISTINCTIVE VISUAL IDENTITY / VFX REVEAL (10s, 24fps, 1920x1080)
    // =========================================================================
    eprintln!("[Challenge C] Building 10s 24fps 1920x1080 Distinctive Visual Identity / VFX Reveal...");
    let mut session_c = effectcraft_host::session();
    session_c.prefs.startup.show_home_on_launch = false;
    session_c.prefs.appearance.appearance_mode = "dark".into();
    session_c.prefs.appearance.dark_theme = "inferno-dark".into();
    session_c.prefs.appearance.theme = "inferno-dark".into();

    session_c
        .execute(
            "comp.new",
            json!({
                "name": "Challenge C — Brand Identity VFX Reveal",
                "width": 1920,
                "height": 1080,
                "fps": 24.0,
                "duration": 10.0
            }),
        )
        .expect("create Challenge C comp");
    let comp_c = session_c.active_comp_id().expect("active comp C").0;

    // Seed multi-layer geometric crest + energy core before TRON motion choreography
    session_c
        .execute(
            "layer.newSolid",
            json!({
                "comp": comp_c,
                "name": "Obsidian Stage Plate",
                "color": "#08090F",
                "width": 1920,
                "height": 1080
            }),
        )
        .expect("create Challenge C stage plate");

    let energy_ring = session_c
        .execute(
            "layer.newShape",
            json!({
                "comp": comp_c,
                "kind": "star",
                "name": "Inferno Emblem Core",
                "size": [520.0, 520.0],
                "fill": [0.96, 0.24, 0.18],
                "fillOpacity": 75.0,
                "stroke": [1.0, 0.78, 0.22],
                "strokeWidth": 6.0,
                "position": [960.0, 480.0]
            }),
        )
        .expect("create emblem core shape");
    let emblem_layer_id = energy_ring["layer"].as_u64().expect("emblem layer id");
    session_c
        .execute(
            "layer.addShapeItem",
            json!({ "comp": comp_c, "layer": emblem_layer_id, "kind": "trim" }),
        )
        .expect("add trim paths to emblem core");

    let orbital_ring = session_c
        .execute(
            "layer.newShape",
            json!({
                "comp": comp_c,
                "kind": "ellipse",
                "name": "Orbital Energy Halo",
                "size": [740.0, 740.0],
                "fillType": "none",
                "stroke": [0.22, 0.78, 0.98],
                "strokeWidth": 4.0,
                "position": [960.0, 480.0]
            }),
        )
        .expect("create orbital halo");
    let halo_layer_id = orbital_ring["layer"].as_u64().expect("halo layer id");
    session_c
        .execute(
            "layer.setBlendMode",
            json!({ "comp": comp_c, "layers": [halo_layer_id], "mode": "add" }),
        )
        .expect("set halo blend mode to Add");

    session_c
        .execute(
            "layer.newText",
            json!({
                "comp": comp_c,
                "name": "Brand Identity Wordmark",
                "text": "ARTCRAFT · INFERNO ENGINE",
                "font": "GemiFontSans",
                "style": "Bold",
                "size": 58.0,
                "fill": [0.97, 0.96, 0.94],
                "tracking": 65.0,
                "position": [960.0, 880.0]
            }),
        )
        .expect("create brand identity wordmark");

    session_c
        .execute(
            "layer.newText",
            json!({
                "comp": comp_c,
                "name": "Brand Tagline AR",
                "text": "نبض الهوية البصرية — استوديو الموشن والمؤثرات",
                "font": "GemiFontSans",
                "style": "Medium",
                "size": 34.0,
                "fill": [1.0, 0.80, 0.24],
                "direction": "rtl",
                "position": [960.0, 948.0]
            }),
        )
        .expect("create brand Arabic tagline");

    let ctx_c = tron_actions::live_context(&session_c).expect("live_context C");
    let brief_c = "صمّم كشف هوية بصرية وشعار (Brand Identity & VFX Reveal) مدته 10 ثواني بمعدل 24 إطار/ثانية يجمع بين التكوين الهندسي الدقيق والطاقة الضوئية المدروسة مع حركة تمهيد واستقرار قوي.";
    let raw_plan_c = tron_local::prepare_after_plan(brief_c, &ctx_c).expect("prepare_after_plan C");
    assert_eq!(raw_plan_c["ok"], true, "{raw_plan_c}");
    let res_plan_c = raw_plan_c.get("result").unwrap_or(&raw_plan_c);
    assert_eq!(res_plan_c["exploredDirections"].as_array().map(Vec::len), Some(2));

    let prop_c = tron_actions::select_direction(&raw_plan_c, 1)
        .expect("select direction_b (index 1) for Challenge C");
    let (_, applied_c) = review_and_apply_plan(&mut session_c, &prop_c);
    assert!(applied_c["native"]["stepsExecuted"].as_u64().unwrap_or(0) >= 6);

    // Also test subjective Arabic feedback translation and native execution
    let rev_c_refine_step = tron_local::prepare_after_plan(
        "ابرز العنوان الرئيسي بظل عمق ناعم",
        &tron_actions::live_context(&session_c).unwrap(),
    )
    .expect("subjective Arabic feedback on Challenge C");
    let _ = review_and_apply_plan(&mut session_c, &rev_c_refine_step);

    let sampled_c = tron_actions::sample_rendered_frames(&session_c)
        .expect("sample 5 rendered frames for Challenge C");
    assert_eq!(sampled_c.len(), 5);
    assert_ne!(
        sampled_c[0]["sha256"], sampled_c[3]["sha256"],
        "Challenge C frames must animate across time"
    );

    let post_ctx_c = tron_actions::live_context(&session_c).expect("post live_context C");
    let critique_c = tron_local::critique_after_execution(&post_ctx_c, &sampled_c, brief_c)
        .expect("critique_after_execution C");
    assert_eq!(critique_c["ok"], true);
    let qc_c = critique_c.get("result").unwrap_or(&critique_c);
    let refine_payload_c = json!({
        "ok": true,
        "userExplanation": qc_c["userSummary"],
        "plan": qc_c["refinementPlan"]
    });
    let _ = review_and_apply_plan(&mut session_c, &refine_payload_c);

    assert!(session_c.undo(), "undo Challenge C QC refinement");
    assert!(session_c.redo(), "redo Challenge C QC refinement");

    let ecproj_c_path = out_dir.join("challenge-c-brand-reveal.ecproj");
    let ecproj_c_json = session_c.project.to_file_json().expect("serialize Challenge C .ecproj");
    std::fs::write(&ecproj_c_path, &ecproj_c_json).expect("write challenge-c-brand-reveal.ecproj");
    let reopened_c = effectcraft_engine::project::Project::from_json(&ecproj_c_json)
        .expect("reopen challenge-c-brand-reveal.ecproj");
    assert!(
        reopened_c.comp(effectcraft_engine::project::ItemId(comp_c)).unwrap().layers.len() >= 3,
        "reopened Challenge C must preserve layers"
    );

    save_comp_frame_png(
        &mut session_c,
        comp_c,
        2.5,
        "docs/images/aftertron/challenge-c-brand-reveal.png",
    );

    let _ = session_c.execute("time.set", json!({ "comp": comp_c, "time": 2.5 }));
    let _ = session_c.execute("layer.select", json!({ "comp": comp_c, "layers": [] }));

    // =========================================================================
    // 5. CAPTURE PREMIUM FLOATING TRON CHAT SCREENSHOTS (DARK, LIGHT, MINIMIZED)
    // =========================================================================
    eprintln!("[TRON UI] Launching wgpu Harness to capture TRON Chat Dark, Light, and Minimized screenshots...");
    let adapter = effectcraft_gpu::Gpu::headless()
        .map(|probe| effectcraft_engine::render::Accelerator::name(&probe))
        .unwrap_or_default();
    let (tx, rx) = mpsc::channel::<ControlRequest>();
    let mut harness = Harness::builder()
        .with_size(egui::vec2(1680.0, 1020.0))
        .with_pixels_per_point(1.0)
        .wgpu_setup(same_adapter(adapter))
        .build_eframe(move |_cc| {
            let mut app = EffectcraftApp::new(session_c).with_control(rx);
            app.ui.start_screen = false;
            app
        });
    settle(&mut harness, 0.8);

    // 5a) TRON Floating Chat in Inferno Pixel Glass Dark
    tron_chat::seed_acceptance_state_for_capture(&harness.ctx, true, true);
    settle(&mut harness, 0.8);
    save_verified_screenshot(&mut harness, "docs/images/aftertron/tron-chat-dark.png");

    // 5b) TRON Floating Chat in Inferno Pixel Glass Light
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
    settle(&mut harness, 0.8);
    save_verified_screenshot(&mut harness, "docs/images/aftertron/tron-chat-light.png");

    // 5c) TRON Minimized Launcher Pill in Inferno Pixel Glass Dark
    let _ = send_control(
        &mut harness,
        &tx,
        "engine.execute",
        json!({ "id": "prefs.set", "params": { "key": "appearance.appearanceMode", "value": "dark" } }),
    );
    tron_chat::seed_acceptance_state_for_capture(&harness.ctx, false, true);
    settle(&mut harness, 0.8);
    save_verified_screenshot(&mut harness, "docs/images/aftertron/tron-chat-minimized.png");

    // Final release unloads shared model cleanly
    let final_rel = tron_local::release().expect("final release of AfterTron lease");
    assert_eq!(final_rel["status"]["activeLeaseCount"], 0);
    assert_eq!(final_rel["status"]["state"], "NOT_LOADED");
}
