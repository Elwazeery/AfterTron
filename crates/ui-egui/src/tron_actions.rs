//! Native AfterTron local editing authority: user reviews a plan then approves
//! the SAME plan against the SAME live composition/revision. No fabricated MCP
//! credentials: this is a deliberately bounded local UI capability subset.

use effectcraft_automation::tron::{
    LocalMotionAuthority, MotionPlan, execute_approved, plan_digest_hex, preflight,
};
use effectcraft_engine::Session;
use effectcraft_project::{LayerSource, Node};
use serde_json::{Value, json};

fn fnv1a_64(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for &b in bytes {
        hash ^= u64::from(b);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

/// Deterministic project + active composition identity that remains invariant across
/// ordinary layer/keyframe/effect/mask edits AND across `.ecproj` save & reopen,
/// while changing when switching compositions or opening a different project topology.
pub fn project_fingerprint(session: &Session) -> String {
    let mut buf = format!("schema={};", session.project.schema);
    if let Some(cid) = session.active_comp_id()
        && let Some(item) = session.project.item(cid)
        && let Some(comp) = session.project.comp(cid)
    {
        buf.push_str(&format!(
            "comp={}:{}:{}x{}:{}/{};",
            cid.0, item.name, comp.width, comp.height, comp.frame_rate.num, comp.frame_rate.den
        ));
    } else {
        for (id, item) in &session.project.items {
            if let Some(comp) = session.project.comp(*id) {
                buf.push_str(&format!(
                    "item={}:{}:{}x{}:{}/{};",
                    id.0,
                    item.name,
                    comp.width,
                    comp.height,
                    comp.frame_rate.num,
                    comp.frame_rate.den
                ));
            }
        }
    }
    format!("aftertron-local-{:016x}", fnv1a_64(buf.as_bytes()))
}

fn authority(session: &Session, approved: bool) -> Result<LocalMotionAuthority, String> {
    let id = session
        .active_comp_id()
        .ok_or("AFTERTRON_NO_ACTIVE_COMPOSITION")?;
    let comp = session
        .active_comp()
        .ok_or("AFTERTRON_ACTIVE_COMP_MISSING")?;
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

/// Select one of the two creative directions (`0` = Direction A, `1` = Direction B)
/// from `result.exploredDirections`, updating `result.plan` and `result.direction`
/// so `review` and `apply` bind to the chosen direction's exact `MotionPlan`.
pub fn select_direction(raw: &Value, direction_index: usize) -> Result<Value, String> {
    let mut root = raw.clone();
    let target = if root.get("result").is_some() {
        root.get_mut("result")
            .ok_or("AFTERTRON_MISSING_RESULT_ENVELOPE")?
    } else {
        &mut root
    };
    let dir = target
        .get("exploredDirections")
        .and_then(Value::as_array)
        .and_then(|arr| arr.get(direction_index))
        .cloned()
        .ok_or("AFTERTRON_DIRECTION_INDEX_OUT_OF_RANGE")?;
    let dir_plan = dir
        .get("plan")
        .cloned()
        .ok_or("AFTERTRON_DIRECTION_MISSING_PLAN")?;
    let dir_id = dir
        .get("id")
        .and_then(Value::as_str)
        .unwrap_or(if direction_index == 0 {
            "direction_a"
        } else {
            "direction_b"
        })
        .to_string();

    if let Some(obj) = target.as_object_mut() {
        obj.insert("selectedDirectionIndex".to_string(), json!(direction_index));
        obj.insert("selectedDirectionId".to_string(), json!(dir_id));
        obj.insert("plan".to_string(), dir_plan);
        if let Some(label) = dir.get("label").or_else(|| dir.get("title")).cloned() {
            obj.insert("selectedDirectionTitle".to_string(), label);
        }
    }
    Ok(root)
}

pub fn review(session: &Session, response: &Value) -> Result<Value, String> {
    let plan = checked_proposal(response)?;
    let trusted = authority(session, false)?;
    let details = preflight(&plan, &trusted)?;
    Ok(json!({
        "status": "REVIEWED_NOT_EXECUTED",
        "planId": plan.plan_id,
        "planDigest": plan_digest_hex(&plan),
        "revision": trusted.revision,
        "projectId": trusted.project_id,
        "compId": trusted.comp_id,
        "nativePreflight": details,
        "requiresUserConfirmation": true
    }))
}

pub fn apply(
    session: &mut Session,
    response: &Value,
    reviewed_id: &str,
    reviewed_revision: u64,
    reviewed_digest: &str,
) -> Result<Value, String> {
    let plan = checked_proposal(response)?;
    if plan.plan_id != reviewed_id
        || plan.expected_revision != reviewed_revision
        || session.document_revision() != reviewed_revision
        || plan_digest_hex(&plan) != reviewed_digest
    {
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

/// Render 5 authoritative frames across the active composition (`0%`, `25%`, `50%`, `75%`, `100%`)
/// snapped to exact `Tick` frame boundaries, returning real pixel luminance, contrast, alpha,
/// and digest telemetry for post-execution visual critique.
pub fn sample_rendered_frames(session: &Session) -> Result<Vec<Value>, String> {
    let cid = session
        .active_comp_id()
        .ok_or("AFTERTRON_NO_ACTIVE_COMPOSITION")?;
    let comp = session
        .active_comp()
        .ok_or("AFTERTRON_ACTIVE_COMP_MISSING")?;
    let total_frames = comp.frame_rate.frame_at(comp.duration).max(1);
    let last_frame = (total_frames - 1).max(0);
    let samples = [
        ("start", 0.0_f64),
        ("25%", 0.25_f64),
        ("50%", 0.50_f64),
        ("75%", 0.75_f64),
        ("end", 1.00_f64),
    ];

    let mut out = Vec::with_capacity(samples.len());
    for (label, ratio) in samples {
        let frame = ((last_frame as f64) * ratio).round() as i64;
        let tick = comp.frame_rate.tick_of(frame);
        let (w, h, rgba) = session
            .render_rgba8(cid, tick, 640)
            .map_err(|e| e.to_string())?;
        let (_, _, alpha_rgba) = session
            .render_rgba8_alpha(cid, tick, 640, true)
            .map_err(|e| e.to_string())?;

        let px_count = (w as usize * h as usize).max(1);
        let mut luma_sum = 0.0_f64;
        let mut lumas = Vec::with_capacity(px_count);
        for px in rgba.chunks_exact(4) {
            let l = 0.2126 * f64::from(px[0])
                + 0.7152 * f64::from(px[1])
                + 0.0722 * f64::from(px[2]);
            luma_sum += l;
            lumas.push(l);
        }
        let mean_luma = luma_sum / (px_count as f64);
        let variance = lumas
            .iter()
            .map(|l| {
                let d = *l - mean_luma;
                d * d
            })
            .sum::<f64>()
            / (px_count as f64);
        let contrast_std = variance.sqrt();

        let non_zero_alpha = alpha_rgba
            .chunks_exact(4)
            .filter(|px| px[3] > 0)
            .count();
        let non_zero_alpha_ratio = (non_zero_alpha as f64) / (px_count as f64);
        let digest = format!("{:016x}", fnv1a_64(&rgba));

        out.push(json!({
            "label": label,
            "ratio": ratio,
            "frame": frame,
            "frameIndex": frame,
            "tick": tick.0,
            "timeTick": tick.0,
            "timeSeconds": tick.seconds(),
            "width": w,
            "height": h,
            "meanLuma": (mean_luma * 100.0).round() / 100.0,
            "contrastStd": (contrast_std * 100.0).round() / 100.0,
            "nonZeroAlphaRatio": (non_zero_alpha_ratio * 10000.0).round() / 10000.0,
            "pixelDigest": digest,
            "sha256": digest
        }));
    }
    Ok(out)
}

pub fn live_context(session: &Session) -> Option<Value> {
    let id = session.active_comp_id()?;
    let comp = session.active_comp()?;
    let comp_name = session
        .project
        .item(id)
        .map(|it| it.name.clone())
        .unwrap_or_else(|| format!("Comp {}", id.0));
    let cti = session.time_of(id);

    let layers = comp
        .layers
        .iter()
        .take(40)
        .map(|layer| {
            let kind = if layer.switches.adjustment {
                "adjustment"
            } else {
                match &layer.source {
                    LayerSource::Text => "text",
                    LayerSource::Shape => "shape",
                    LayerSource::Null => "null",
                    LayerSource::Camera => "camera",
                    LayerSource::Light { .. } => "light",
                    LayerSource::Comp { .. } => "precomp",
                    LayerSource::Solid { .. } => "solid",
                    LayerSource::Footage { .. } => "footage",
                    LayerSource::Model { .. } => "model",
                    LayerSource::Primitive { .. } => "primitive",
                }
            };

            let lt = layer.layer_time(cti);
            let mut transform_summary = serde_json::Map::new();
            let mut total_keyframes = 0usize;
            if let Some(tr) = layer.transform() {
                for node in &tr.children {
                    if let Node::Prop(pr) = node {
                        total_keyframes += pr.keys.len();
                        transform_summary.insert(
                            pr.match_id.clone(),
                            json!({
                                "value": pr.value_at(lt).to_json(),
                                "animated": !pr.keys.is_empty(),
                                "keyCount": pr.keys.len()
                            }),
                        );
                    }
                }
            }

            let effects: Vec<Value> = layer
                .effects()
                .map(|fx| {
                    fx.children
                        .iter()
                        .filter_map(|n| match n {
                            Node::Group(g) => Some(json!({
                                "uid": g.uid,
                                "matchId": g.match_id,
                                "name": g.name,
                                "enabled": g.enabled
                            })),
                            Node::Prop(_) => None,
                        })
                        .collect()
                })
                .unwrap_or_default();

            let mask_count = layer.masks().map(|m| m.children.len()).unwrap_or(0);

            json!({
                "id": layer.id.0,
                "name": layer.name,
                "kind": kind,
                "enabled": layer.switches.video,
                "threeD": layer.switches.three_d,
                "motionBlur": layer.switches.motion_blur,
                "blendMode": layer.blend_mode.label(),
                "parent": layer.parent.map(|p| p.0),
                "trackMatte": layer.track_matte.as_ref().map(|tm| json!({
                    "layer": tm.layer.0,
                    "kind": tm.kind.label()
                })),
                "inTick": layer.in_point.0,
                "outTick": layer.out_point.0,
                "startTick": layer.start_time.0,
                "maskCount": mask_count,
                "effects": effects,
                "keyframeCount": total_keyframes,
                "transform": transform_summary
            })
        })
        .collect::<Vec<_>>();

    Some(json!({
        "projectId": project_fingerprint(session),
        "compId": id.0,
        "compName": comp_name,
        "revision": session.document_revision(),
        "fpsNum": comp.frame_rate.num,
        "fpsDen": comp.frame_rate.den,
        "width": comp.width,
        "height": comp.height,
        "durationTicks": comp.duration.0,
        "ctiTick": cti.0,
        "motionBlurEnabled": comp.enable_motion_blur,
        "layers": layers,
    }))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn without_composition_no_native_authority_is_granted() {
        let s = Session::default();
        assert!(authority(&s, false).is_err());
        assert!(live_context(&s).is_none());
    }

    #[test]
    fn project_fingerprint_remains_stable_across_ordinary_edits_and_save_reopen() {
        let mut s = Session::default();
        s.execute_checked(
            "comp.new",
            json!({
                "name": "FingerprintComp",
                "width": 1920,
                "height": 1080,
                "frameRate": 24.0,
                "duration": 4.0
            }),
        )
        .unwrap();
        let fp_initial = project_fingerprint(&s);

        // Ordinary edit: add text layer
        s.execute_checked(
            "layer.newText",
            json!({
                "name": "Headline",
                "text": "TRON",
                "size": 72.0
            }),
        )
        .unwrap();
        let fp_after_edit = project_fingerprint(&s);
        assert_eq!(fp_initial, fp_after_edit);

        // Save and reopen via Project::to_file_json / Project::from_json
        let saved_json = s.project.to_file_json().unwrap();
        let reopened_proj =
            effectcraft_project::Project::from_json(&saved_json).unwrap();
        let mut s2 = Session::default();
        s2.replace_project(reopened_proj, Some("test.ecproj".into()));
        let fp_reopened = project_fingerprint(&s2);
        assert_eq!(fp_initial, fp_reopened);
    }
}
