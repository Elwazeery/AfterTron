//! Native AfterTron local editing authority: user reviews a plan then approves
//! the SAME plan against the SAME live composition/revision. No fabricated MCP
//! credentials: this is a deliberately bounded local UI capability subset.
use std::hash::{Hash, Hasher};

use effectcraft_automation::tron::{
    LocalMotionAuthority, MotionPlan, execute_approved, preflight,
};
use effectcraft_engine::Session;
use serde_json::{Value, json};

pub fn project_fingerprint(session: &Session) -> String {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    session.project.name.hash(&mut h);
    session.path.hash(&mut h);
    // Each opened project has a distinct original snapshot even when it shares
    // a filename with another document. It changes on save -> old plans stale.
    (std::sync::Arc::as_ptr(&session.saved_project) as usize).hash(&mut h);
    std::process::id().hash(&mut h);
    format!("aftertron-local-{:016x}", h.finish())
}

fn authority(session: &Session, approved: bool) -> Result<LocalMotionAuthority, String> {
    let id = session.active_comp_id().ok_or("AFTERTRON_NO_ACTIVE_COMPOSITION")?;
    let comp = session.active_comp().ok_or("AFTERTRON_ACTIVE_COMP_MISSING")?;
    Ok(LocalMotionAuthority {
        project_id: project_fingerprint(session),
        comp_id: id.0,
        revision: session.document_revision(),
        fps_num: comp.frame_rate.num,
        fps_den: comp.frame_rate.den,
        approved,
    })
}

fn checked_proposal(raw: &Value) -> Result<MotionPlan, String> {
    let response = raw.get("result").unwrap_or(raw);
    if response.get("ok").and_then(Value::as_bool) == Some(false) {
        return Err("AFTERTRON_PLANNER_REJECTED_THIS_PROPOSAL".into());
    }
    let plan = response.get("plan").ok_or("AFTERTRON_NO_PLAN")?;
    serde_json::from_value(plan.clone()).map_err(|e| format!("AFTERTRON_INVALID_PLAN: {e}"))
}

fn digest_plan(plan: &MotionPlan) -> Result<String, String> {
    let bytes = serde_json::to_vec(plan).map_err(|e| format!("PLAN_DIGEST_ENCODING_FAILED: {e}"))?;
    let mut h = std::collections::hash_map::DefaultHasher::new();
    bytes.hash(&mut h);
    Ok(format!("{:016x}", h.finish()))
}

pub fn review(session: &Session, response: &Value) -> Result<Value, String> {
    let plan = checked_proposal(response)?;
    let trusted = authority(session, false)?;
    let details = preflight(&plan, &trusted)?;
    Ok(json!({
        "status": "REVIEWED_NOT_EXECUTED",
        "planId": plan.plan_id,
        "planDigest": digest_plan(&plan)?,
        "revision": trusted.revision,
        "projectId": trusted.project_id,
        "compId": trusted.comp_id,
        "nativePreflight": details,
        "requiresUserConfirmation": true
    }))
}

pub fn apply(session: &mut Session, response: &Value, reviewed_id: &str, reviewed_revision: u64, reviewed_digest: &str) -> Result<Value, String> {
    let plan = checked_proposal(response)?;
    if plan.plan_id != reviewed_id || plan.expected_revision != reviewed_revision ||
        session.document_revision() != reviewed_revision || digest_plan(&plan)? != reviewed_digest {
        return Err("AFTERTRON_STALE_OR_CHANGED_PLAN".into());
    }
    let trusted = authority(session, true)?;
    let receipt = execute_approved(session, &plan, &trusted)?;
    Ok(json!({
        "native": receipt,
        "afterRevision": session.document_revision(),
        "projectId": project_fingerprint(session),
        "compId": session.active_comp_id().map(|x| x.0),
        "requiresRenderedFrameInspection": true,
        "saveRenderAndAlphaQcNotImplicit": true
    }))
}

pub fn live_context(session: &Session) -> Option<Value> {
    let id = session.active_comp_id()?;
    let comp = session.active_comp()?;
    Some(json!({
        "projectId": project_fingerprint(session),
        "compId": id.0,
        "revision": session.document_revision(),
        "fpsNum": comp.frame_rate.num,
        "fpsDen": comp.frame_rate.den,
        "width": comp.width,
        "height": comp.height,
        "durationTicks": comp.duration.0,
        "layers": comp.layers.iter().take(40).map(|layer|
            json!({"id": layer.id.0, "name": layer.name})
        ).collect::<Vec<_>>(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn without_composition_no_native_authority_is_granted() {
        let s = Session::default();
        assert!(authority(&s, false).is_err());
        assert!(live_context(&s).is_none());
    }
}
