//! AfterTron local TRON plan boundary: exact comp/revision and native edits.
//! This is NOT a cloud authorization bypass. Local actions require a real UI
//! approval; external file I/O, scripts, expressions and raw commands denied.

use std::collections::{BTreeSet, HashSet};
use std::sync::{Mutex, OnceLock};

use effectcraft_engine::Session;
use effectcraft_time::{FrameRate, TICKS_PER_SECOND, Tick};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::gemimcp::build_aftertron_product_manifest;

pub const PLAN_SCHEMA: &str = "aftertron.tron.plan.v1";
pub const MAX_STEPS: usize = 24;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MotionPlan {
    pub schema: String,
    pub product: String,
    pub plan_id: String,
    pub project_id: String,
    pub comp_id: u64,
    pub expected_revision: u64,
    pub fps_num: i64,
    pub fps_den: i64,
    pub steps: Vec<MotionStep>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MotionStep {
    pub id: String,
    pub tool: String,
    pub params: Value,
}

/// Must be constructed by trusted local UI, NEVER taken from AI response.
#[derive(Debug, Clone)]
pub struct LocalMotionAuthority {
    pub project_id: String,
    pub comp_id: u64,
    pub revision: u64,
    pub fps_num: i64,
    pub fps_den: i64,
    pub approved: bool,
}

fn consumed_approvals() -> &'static Mutex<HashSet<String>> {
    static CONSUMED: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
    CONSUMED.get_or_init(|| Mutex::new(HashSet::new()))
}

fn err(code: &str) -> String {
    format!("AFTERTRON_PREFLIGHT_DENIED:{code}")
}

fn safe_id(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 128
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_' || b == b'.')
}

/// Deterministic FNV-1a 64-bit digest of a `MotionPlan` bound to its exact steps and parameters.
pub fn plan_digest_hex(plan: &MotionPlan) -> String {
    let bytes = serde_json::to_vec(plan).unwrap_or_default();
    let mut hash = 0xcbf29ce484222325u64;
    for b in bytes {
        hash ^= u64::from(b);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}

fn approval_replay_key(plan: &MotionPlan, authority: &LocalMotionAuthority) -> String {
    format!(
        "{}:{}:{}:{}:{}",
        authority.project_id,
        authority.comp_id,
        authority.revision,
        plan.plan_id,
        plan_digest_hex(plan)
    )
}

/// Resolve primary native command for every allowed mutating `after.*` tool.
pub fn native_command(tool: &str, p: &Value) -> Option<&'static str> {
    match tool {
        "after.composition.settings" => Some("comp.settings"),
        "after.layer.create_text" => Some("layer.newText"),
        "after.layer.create_shape" => Some("layer.newShape"),
        "after.layer.create_solid" => Some("layer.newSolid"),
        "after.layer.create_null" => Some("layer.newNull"),
        "after.layer.create_adjustment" => Some("layer.newAdjustment"),
        "after.layer.set_parent" => Some("layer.setParent"),
        "after.layer.set_blend_mode" => Some("layer.setBlendMode"),
        "after.layer.set_in_out" => Some("layer.timing"),
        "after.keyframe.easy_ease" => Some("keys.easyEase"),
        "after.mask.add" => {
            if p.get("vertices").is_some() {
                Some("mask.new")
            } else {
                Some("layer.addMask")
            }
        }
        "after.matte.set_track_matte" => {
            if p.get("matteLayer").is_some() || p.get("matte").is_some() {
                Some("layer.setTrackMatte")
            } else {
                Some("layer.trackMatte")
            }
        }
        "after.text.set_source" => Some("layer.setText"),
        "after.property.set" if p.get("expression").is_none() => {
            let path = p.get("path").and_then(Value::as_str).unwrap_or("");
            if matches!(
                path,
                "switches/motionBlur" | "motionBlur" | "switches/threeD" | "threeD"
            ) {
                Some("layer.setSwitch")
            } else if p.get("time").is_some() || p.get("timeTick").is_some() {
                Some("prop.addKey")
            } else {
                Some("prop.set")
            }
        }
        "after.keyframe.add"
            if p.get("keys").is_none()
                && (p.get("time").is_some() || p.get("timeTick").is_some()) =>
        {
            Some("prop.addKey")
        }
        "after.effect.apply" => Some("effect.apply"),
        _ => None,
    }
}

fn allowed_keys_for_tool(tool: &str) -> &'static [&'static str] {
    match tool {
        "after.composition.settings" => &[
            "comp",
            "name",
            "width",
            "height",
            "pixelAspect",
            "frameRate",
            "duration",
            "startTime",
            "background",
            "shutterAngle",
            "shutterPhase",
            "samplesPerFrame",
            "adaptiveSampleLimit",
            "preserveFrameRate",
            "preserveResolution",
            "renderer",
            "previewResolution",
            "dropFrame",
            "hideShy",
            "frameBlending",
            "motionBlur",
        ],
        "after.layer.create_text" => &[
            "comp",
            "text",
            "name",
            "position",
            "anchor",
            "anchorPoint",
            "box",
            "vertical",
            "edit",
            "font",
            "style",
            "size",
            "fontSize",
            "fill",
            "fillColor",
            "stroke",
            "strokeColor",
            "applyFill",
            "applyStroke",
            "strokeWidth",
            "tracking",
            "leading",
            "baselineShift",
            "hScale",
            "vScale",
            "justify",
            "justification",
            "align",
            "direction",
            "strokeOverFill",
            "fauxBold",
            "fauxItalic",
            "allCaps",
            "smallCaps",
            "superscript",
            "subscript",
            "underline",
            "strikethrough",
            "ligatures",
            "digits",
            "kashida",
        ],
        "after.layer.create_solid" => &["comp", "name", "color", "width", "height", "pixelAspect"],
        "after.layer.create_shape" => &[
            "comp",
            "kind",
            "shape",
            "name",
            "size",
            "position",
            "roundness",
            "fill",
            "fillColor",
            "fillType",
            "fillBlend",
            "fillOpacity",
            "stroke",
            "strokeColor",
            "strokeType",
            "strokeBlend",
            "strokeOpacity",
            "strokeWidth",
            "trim",
        ],
        "after.layer.create_null" => &["comp", "name", "position"],
        "after.layer.create_adjustment" => &["comp", "name"],
        "after.layer.set_parent" => &["comp", "layer", "layers", "parent", "compensate"],
        "after.layer.set_blend_mode" => &["comp", "layer", "layers", "mode", "step"],
        "after.layer.set_in_out" => &[
            "comp",
            "layer",
            "layers",
            "in",
            "out",
            "start",
            "delta",
            "inTick",
            "outTick",
            "startTick",
            "deltaTick",
            "stretch",
            "op",
        ],
        "after.property.set" => &[
            "comp", "layer", "path", "value", "time", "timeTick", "timeBase",
        ],
        "after.keyframe.add" => &[
            "comp",
            "layer",
            "path",
            "value",
            "time",
            "timeTick",
            "timeBase",
            "interpolation",
            "easyEase",
            "which",
            "inSpeed",
            "inInfluence",
            "outSpeed",
            "outInfluence",
            "continuous",
        ],
        "after.keyframe.easy_ease" => &[
            "comp",
            "layer",
            "path",
            "time",
            "timeTick",
            "which",
            "inSpeed",
            "inInfluence",
            "outSpeed",
            "outInfluence",
            "continuous",
        ],
        "after.effect.apply" => &[
            "comp",
            "layer",
            "layers",
            "effect",
            "matchName",
            "values",
            "params",
            "index",
        ],
        "after.mask.add" => &[
            "comp",
            "layer",
            "shape",
            "rect",
            "vertices",
            "inTangents",
            "outTangents",
            "closed",
            "mode",
            "inverted",
            "feather",
            "opacity",
            "expansion",
        ],
        "after.text.set_source" => &[
            "comp",
            "layer",
            "layers",
            "text",
            "font",
            "style",
            "size",
            "fontSize",
            "fill",
            "fillColor",
            "stroke",
            "strokeColor",
            "applyFill",
            "applyStroke",
            "strokeWidth",
            "tracking",
            "leading",
            "baselineShift",
            "hScale",
            "vScale",
            "justify",
            "justification",
            "align",
            "direction",
        ],
        "after.matte.set_track_matte" => &[
            "comp",
            "layer",
            "layers",
            "matte",
            "matteLayer",
            "mode",
            "kind",
            "op",
        ],
        _ => &[],
    }
}

pub fn resolve_effect_spec(raw: &str) -> Option<&'static effectcraft_effects::EffectSpec> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Some(spec) = effectcraft_effects::lookup(trimmed) {
        return Some(spec);
    }
    let normalized = match trimmed
        .to_ascii_lowercase()
        .replace(['_', '-', ' '], "")
        .as_str()
    {
        "gaussianblur" | "blur" => "ec.blur.gaussian",
        "fastboxblur" | "boxblur" => "ec.blur.fastbox",
        "directionalblur" => "ec.blur.directional",
        "radialblur" => "ec.blur.radial",
        "sharpen" => "ec.blur.sharpen",
        "unsharpmask" => "ec.blur.unsharp",
        "glow" | "stylizeglow" => "ec.stylize.glow",
        "dropshadow" | "shadow" => "ec.perspective.dropshadow",
        "bevelalpha" => "ec.perspective.bevelalpha",
        "levels" | "colorlevels" => "ec.color.levels",
        "curves" | "colorcurves" => "ec.color.curves",
        "huesaturation" | "huesat" => "ec.color.huesaturation",
        "tint" => "ec.color.tint",
        "tritone" => "ec.color.tritone",
        "brightnesscontrast" | "brightnessandcontrast" => "ec.color.brightnesscontrast",
        "exposure" => "ec.color.exposure",
        "vibrance" => "ec.color.vibrance",
        "lumetri" | "lumetricolor" => "ec.color.lumetri",
        "fill" | "generatefill" => "ec.generate.fill",
        "gradientramp" | "ramp" => "ec.generate.gradientramp",
        "cclightsweep" | "lightsweep" => "ec.generate.cclightsweep",
        "cclightrays" | "lightrays" => "ec.generate.cclightrays",
        "cclightburst" | "lightburst" => "ec.generate.cclightburst",
        "vignette" | "ccvignette" => "ec.stylize.ccvignette",
        "fractalnoise" | "fractal" => "ec.noise.fractal",
        "turbulentnoise" => "ec.noise.turbulent",
        "noise" => "ec.noise.noise",
        "linearwipe" => "ec.transition.linearwipe",
        "radialwipe" => "ec.transition.radialwipe",
        "venetianblinds" => "ec.transition.venetian",
        "turbulentdisplace" => "ec.distort.turbulentdisplace",
        "transform" | "distorttransform" => "ec.distort.transform",
        _ => return None,
    };
    effectcraft_effects::find(normalized)
}

fn resolve_effect_param_id(
    spec: &'static effectcraft_effects::EffectSpec,
    raw_key: &str,
) -> Option<&'static str> {
    if let Some(ps) = spec
        .params
        .iter()
        .find(|p| p.id == raw_key || p.id.eq_ignore_ascii_case(raw_key))
    {
        return Some(ps.id);
    }
    if let Some(ps) = spec
        .params
        .iter()
        .find(|p| p.name.eq_ignore_ascii_case(raw_key))
    {
        return Some(ps.id);
    }
    let alias = match (
        spec.id,
        raw_key
            .to_ascii_lowercase()
            .replace(['_', '-', ' '], "")
            .as_str(),
    ) {
        ("ec.color.levels", "inputblack") => "inBlack",
        ("ec.color.levels", "inputwhite") => "inWhite",
        ("ec.color.levels", "outputblack") => "outBlack",
        ("ec.color.levels", "outputwhite") => "outWhite",
        ("ec.blur.gaussian", "blur" | "radius") => "blurriness",
        ("ec.blur.fastbox", "blur" | "blurriness") => "radius",
        ("ec.color.exposure", "exposure") => "master/exposure",
        ("ec.color.exposure", "offset") => "master/offset",
        ("ec.color.exposure", "gamma") => "master/gamma",
        ("ec.perspective.dropshadow", "shadowcolor") => "color",
        ("ec.stylize.glow", "glowradius") => "radius",
        ("ec.stylize.glow", "glowintensity") => "intensity",
        ("ec.stylize.glow", "glowthreshold") => "threshold",
        _ => return None,
    };
    spec.params.iter().find(|p| p.id == alias).map(|p| p.id)
}

pub fn normalize_property_path(raw: &str) -> String {
    let trimmed = raw.trim();
    match trimmed
        .to_ascii_lowercase()
        .replace(['_', '-', ' '], "")
        .as_str()
    {
        "position" | "transform/position" => "transform/position".to_string(),
        "scale" | "transform/scale" => "transform/scale".to_string(),
        "rotation" | "transform/rotation" | "zrotation" | "transform/zrotation" => {
            "transform/rotation".to_string()
        }
        "opacity" | "transform/opacity" => "transform/opacity".to_string(),
        "anchor" | "anchorpoint" | "transform/anchor" | "transform/anchorpoint" => {
            "transform/anchor".to_string()
        }
        "positionx" | "transform/positionx" | "xposition" => "transform/positionX".to_string(),
        "positiony" | "transform/positiony" | "yposition" => "transform/positionY".to_string(),
        "positionz" | "transform/positionz" | "zposition" => "transform/positionZ".to_string(),
        "motionblur" | "switches/motionblur" => "switches/motionBlur".to_string(),
        "threed" | "3d" | "switches/threed" => "switches/threeD".to_string(),
        _ => {
            if let Some(rest) = trimmed.strip_prefix("Transform/") {
                normalize_property_path(&format!("transform/{rest}"))
            } else {
                trimmed.to_string()
            }
        }
    }
}

fn safe_frame(t: i64, rate: FrameRate) -> bool {
    t >= 0 && t <= Tick::MAX.0 && rate.tick_of(rate.frame_at(Tick(t))).0 == t
}

fn tick_to_roundtrip_seconds(t: i64, rate: FrameRate) -> Result<f64, String> {
    if !safe_frame(t, rate) {
        return Err(err("TIME_NOT_EXACT_FRAME_TICK"));
    }
    let seconds = Tick(t).seconds();
    if Tick::from_seconds_f64(seconds).0 != t {
        return Err(err("TICK_TO_SECONDS_NOT_ROUNDTRIPPABLE"));
    }
    Ok(seconds)
}

fn verify_params(tool: &str, p: &Value, rate: FrameRate, comp_id: u64) -> Result<(), String> {
    let o = p.as_object().ok_or_else(|| err("PARAMS_NOT_OBJECT"))?;
    if serde_json::to_vec(p).map_err(|_| err("INVALID_JSON"))?.len() > 16384 {
        return Err(err("PARAMS_TOO_LARGE"));
    }
    for forbidden in [
        "projectId",
        "compId",
        "expectedRevision",
        "workspaceRoots",
        "grantedScopes",
        "approvalReceiptId",
        "accessToken",
        "authToken",
        "script",
        "shell",
        "expression",
        "command",
        "file",
        "uri",
    ] {
        if o.contains_key(forbidden) {
            return Err(err("MODEL_SUPPLIED_PRIVILEGED_FIELD"));
        }
    }
    let allowed = allowed_keys_for_tool(tool);
    for key in o.keys() {
        if !allowed.contains(&key.as_str()) {
            return Err(err("UNKNOWN_TOOL_PARAM"));
        }
    }
    if let Some(comp) = o.get("comp")
        && comp.as_u64() != Some(comp_id)
    {
        return Err(err("CROSS_COMPOSITION_ARGUMENT"));
    }
    for field in [
        "timeTick",
        "startTick",
        "endTick",
        "inTick",
        "outTick",
        "deltaTick",
        "durationTicks",
        "atTick",
    ] {
        if let Some(value) = o.get(field) {
            if !value.as_i64().is_some_and(|t| safe_frame(t, rate)) {
                return Err(err("TIME_NOT_EXACT_FRAME_TICK"));
            }
        }
    }
    if let Some(value) = o.get("time") {
        let sec = value
            .as_f64()
            .ok_or_else(|| err("KEYFRAME_TIME_NOT_NUMERIC"))?;
        if !sec.is_finite() || sec < 0.0 || !safe_frame(Tick::from_seconds_f64(sec).0, rate) {
            return Err(err("KEYFRAME_TIME_NOT_ALIGNED"));
        }
    }
    Ok(())
}

pub fn preflight(plan: &MotionPlan, authority: &LocalMotionAuthority) -> Result<Value, String> {
    if plan.schema != PLAN_SCHEMA || plan.product != "aftertron" {
        return Err(err("PLAN_SCHEMA"));
    }
    if plan.project_id.trim().is_empty()
        || plan.project_id != authority.project_id
        || plan.comp_id != authority.comp_id
    {
        return Err(err("PROJECT_OR_COMP_MISMATCH"));
    }
    if plan.expected_revision != authority.revision {
        return Err(err("REVISION_CONFLICT"));
    }
    if (plan.fps_num, plan.fps_den) != (authority.fps_num, authority.fps_den)
        || plan.fps_num <= 0
        || plan.fps_den <= 0
        || plan.fps_num > 1_000_000
        || plan.fps_den > 1_000_000
    {
        return Err(err("FPS_MISMATCH"));
    }
    if !safe_id(&plan.plan_id) || plan.steps.is_empty() || plan.steps.len() > MAX_STEPS {
        return Err(err("PLAN_BUDGET"));
    }
    let rate = FrameRate::new(plan.fps_num, plan.fps_den);
    let catalog = build_aftertron_product_manifest();
    let mut ids = BTreeSet::new();
    for step in &plan.steps {
        if !safe_id(&step.id) || !ids.insert(&step.id) {
            return Err(err("DUPLICATE_STEP_ID"));
        }
        if !step.params.is_object() {
            return Err(err("INVALID_PARAMS"));
        }
        let tool = catalog
            .tools
            .iter()
            .find(|t| t.name == step.tool && t.mutating)
            .ok_or_else(|| err("UNKNOWN_OR_READ_ONLY_TOOL"))?;
        let cmd = native_command(&step.tool, &step.params)
            .ok_or_else(|| err("COMPOUND_TOOL_NOT_SUPPORTED"))?;
        if effectcraft_engine::commands::find(cmd).is_none() {
            return Err(err("MISSING_NATIVE_COMMAND"));
        }
        if tool.engine_target.starts_with("tool:")
            && !matches!(
                step.tool.as_str(),
                "after.property.set" | "after.keyframe.add" | "after.effect.apply"
            )
        {
            return Err(err("UNMAPPED_COMPOUND_TOOL"));
        }
        verify_params(&step.tool, &step.params, rate, plan.comp_id)?;
        if let Some(prop_path) = step.params.get("path") {
            if !matches!(
                step.tool.as_str(),
                "after.property.set" | "after.keyframe.add" | "after.keyframe.easy_ease"
            ) {
                return Err(err("PROPERTY_PATH_TOOL_MISMATCH"));
            }
            let path = prop_path.as_str().ok_or_else(|| err("BAD_PROPERTY_PATH"))?;
            if path.is_empty()
                || path.len() > 180
                || path.starts_with('/')
                || path.contains("..")
                || !path.bytes().all(|b| {
                    b.is_ascii_alphanumeric() || matches!(b, b'_' | b'.' | b'/' | b'#' | b'-' | b' ')
                })
            {
                return Err(err("BAD_PROPERTY_PATH"));
            }
        }
        if step.params.get("timeTick").is_some()
            && !matches!(
                step.tool.as_str(),
                "after.property.set" | "after.keyframe.add" | "after.keyframe.easy_ease"
            )
        {
            return Err(err("TIME_TICK_UNSUPPORTED_BY_TOOL"));
        }
        if step.params.get("time").is_some() && step.params.get("timeTick").is_some() {
            return Err(err("AMBIGUOUS_KEYFRAME_TIME"));
        }
        if matches!(
            step.tool.as_str(),
            "after.property.set" | "after.keyframe.add"
        ) {
            if !step
                .params
                .get("layer")
                .is_some_and(|v| v.is_string() || v.is_u64())
                || !step.params.get("path").is_some_and(Value::is_string)
                || step.params.get("value").is_none()
                || (step.tool == "after.keyframe.add"
                    && step.params.get("time").is_none()
                    && step.params.get("timeTick").is_none())
            {
                return Err(err("KEYFRAME_PROPERTY_ARGUMENTS_REQUIRED"));
            }
        }
        if step.tool == "after.effect.apply" {
            let has_layer = step
                .params
                .get("layer")
                .or_else(|| step.params.get("layers"))
                .is_some();
            let raw_fx = step
                .params
                .get("effect")
                .or_else(|| step.params.get("matchName"))
                .and_then(Value::as_str);
            let Some(raw_fx) = raw_fx.filter(|_| has_layer) else {
                return Err(err("EFFECT_LAYER_AND_NAME_REQUIRED"));
            };
            let spec = resolve_effect_spec(raw_fx).ok_or_else(|| err("UNKNOWN_EFFECT"))?;
            if let Some(vals) = step
                .params
                .get("values")
                .or_else(|| step.params.get("params"))
            {
                let obj = vals
                    .as_object()
                    .ok_or_else(|| err("EFFECT_VALUES_NOT_OBJECT"))?;
                for k in obj.keys() {
                    if resolve_effect_param_id(spec, k).is_none() {
                        return Err(err("UNKNOWN_EFFECT_PARAM"));
                    }
                }
            }
        }
    }
    Ok(json!({
        "status": "NATIVE_PREFLIGHT_PASSED_NOT_EXECUTED",
        "schema": "aftertron.tron.native-preflight.v1",
        "projectId": authority.project_id,
        "compId": authority.comp_id,
        "expectedRevision": authority.revision,
        "planDigest": plan_digest_hex(plan),
        "steps": plan.steps.len(),
        "ticksPerSecond": TICKS_PER_SECOND,
        "needsExplicitApproval": true,
        "applied": false
    }))
}

fn execute_single_step(
    session: &mut Session,
    step: &MotionStep,
    comp_id: u64,
    rate: FrameRate,
) -> Result<(&'static str, Value), String> {
    let cmd =
        native_command(&step.tool, &step.params).ok_or_else(|| err("UNSUPPORTED_RUNTIME_TOOL"))?;
    let p = &step.params;

    match step.tool.as_str() {
        "after.composition.settings" => {
            let mut q = p.clone();
            if let Some(o) = q.as_object_mut() {
                o.insert("comp".to_string(), json!(comp_id));
            }
            let out = session.execute_checked(cmd, q).map_err(|e| e.to_string())?;
            Ok((cmd, out))
        }
        "after.layer.create_text" => {
            let mut q = json!({ "comp": comp_id });
            let Some(qo) = q.as_object_mut() else {
                return Err(err("INVALID_PARAMS"));
            };
            if let Some(po) = p.as_object() {
                for (k, v) in po {
                    match k.as_str() {
                        "fontSize" => {
                            qo.insert("size".to_string(), v.clone());
                        }
                        "fillColor" => {
                            qo.insert("fill".to_string(), v.clone());
                        }
                        "strokeColor" => {
                            qo.insert("stroke".to_string(), v.clone());
                        }
                        "justification" | "align" => {
                            qo.insert("justify".to_string(), v.clone());
                        }
                        "anchor" | "anchorPoint" => {}
                        _ => {
                            qo.insert(k.clone(), v.clone());
                        }
                    }
                }
            }
            if !qo.contains_key("font") {
                qo.insert("font".to_string(), json!("GemiFontSans"));
            }
            if !qo.contains_key("style") {
                let sz = qo.get("size").and_then(Value::as_f64).unwrap_or(48.0);
                qo.insert(
                    "style".to_string(),
                    json!(if sz >= 48.0 { "Bold" } else { "Medium" }),
                );
            }
            if !qo.contains_key("direction")
                && qo
                    .get("text")
                    .and_then(Value::as_str)
                    .is_some_and(|t| t.chars().any(|c| ('\u{0600}'..='\u{06FF}').contains(&c)))
            {
                qo.insert("direction".to_string(), json!("rtl"));
            }
            let out = session
                .execute_checked("layer.newText", q)
                .map_err(|e| e.to_string())?;
            if let Some(anchor) = p.get("anchor").or_else(|| p.get("anchorPoint"))
                && let Some(lid) = out
                    .get("layer")
                    .or_else(|| out.get("id"))
                    .and_then(Value::as_u64)
            {
                session
                    .execute_checked(
                        "prop.set",
                        json!({
                            "comp": comp_id,
                            "layer": lid,
                            "path": "transform/anchor",
                            "value": anchor
                        }),
                    )
                    .map_err(|e| e.to_string())?;
            }
            Ok((cmd, out))
        }
        "after.layer.create_solid" => {
            let mut q = p.clone();
            if let Some(o) = q.as_object_mut() {
                o.insert("comp".to_string(), json!(comp_id));
            }
            let out = session
                .execute_checked("layer.newSolid", q)
                .map_err(|e| e.to_string())?;
            Ok((cmd, out))
        }
        "after.layer.create_shape" => {
            let mut q = json!({ "comp": comp_id });
            let Some(qo) = q.as_object_mut() else {
                return Err(err("INVALID_PARAMS"));
            };
            let roundness = p
                .get("roundness")
                .and_then(Value::as_f64)
                .unwrap_or(0.0);
            if let Some(po) = p.as_object() {
                for (k, v) in po {
                    match k.as_str() {
                        "shape" => {
                            qo.insert("kind".to_string(), v.clone());
                        }
                        "fillColor" => {
                            qo.insert("fill".to_string(), v.clone());
                        }
                        "strokeColor" => {
                            qo.insert("stroke".to_string(), v.clone());
                        }
                        "roundness" | "trim" => {}
                        _ => {
                            qo.insert(k.clone(), v.clone());
                        }
                    }
                }
            }
            if roundness > 0.0
                && qo
                    .get("kind")
                    .and_then(Value::as_str)
                    .is_none_or(|k| k == "rect")
            {
                qo.insert("kind".to_string(), json!("rounded"));
            }
            if qo.contains_key("stroke") && !qo.contains_key("fill") {
                qo.insert("fillType".to_string(), json!("none"));
            }
            let out = session
                .execute_checked("layer.newShape", q)
                .map_err(|e| e.to_string())?;
            if let Some(lid) = out
                .get("layer")
                .or_else(|| out.get("id"))
                .and_then(Value::as_u64)
                && let Some(trim_obj) = p.get("trim").and_then(Value::as_object)
            {
                let trim_res = session
                    .execute_checked(
                        "layer.addShapeItem",
                        json!({
                            "comp": comp_id,
                            "layer": lid,
                            "kind": "trim"
                        }),
                    )
                    .map_err(|e| e.to_string())?;
                if let Some(trim_path) = trim_res.get("path").and_then(Value::as_str) {
                    for sub in ["start", "end", "offset"] {
                        if let Some(val) = trim_obj.get(sub) {
                            session
                                .execute_checked(
                                    "prop.set",
                                    json!({
                                        "comp": comp_id,
                                        "layer": lid,
                                        "path": format!("{trim_path}/{sub}"),
                                        "value": val
                                    }),
                                )
                                .map_err(|e| e.to_string())?;
                        }
                    }
                }
            }
            Ok((cmd, out))
        }
        "after.layer.create_null" => {
            let mut q = json!({ "comp": comp_id });
            if let Some(name) = p.get("name") {
                q["name"] = name.clone();
            }
            let out = session
                .execute_checked("layer.newNull", q)
                .map_err(|e| e.to_string())?;
            if let Some(pos) = p.get("position")
                && let Some(lid) = out
                    .get("layer")
                    .or_else(|| out.get("id"))
                    .and_then(Value::as_u64)
            {
                session
                    .execute_checked(
                        "prop.set",
                        json!({
                            "comp": comp_id,
                            "layer": lid,
                            "path": "transform/position",
                            "value": pos
                        }),
                    )
                    .map_err(|e| e.to_string())?;
            }
            Ok((cmd, out))
        }
        "after.layer.create_adjustment" => {
            let mut q = json!({ "comp": comp_id });
            if let Some(name) = p.get("name") {
                q["name"] = name.clone();
            }
            let out = session
                .execute_checked("layer.newAdjustment", q)
                .map_err(|e| e.to_string())?;
            Ok((cmd, out))
        }
        "after.layer.set_parent" => {
            let mut q = json!({
                "comp": comp_id,
                "parent": p.get("parent").cloned().unwrap_or(Value::Null),
            });
            if let Some(layers) = p.get("layers") {
                q["layers"] = layers.clone();
            } else if let Some(layer) = p.get("layer") {
                q["layers"] = json!([layer]);
            }
            if let Some(comp_flag) = p.get("compensate") {
                q["compensate"] = comp_flag.clone();
            }
            let out = session
                .execute_checked("layer.setParent", q)
                .map_err(|e| e.to_string())?;
            Ok((cmd, out))
        }
        "after.layer.set_blend_mode" => {
            let mut q = json!({ "comp": comp_id });
            if let Some(layers) = p.get("layers") {
                q["layers"] = layers.clone();
            } else if let Some(layer) = p.get("layer") {
                q["layers"] = json!([layer]);
            }
            if let Some(mode) = p.get("mode") {
                q["mode"] = mode.clone();
            }
            if let Some(step_val) = p.get("step") {
                q["step"] = step_val.clone();
            }
            let out = session
                .execute_checked("layer.setBlendMode", q)
                .map_err(|e| e.to_string())?;
            Ok((cmd, out))
        }
        "after.layer.set_in_out" => {
            let mut q = json!({ "comp": comp_id });
            if let Some(layers) = p.get("layers") {
                q["layers"] = layers.clone();
            } else if let Some(layer) = p.get("layer") {
                q["layers"] = json!([layer]);
            }
            for (sec_key, tick_key) in [
                ("in", "inTick"),
                ("out", "outTick"),
                ("start", "startTick"),
                ("delta", "deltaTick"),
            ] {
                if let Some(v) = p.get(sec_key) {
                    q[sec_key] = v.clone();
                } else if let Some(t) = p.get(tick_key).and_then(Value::as_i64) {
                    q[sec_key] = json!(tick_to_roundtrip_seconds(t, rate)?);
                }
            }
            if let Some(op) = p.get("op") {
                q["op"] = op.clone();
            }
            let out = session
                .execute_checked("layer.timing", q.clone())
                .map_err(|e| e.to_string())?;
            if let Some(stretch) = p.get("stretch") {
                let mut sq = json!({
                    "comp": comp_id,
                    "percent": stretch
                });
                if let Some(layers) = q.get("layers") {
                    sq["layers"] = layers.clone();
                }
                session
                    .execute_checked("layer.timeStretch", sq)
                    .map_err(|e| e.to_string())?;
            }
            Ok((cmd, out))
        }
        "after.property.set" => {
            let layer = p
                .get("layer")
                .cloned()
                .ok_or_else(|| err("KEYFRAME_PROPERTY_ARGUMENTS_REQUIRED"))?;
            let raw_path = p
                .get("path")
                .and_then(Value::as_str)
                .ok_or_else(|| err("BAD_PROPERTY_PATH"))?;
            let path = normalize_property_path(raw_path);
            let value = p
                .get("value")
                .cloned()
                .ok_or_else(|| err("KEYFRAME_PROPERTY_ARGUMENTS_REQUIRED"))?;
            if path == "switches/motionBlur" {
                let out = session
                    .execute_checked(
                        "layer.setSwitch",
                        json!({
                            "comp": comp_id,
                            "layers": [layer],
                            "switch": "motionBlur",
                            "value": value.as_bool().unwrap_or(true)
                        }),
                    )
                    .map_err(|e| e.to_string())?;
                if value.as_bool().unwrap_or(true) {
                    let _ = session.execute_checked(
                        "comp.setSwitch",
                        json!({
                            "comp": comp_id,
                            "switch": "motionBlur",
                            "value": true
                        }),
                    );
                }
                return Ok(("layer.setSwitch", out));
            }
            if path == "switches/threeD" {
                let out = session
                    .execute_checked(
                        "layer.setSwitch",
                        json!({
                            "comp": comp_id,
                            "layers": [layer],
                            "switch": "threeD",
                            "value": value.as_bool().unwrap_or(true)
                        }),
                    )
                    .map_err(|e| e.to_string())?;
                return Ok(("layer.setSwitch", out));
            }
            let mut q = json!({
                "comp": comp_id,
                "layer": layer,
                "path": path,
                "value": value,
                "timeBase": p.get("timeBase").and_then(Value::as_str).unwrap_or("comp")
            });
            if let Some(tick) = p.get("timeTick").and_then(Value::as_i64) {
                q["time"] = json!(tick_to_roundtrip_seconds(tick, rate)?);
            } else if let Some(sec) = p.get("time") {
                q["time"] = sec.clone();
            }
            let out = session.execute_checked(cmd, q).map_err(|e| e.to_string())?;
            Ok((cmd, out))
        }
        "after.keyframe.add" => {
            let layer = p
                .get("layer")
                .cloned()
                .ok_or_else(|| err("KEYFRAME_PROPERTY_ARGUMENTS_REQUIRED"))?;
            let raw_path = p
                .get("path")
                .and_then(Value::as_str)
                .ok_or_else(|| err("BAD_PROPERTY_PATH"))?;
            let path = normalize_property_path(raw_path);
            let value = p
                .get("value")
                .cloned()
                .ok_or_else(|| err("KEYFRAME_PROPERTY_ARGUMENTS_REQUIRED"))?;
            let seconds = if let Some(tick) = p.get("timeTick").and_then(Value::as_i64) {
                tick_to_roundtrip_seconds(tick, rate)?
            } else if let Some(sec) = p.get("time").and_then(Value::as_f64) {
                sec
            } else {
                return Err(err("KEYFRAME_PROPERTY_ARGUMENTS_REQUIRED"));
            };
            let time_base = p.get("timeBase").and_then(Value::as_str).unwrap_or("comp");
            let out = session
                .execute_checked(
                    "prop.addKey",
                    json!({
                        "comp": comp_id,
                        "layer": layer,
                        "path": path,
                        "time": seconds,
                        "timeBase": time_base,
                        "value": value
                    }),
                )
                .map_err(|e| e.to_string())?;

            let wants_ease = p.get("easyEase").and_then(Value::as_bool).unwrap_or(false)
                || p.get("interpolation").is_some()
                || p.get("inSpeed").is_some()
                || p.get("inInfluence").is_some()
                || p.get("outSpeed").is_some()
                || p.get("outInfluence").is_some();

            if wants_ease {
                session
                    .execute_checked(
                        "keys.select",
                        json!({
                            "comp": comp_id,
                            "timeBase": time_base,
                            "keys": [{ "layer": layer, "path": path, "time": seconds }]
                        }),
                    )
                    .map_err(|e| e.to_string())?;
                if p.get("easyEase").and_then(Value::as_bool).unwrap_or(false) {
                    let which = p.get("which").and_then(Value::as_str).unwrap_or("both");
                    session
                        .execute_checked("keys.easyEase", json!({ "comp": comp_id, "which": which }))
                        .map_err(|e| e.to_string())?;
                }
                if let Some(interp) = p.get("interpolation").and_then(Value::as_str) {
                    match interp {
                        "easyEase" => {
                            session
                                .execute_checked(
                                    "keys.easyEase",
                                    json!({ "comp": comp_id, "which": "both" }),
                                )
                                .map_err(|e| e.to_string())?;
                        }
                        "easyEaseIn" => {
                            session
                                .execute_checked(
                                    "keys.easyEase",
                                    json!({ "comp": comp_id, "which": "in" }),
                                )
                                .map_err(|e| e.to_string())?;
                        }
                        "easyEaseOut" => {
                            session
                                .execute_checked(
                                    "keys.easyEase",
                                    json!({ "comp": comp_id, "which": "out" }),
                                )
                                .map_err(|e| e.to_string())?;
                        }
                        "linear" | "bezier" | "hold" => {
                            session
                                .execute_checked(
                                    "keys.interpolation",
                                    json!({ "comp": comp_id, "in": interp, "out": interp }),
                                )
                                .map_err(|e| e.to_string())?;
                        }
                        _ => {}
                    }
                }
                if p.get("inSpeed").is_some()
                    || p.get("inInfluence").is_some()
                    || p.get("outSpeed").is_some()
                    || p.get("outInfluence").is_some()
                {
                    let mut vq = json!({ "comp": comp_id });
                    for vk in [
                        "inSpeed",
                        "inInfluence",
                        "outSpeed",
                        "outInfluence",
                        "continuous",
                    ] {
                        if let Some(vv) = p.get(vk) {
                            vq[vk] = vv.clone();
                        }
                    }
                    session
                        .execute_checked("keys.velocity", vq)
                        .map_err(|e| e.to_string())?;
                }
            }
            Ok((cmd, out))
        }
        "after.keyframe.easy_ease" => {
            if let Some(layer) = p.get("layer")
                && let Some(raw_path) = p.get("path").and_then(Value::as_str)
            {
                let path = normalize_property_path(raw_path);
                let opt_sec = if let Some(tick) = p.get("timeTick").and_then(Value::as_i64) {
                    Some(tick_to_roundtrip_seconds(tick, rate)?)
                } else {
                    p.get("time").and_then(Value::as_f64)
                };
                if let Some(sec) = opt_sec {
                    session
                        .execute_checked(
                            "keys.select",
                            json!({
                                "comp": comp_id,
                                "timeBase": "comp",
                                "keys": [{ "layer": layer, "path": path, "time": sec }]
                            }),
                        )
                        .map_err(|e| e.to_string())?;
                } else {
                    session
                        .execute_checked(
                            "prop.select",
                            json!({
                                "comp": comp_id,
                                "layer": layer,
                                "path": path,
                                "selectKeys": true
                            }),
                        )
                        .map_err(|e| e.to_string())?;
                }
                if session.state.selected_keys.is_empty() {
                    let _ = session.execute_checked(
                        "prop.toggleKey",
                        json!({
                            "comp": comp_id,
                            "layer": layer,
                            "path": path
                        }),
                    );
                    let _ = session.execute_checked(
                        "prop.select",
                        json!({
                            "comp": comp_id,
                            "layer": layer,
                            "path": path,
                            "selectKeys": true
                        }),
                    );
                }
            }
            let which = p.get("which").and_then(Value::as_str).unwrap_or("both");
            let out = session
                .execute_checked("keys.easyEase", json!({ "comp": comp_id, "which": which }))
                .map_err(|e| e.to_string())?;
            if p.get("inSpeed").is_some()
                || p.get("inInfluence").is_some()
                || p.get("outSpeed").is_some()
                || p.get("outInfluence").is_some()
            {
                let mut vq = json!({ "comp": comp_id });
                for vk in [
                    "inSpeed",
                    "inInfluence",
                    "outSpeed",
                    "outInfluence",
                    "continuous",
                ] {
                    if let Some(vv) = p.get(vk) {
                        vq[vk] = vv.clone();
                    }
                }
                session
                    .execute_checked("keys.velocity", vq)
                    .map_err(|e| e.to_string())?;
            }
            Ok((cmd, out))
        }
        "after.effect.apply" => {
            let layer = p
                .get("layer")
                .cloned()
                .or_else(|| {
                    p.get("layers")
                        .and_then(Value::as_array)
                        .and_then(|a| a.first().cloned())
                })
                .ok_or_else(|| err("EFFECT_LAYER_REQUIRED"))?;
            let raw_fx = p
                .get("effect")
                .or_else(|| p.get("matchName"))
                .and_then(Value::as_str)
                .ok_or_else(|| err("EFFECT_NAME_REQUIRED"))?;
            let spec = resolve_effect_spec(raw_fx).ok_or_else(|| err("UNKNOWN_EFFECT"))?;
            let out = session
                .execute_checked(
                    "effect.apply",
                    json!({
                        "comp": comp_id,
                        "layers": [layer],
                        "effect": spec.id
                    }),
                )
                .map_err(|e| e.to_string())?;

            let effect_path = out
                .get("paths")
                .and_then(Value::as_array)
                .and_then(|a| a.first())
                .and_then(Value::as_str)
                .unwrap_or("effects/#1")
                .to_string();

            if let Some(vals) = p
                .get("values")
                .or_else(|| p.get("params"))
                .and_then(Value::as_object)
            {
                for (k, v) in vals {
                    let param_id = resolve_effect_param_id(spec, k)
                        .ok_or_else(|| err("UNKNOWN_EFFECT_PARAM"))?;
                    let mut normalized_v = v.clone();
                    if spec.id == "ec.color.levels"
                        && matches!(param_id, "inBlack" | "inWhite" | "outBlack" | "outWhite")
                        && let Some(num) = v.as_f64()
                        && num > 1.0
                    {
                        normalized_v = json!((num / 255.0).clamp(0.0, 1.0));
                    }
                    session
                        .execute_checked(
                            "prop.set",
                            json!({
                                "comp": comp_id,
                                "layer": layer,
                                "path": format!("{effect_path}/{param_id}"),
                                "value": normalized_v
                            }),
                        )
                        .map_err(|e| e.to_string())?;
                }
            }
            if let Some(index) = p.get("index").and_then(Value::as_u64)
                && let Some(uid) = out
                    .get("uids")
                    .and_then(Value::as_array)
                    .and_then(|a| a.first())
                    .and_then(Value::as_u64)
            {
                session
                    .execute_checked(
                        "effect.reorder",
                        json!({
                            "comp": comp_id,
                            "layer": layer,
                            "effect": uid,
                            "index": index
                        }),
                    )
                    .map_err(|e| e.to_string())?;
            }
            Ok((cmd, out))
        }
        "after.mask.add" => {
            let layer = p
                .get("layer")
                .cloned()
                .ok_or_else(|| err("MASK_LAYER_REQUIRED"))?;
            let out = if let Some(vertices) = p.get("vertices") {
                let mut q = json!({
                    "comp": comp_id,
                    "layer": layer,
                    "vertices": vertices
                });
                for k in ["inTangents", "outTangents", "closed", "mode"] {
                    if let Some(v) = p.get(k) {
                        q[k] = v.clone();
                    }
                }
                session
                    .execute_checked("mask.new", q)
                    .map_err(|e| e.to_string())?
            } else {
                let mut q = json!({
                    "comp": comp_id,
                    "layer": layer
                });
                for k in ["shape", "rect", "mode"] {
                    if let Some(v) = p.get(k) {
                        q[k] = v.clone();
                    }
                }
                session
                    .execute_checked("layer.addMask", q)
                    .map_err(|e| e.to_string())?
            };
            if let Some(inverted) = p.get("inverted").and_then(Value::as_bool) {
                session
                    .execute_checked(
                        "layer.mask.invert",
                        json!({
                            "comp": comp_id,
                            "layer": layer,
                            "value": inverted
                        }),
                    )
                    .map_err(|e| e.to_string())?;
            }
            for field in ["feather", "opacity", "expansion"] {
                if let Some(val) = p.get(field) {
                    let scalar = if let Some(arr) = val.as_array() {
                        arr.first().and_then(Value::as_f64).unwrap_or(0.0)
                    } else {
                        val.as_f64().unwrap_or(0.0)
                    };
                    session
                        .execute_checked(
                            "layer.mask.set",
                            json!({
                                "comp": comp_id,
                                "layer": layer,
                                "field": field,
                                "value": scalar
                            }),
                        )
                        .map_err(|e| e.to_string())?;
                }
            }
            Ok((cmd, out))
        }
        "after.text.set_source" => {
            let mut q = json!({ "comp": comp_id });
            let Some(qo) = q.as_object_mut() else {
                return Err(err("INVALID_PARAMS"));
            };
            if let Some(po) = p.as_object() {
                for (k, v) in po {
                    match k.as_str() {
                        "fontSize" => {
                            qo.insert("size".to_string(), v.clone());
                        }
                        "fillColor" => {
                            qo.insert("fill".to_string(), v.clone());
                        }
                        "strokeColor" => {
                            qo.insert("stroke".to_string(), v.clone());
                        }
                        "justification" | "align" => {
                            qo.insert("justify".to_string(), v.clone());
                        }
                        _ => {
                            qo.insert(k.clone(), v.clone());
                        }
                    }
                }
            }
            let out = session
                .execute_checked("layer.setText", q)
                .map_err(|e| e.to_string())?;
            Ok((cmd, out))
        }
        "after.matte.set_track_matte" => {
            if let Some(matte) = p.get("matteLayer").or_else(|| p.get("matte")) {
                let mut q = json!({
                    "comp": comp_id,
                    "matte": matte
                });
                if let Some(layer) = p.get("layer") {
                    q["layer"] = layer.clone();
                }
                if let Some(kind) = p
                    .get("kind")
                    .or_else(|| p.get("mode"))
                    .and_then(Value::as_str)
                    .filter(|m| *m != "none")
                {
                    q["kind"] = json!(kind);
                }
                let out = session
                    .execute_checked("layer.setTrackMatte", q)
                    .map_err(|e| e.to_string())?;
                Ok(("layer.setTrackMatte", out))
            } else {
                let mut q = json!({
                    "comp": comp_id,
                    "op": p.get("op").or_else(|| p.get("mode")).cloned().unwrap_or_else(|| json!("none"))
                });
                if let Some(layers) = p.get("layers") {
                    q["layers"] = layers.clone();
                } else if let Some(layer) = p.get("layer") {
                    q["layers"] = json!([layer]);
                }
                let out = session
                    .execute_checked("layer.trackMatte", q)
                    .map_err(|e| e.to_string())?;
                Ok(("layer.trackMatte", out))
            }
        }
        _ => Err(err("UNSUPPORTED_RUNTIME_TOOL")),
    }
}

pub fn execute_approved(
    session: &mut Session,
    plan: &MotionPlan,
    authority: &LocalMotionAuthority,
) -> Result<Value, String> {
    if !authority.approved {
        return Err(err("OWNER_APPROVAL_REQUIRED"));
    }
    let initial = preflight(plan, authority)?;
    if session.document_revision() != authority.revision
        || session.active_comp_id().map(|id| id.0) != Some(authority.comp_id)
    {
        return Err(err("STALE_LIVE_COMPOSITION"));
    }
    let Some(comp) = session.active_comp() else {
        return Err(err("NO_ACTIVE_COMP"));
    };
    if (comp.frame_rate.num, comp.frame_rate.den) != (authority.fps_num, authority.fps_den) {
        return Err(err("LIVE_FRAME_RATE_CHANGED"));
    }

    let replay_key = approval_replay_key(plan, authority);
    {
        let guard = consumed_approvals()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if guard.contains(&replay_key) {
            return Err(err("APPROVAL_ALREADY_CONSUMED"));
        }
    }

    let rate = FrameRate::new(authority.fps_num, authority.fps_den);
    let saved_undo = std::mem::take(&mut session.history.undo);
    let saved_redo = session.history.redo.clone();
    let before_project = session.project.clone();
    let mut receipts = Vec::new();

    for step in &plan.steps {
        let before = session.document_revision();
        match execute_single_step(session, step, authority.comp_id, rate) {
            Ok((cmd, value)) => receipts.push(json!({
                "stepId": step.id,
                "tool": step.tool,
                "native": cmd,
                "revisionBefore": before,
                "revisionAfter": session.document_revision(),
                "result": value,
                "ok": true
            })),
            Err(e) => {
                while !session.history.undo.is_empty() {
                    session.undo();
                }
                session.history.undo = saved_undo;
                session.history.redo = saved_redo;
                session.history.merge_key = None;
                return Ok(json!({
                    "status": "ROLLED_BACK_ON_STEP_ERROR",
                    "applied": false,
                    "rolledBack": true,
                    "recovery": "All prior steps in this motion plan were atomically rolled back to the pre-execution project snapshot.",
                    "failedStepId": step.id,
                    "error": e,
                    "revision": session.document_revision(),
                    "receipts": receipts,
                    "nativePreflight": initial
                }));
            }
        }
    }

    let changed = !session.history.undo.is_empty();
    session.history.undo = saved_undo;
    session.history.merge_key = None;
    if changed {
        session
            .history
            .undo
            .push(("TRON Motion Plan".to_string(), before_project));
        let levels = session.prefs.general.undo_levels.max(1) as usize;
        if session.history.undo.len() > levels {
            let extra = session.history.undo.len() - levels;
            session.history.undo.drain(..extra);
        }
    }

    {
        let mut guard = consumed_approvals()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        guard.insert(replay_key);
    }

    Ok(json!({
        "status": "EXECUTED_NATIVE_REQUIRES_VISUAL_QC",
        "applied": true,
        "stepsExecuted": receipts.len(),
        "singleUndoCoalesced": changed,
        "planDigest": plan_digest_hex(plan),
        "receipts": receipts,
        "revision": session.document_revision(),
        "nativePreflight": initial,
        "saveExportPerformed": false,
        "humanCreativeReviewRequired": true
    }))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    fn authority() -> LocalMotionAuthority {
        LocalMotionAuthority {
            project_id: "after-test-1".into(),
            comp_id: 1,
            revision: 4,
            fps_num: 30000,
            fps_den: 1001,
            approved: false,
        }
    }

    fn plan() -> MotionPlan {
        MotionPlan {
            schema: PLAN_SCHEMA.into(),
            product: "aftertron".into(),
            plan_id: "motion-1".into(),
            project_id: "after-test-1".into(),
            comp_id: 1,
            expected_revision: 4,
            fps_num: 30000,
            fps_den: 1001,
            steps: vec![MotionStep {
                id: "text-1".into(),
                tool: "after.layer.create_text".into(),
                params: json!({"text": "TRON", "name": "Editable Title"}),
            }],
        }
    }

    #[test]
    fn recognizes_real_native_layer_tool_and_rejects_wrong_comp() {
        assert!(preflight(&plan(), &authority()).is_ok());
        let mut p = plan();
        p.comp_id = 2;
        assert!(preflight(&p, &authority()).is_err());
    }

    #[test]
    fn refuses_unapproved_unsafe_commands_unknown_params_and_stale_revisions() {
        let mut p = plan();
        p.expected_revision = 99;
        assert!(preflight(&p, &authority()).is_err());
        p = plan();
        p.steps[0].tool = "after.expression.set".into();
        assert!(preflight(&p, &authority()).is_err());
        p = plan();
        p.steps[0].params = json!({"text": "x", "script": "delete()"});
        assert!(preflight(&p, &authority()).is_err());
        p = plan();
        p.steps[0].params = json!({"text": "x", "unknownHallucinatedParam": 42});
        assert!(preflight(&p, &authority()).is_err());
    }

    #[test]
    fn approval_is_not_model_supplied() {
        let mut session = Session::default();
        assert!(execute_approved(&mut session, &plan(), &authority()).is_err());
    }

    #[test]
    fn executes_compound_tools_and_supports_single_step_undo_and_atomic_rollback() {
        let mut session = Session::default();
        session
            .execute_checked(
                "comp.new",
                json!({
                    "name": "AuditComp",
                    "width": 1920,
                    "height": 1080,
                    "frameRate": 24.0,
                    "duration": 5.0
                }),
            )
            .unwrap();
        let cid = session.active_comp_id().unwrap().0;
        let rev0 = session.document_revision();
        let rate = FrameRate::new(24, 1);
        let f12_tick = rate.tick_of(12).0;

        let auth = LocalMotionAuthority {
            project_id: "audit-proj".into(),
            comp_id: cid,
            revision: rev0,
            fps_num: 24,
            fps_den: 1,
            approved: true,
        };
        let motion_plan = MotionPlan {
            schema: PLAN_SCHEMA.into(),
            product: "aftertron".into(),
            plan_id: "compound-audit-1".into(),
            project_id: "audit-proj".into(),
            comp_id: cid,
            expected_revision: rev0,
            fps_num: 24,
            fps_den: 1,
            steps: vec![
                MotionStep {
                    id: "s1".into(),
                    tool: "after.layer.create_solid".into(),
                    params: json!({"name": "BG_Plate", "color": "#111622", "width": 1920, "height": 1080}),
                },
                MotionStep {
                    id: "s2".into(),
                    tool: "after.layer.create_null".into(),
                    params: json!({"name": "Master_Rig", "position": [960.0, 540.0]}),
                },
                MotionStep {
                    id: "s3".into(),
                    tool: "after.layer.create_text".into(),
                    params: json!({"name": "Hero_Title", "text": "AFTERTRON", "fontSize": 96.0, "fillColor": "#F8FAFC", "tracking": 80.0}),
                },
                MotionStep {
                    id: "s4".into(),
                    tool: "after.layer.set_parent".into(),
                    params: json!({"layer": "Hero_Title", "parent": "Master_Rig"}),
                },
                MotionStep {
                    id: "s5".into(),
                    tool: "after.layer.set_blend_mode".into(),
                    params: json!({"layer": "Hero_Title", "mode": "Screen"}),
                },
                MotionStep {
                    id: "s6".into(),
                    tool: "after.keyframe.add".into(),
                    params: json!({
                        "layer": "Hero_Title",
                        "path": "Transform/Opacity",
                        "timeTick": 0,
                        "value": 0.0,
                        "easyEase": true
                    }),
                },
                MotionStep {
                    id: "s7".into(),
                    tool: "after.keyframe.add".into(),
                    params: json!({
                        "layer": "Hero_Title",
                        "path": "Transform/Opacity",
                        "timeTick": f12_tick,
                        "value": 100.0,
                        "easyEase": true,
                        "inSpeed": 0.0,
                        "inInfluence": 80.0
                    }),
                },
                MotionStep {
                    id: "s8".into(),
                    tool: "after.effect.apply".into(),
                    params: json!({
                        "layer": "Hero_Title",
                        "effect": "Drop Shadow",
                        "values": {
                            "opacity": 60.0,
                            "distance": 18.0,
                            "softness": 36.0
                        }
                    }),
                },
                MotionStep {
                    id: "s9".into(),
                    tool: "after.mask.add".into(),
                    params: json!({
                        "layer": "BG_Plate",
                        "shape": "ellipse",
                        "rect": [160.0, 100.0, 1600.0, 880.0],
                        "mode": "add",
                        "feather": 120.0
                    }),
                },
            ],
        };

        let res = execute_approved(&mut session, &motion_plan, &auth).unwrap();
        assert_eq!(res["applied"], json!(true));
        assert_eq!(session.active_comp().unwrap().layers.len(), 3);

        // Replay of the exact same approval is blocked
        assert!(execute_approved(&mut session, &motion_plan, &auth).is_err());

        // Single undo reverts all 9 steps at once
        assert!(session.undo());
        assert_eq!(session.active_comp().unwrap().layers.len(), 0);

        // Single redo restores all 9 steps at once
        assert!(session.redo());
        assert_eq!(session.active_comp().unwrap().layers.len(), 3);
    }
}
