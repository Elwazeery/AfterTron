//! AfterTron local TRON plan boundary: exact comp/revision and native edits.
//! This is NOT a cloud authorization bypass. Local actions require a real UI
//! approval; external file I/O, scripts, expressions and raw commands denied.
use std::collections::BTreeSet;

use effectcraft_engine::Session;
use effectcraft_time::{FrameRate, Tick, TICKS_PER_SECOND};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::gemimcp::build_aftertron_product_manifest;

pub const PLAN_SCHEMA: &str = "aftertron.tron.plan.v1";
pub const MAX_STEPS: usize = 12;

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

fn err(code: &str) -> String { format!("AFTERTRON_PREFLIGHT_DENIED:{code}") }

fn safe_id(s: &str) -> bool {
    !s.is_empty() && s.len() <= 128
        && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

/// Resolve only native command-backed tools. The compound set_property,
/// add_keyframe and add_effect tools need typed multi-operation adapters:
/// their component native commands are explicitly mapped here.
fn native_command(tool: &str, p: &Value) -> Option<&'static str> {
    match tool {
        "after.layer.create_text" => Some("layer.newText"),
        "after.layer.create_shape" => Some("layer.newShape"),
        "after.layer.create_solid" => Some("layer.newSolid"),
        "after.layer.create_null" => Some("layer.newNull"),
        "after.layer.create_adjustment" => Some("layer.newAdjustment"),
        "after.layer.set_parent" => Some("layer.parent"),
        "after.layer.set_blend_mode" => Some("layer.blendMode"),
        "after.layer.set_in_out" => Some("layer.timing"),
        "after.keyframe.easy_ease" => Some("keys.easyEase"),
        "after.mask.add" => Some("mask.new"),
        "after.matte.set_track_matte" => Some("layer.trackMatte"),
        "after.text.set_source" => Some("layer.setText"),
        "after.property.set" if p.get("expression").is_none()
            && p.get("time").is_none() => Some("prop.set"),
        "after.property.set" if p.get("expression").is_none()
            && p.get("time").is_some() => Some("prop.addKey"),
        "after.keyframe.add" if p.get("keys").is_none()
            && p.get("time").is_some() => Some("prop.addKey"),
        "after.effect.apply" if p.get("values").is_none() => Some("effect.apply"),
        _ => None,
    }
}

fn safe_frame(t: i64, rate: FrameRate) -> bool {
    t >= 0 && t <= Tick::MAX.0 && rate.tick_of(rate.frame_at(Tick(t))).0 == t
}

fn verify_params(p: &Value, rate: FrameRate, comp_id: u64) -> Result<(), String> {
    let o = p.as_object().ok_or_else(|| err("PARAMS_NOT_OBJECT"))?;
    if serde_json::to_vec(p).map_err(|_| err("INVALID_JSON"))?.len() > 16384 {
        return Err(err("PARAMS_TOO_LARGE"));
    }
    for forbidden in [
        "projectId", "compId", "expectedRevision", "workspaceRoots",
        "grantedScopes", "approvalReceiptId", "accessToken", "authToken",
        "script", "shell", "expression", "command", "file", "uri"
    ] {
        if o.contains_key(forbidden) { return Err(err("MODEL_SUPPLIED_PRIVILEGED_FIELD")); }
    }
    if let Some(comp) = o.get("comp") {
        if comp.as_u64() != Some(comp_id) { return Err(err("CROSS_COMPOSITION_ARGUMENT")); }
    }
    for field in ["timeTick", "startTick", "endTick", "durationTicks", "atTick"] {
        if let Some(value) = o.get(field) {
            if !value.as_i64().is_some_and(|t| safe_frame(t, rate)) {
                return Err(err("TIME_NOT_EXACT_FRAME_TICK"));
            }
        }
    }
    if let Some(value) = o.get("time") {
        let sec = value.as_f64().ok_or_else(|| err("KEYFRAME_TIME_NOT_NUMERIC"))?;
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
    if plan.project_id.trim().is_empty() || plan.project_id != authority.project_id ||
        plan.comp_id != authority.comp_id { return Err(err("PROJECT_OR_COMP_MISMATCH")); }
    if plan.expected_revision != authority.revision { return Err(err("REVISION_CONFLICT")); }
    if (plan.fps_num, plan.fps_den) != (authority.fps_num, authority.fps_den)
        || plan.fps_num <= 0 || plan.fps_den <= 0 || plan.fps_num > 1000000 || plan.fps_den > 1000000
    { return Err(err("FPS_MISMATCH")); }
    if !safe_id(&plan.plan_id) || plan.steps.is_empty() || plan.steps.len() > MAX_STEPS {
        return Err(err("PLAN_BUDGET"));
    }
    let rate = FrameRate::new(plan.fps_num, plan.fps_den);
    let catalog = build_aftertron_product_manifest();
    let mut ids = BTreeSet::new();
    for step in &plan.steps {
        if !safe_id(&step.id) || !ids.insert(&step.id) { return Err(err("DUPLICATE_STEP_ID")); }
        if !step.params.is_object() { return Err(err("INVALID_PARAMS")); }
        let tool = catalog.tools.iter().find(|t| t.name == step.tool && t.mutating)
            .ok_or_else(|| err("UNKNOWN_OR_READ_ONLY_TOOL"))?;
        let cmd = native_command(&step.tool, &step.params)
            .ok_or_else(|| err("COMPOUND_TOOL_NOT_SUPPORTED"))?;
        if effectcraft_engine::commands::find(cmd).is_none() { return Err(err("MISSING_NATIVE_COMMAND")); }
        if tool.engine_target.starts_with("tool:")
            && !matches!(step.tool.as_str(), "after.property.set" | "after.keyframe.add" | "after.effect.apply")
        { return Err(err("UNMAPPED_COMPOUND_TOOL")); }
        verify_params(&step.params, rate, plan.comp_id)?;
        if let Some(prop_path) = step.params.get("path") {
            if !matches!(step.tool.as_str(), "after.property.set" | "after.keyframe.add") {
                return Err(err("PROPERTY_PATH_TOOL_MISMATCH"));
            }
            let path = prop_path.as_str().ok_or_else(|| err("BAD_PROPERTY_PATH"))?;
            if path.is_empty() || path.len() > 180 || path.starts_with('/') || path.contains("..")
                || !path.bytes().all(|b| b.is_ascii_alphanumeric()
                    || matches!(b, b'_' | b'.' | b'/' | b'#' | b'-' | b' '))
            {
                return Err(err("BAD_PROPERTY_PATH"));
            }
        }
        if matches!(step.tool.as_str(), "after.property.set" | "after.keyframe.add") {
            if !step.params.get("layer").is_some_and(|v| v.is_string() || v.is_u64())
                || !step.params.get("path").is_some_and(Value::is_string)
                || step.params.get("value").is_none()
            {
                return Err(err("KEYFRAME_PROPERTY_ARGUMENTS_REQUIRED"));
            }
        }
        if step.tool == "after.effect.apply"
            && (step.params.get("layer").is_none() || !step.params.get("effect").is_some_and(Value::is_string))
        {
            return Err(err("EFFECT_LAYER_AND_NAME_REQUIRED"));
        }
    }
    Ok(json!({
        "status": "NATIVE_PREFLIGHT_PASSED_NOT_EXECUTED",
        "schema": "aftertron.tron.native-preflight.v1",
        "projectId": authority.project_id,
        "compId": authority.comp_id,
        "expectedRevision": authority.revision,
        "steps": plan.steps.len(),
        "ticksPerSecond": TICKS_PER_SECOND,
        "needsExplicitApproval": true,
        "applied": false
    }))
}

pub fn execute_approved(
    session: &mut Session, plan: &MotionPlan, authority: &LocalMotionAuthority
) -> Result<Value, String> {
    if !authority.approved { return Err(err("OWNER_APPROVAL_REQUIRED")); }
    let initial = preflight(plan, authority)?;
    if session.document_revision() != authority.revision ||
        session.active_comp_id().map(|id| id.0) != Some(authority.comp_id) {
        return Err(err("STALE_LIVE_COMPOSITION"));
    }
    let Some(comp) = session.active_comp() else { return Err(err("NO_ACTIVE_COMP")); };
    if (comp.frame_rate.num, comp.frame_rate.den) != (authority.fps_num, authority.fps_den) {
        return Err(err("LIVE_FRAME_RATE_CHANGED"));
    }
    let mut receipts = Vec::new();
    for step in &plan.steps {
        // The approval is bound to the specific serialized plan: never silently
        // substitute an additional model-generated step during execution.
        let Some(cmd) = native_command(&step.tool, &step.params) else {
            return Err(err("UNSUPPORTED_RUNTIME_TOOL"));
        };
        let mut params = step.params.clone();
        if step.tool == "after.effect.apply" {
            let layer = params.get("layer").cloned().ok_or_else(|| err("EFFECT_LAYER_REQUIRED"))?;
            let effect = params.get("effect").cloned().ok_or_else(|| err("EFFECT_NAME_REQUIRED"))?;
            params = json!({"layers":[layer], "effect": effect, "comp": authority.comp_id});
        }
        if step.tool == "after.keyframe.add" || step.tool == "after.property.set" {
            if let Some(o) = params.as_object_mut() { o.remove("keys"); }
        }
        let before = session.document_revision();
        match session.execute_checked(cmd, params) {
            Ok(value) => receipts.push(json!({
                "stepId": step.id, "tool": step.tool, "native": cmd,
                "revisionBefore": before, "revisionAfter": session.document_revision(),
                "result": value, "ok": true
            })),
            Err(e) => return Ok(json!({
                "status": "PARTIAL_OR_FAILED", "applied": false,
                "recovery": "Inspect native undo stack; no automatic rollback was attempted",
                "failedStepId": step.id, "error": e.to_string(),
                "revision": session.document_revision(), "receipts": receipts,
                "nativePreflight": initial
            })),
        }
    }
    Ok(json!({
        "status": "EXECUTED_NATIVE_REQUIRES_VISUAL_QC",
        "applied": true, "receipts": receipts, "revision": session.document_revision(),
        "nativePreflight": initial, "saveExportPerformed": false,
        "humanCreativeReviewRequired": true
    }))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    fn authority() -> LocalMotionAuthority {
        LocalMotionAuthority { project_id: "after-test-1".into(), comp_id: 1, revision: 4,
          fps_num: 30000, fps_den: 1001, approved: false }
    }
    fn plan() -> MotionPlan {
        MotionPlan {
          schema: PLAN_SCHEMA.into(), product: "aftertron".into(), plan_id: "motion-1".into(),
          project_id: "after-test-1".into(), comp_id: 1, expected_revision: 4,
          fps_num: 30000, fps_den: 1001,
          steps: vec![MotionStep { id: "text-1".into(), tool: "after.layer.create_text".into(),
            params: json!({"text": "TRON", "name": "Editable Title"}) }]
        }
    }
    #[test]
    fn recognizes_real_native_layer_tool_and_rejects_wrong_comp() {
        assert!(preflight(&plan(), &authority()).is_ok());
        let mut p = plan(); p.comp_id = 2; assert!(preflight(&p, &authority()).is_err());
    }
    #[test]
    fn refuses_unapproved_unsafe_commands_and_stale_revisions() {
        let mut p = plan(); p.expected_revision = 99; assert!(preflight(&p, &authority()).is_err());
        p = plan(); p.steps[0].tool = "after.expression.set".into();
        assert!(preflight(&p, &authority()).is_err());
        p = plan(); p.steps[0].params = json!({"text":"x", "script":"delete()" });
        assert!(preflight(&p, &authority()).is_err());
    }
    #[test]
    fn approval_is_not_model_supplied() {
        let mut session = Session::default();
        assert!(execute_approved(&mut session, &plan(), &authority()).is_err());
    }
}
