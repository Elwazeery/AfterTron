//! AfterTron × TRON native floating motion director.
//! Local AI is optional, OFF on launch, shared with PhotoTron/FilmTron.
//! This chat never creates a second model/runtime and never calls shell or
//! submits model output straight to the native engine without user approval.
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use egui::{Align2, Color32, Context, Id, Order, RichText, ScrollArea, vec2};
use serde_json::Value;

struct ChatState {
    open: bool,
    working: bool,
    power_on: bool,
    transport_connected: bool,
    model_state: String,
    model: String,
    thinking: bool,
    warmed: bool,
    leases: u64,
    error: Option<String>,
    prompt: String,
    proposal: Option<Value>,
    reviewed_id: Option<String>,
    reviewed_revision: Option<u64>,
    review: Option<Value>,
    execution: Option<Value>,
    last_status: Instant,
}

impl Default for ChatState {
    fn default() -> Self {
        Self {
            open: false, working: false, power_on: false, transport_connected: false,
            model_state: "NOT_LOADED".into(), model: "qwen3.5:4b-q4_K_M".into(),
            thinking: false, warmed: false, leases: 0, error: None,
            prompt: String::new(), proposal: None,
            reviewed_id: None, reviewed_revision: None, review: None,
            execution: None, last_status: Instant::now(),
        }
    }
}

fn shared(ctx: &Context) -> Arc<Mutex<ChatState>> {
    let id = Id::new("aftertron.tron.local.view");
    ctx.data_mut(|data| match data.get_temp::<Arc<Mutex<ChatState>>>(id) {
        Some(state) => state,
        None => {
            let s = Arc::new(Mutex::new(ChatState::default()));
            data.insert_temp(id, s.clone());
            s
        }
    })
}

#[cfg(not(target_arch = "wasm32"))]
fn update_status(state: &mut ChatState, raw: &Value) {
    let status = raw.get("status").unwrap_or(raw);
    state.transport_connected = true;
    state.model_state = status.get("state").and_then(Value::as_str).unwrap_or("UNKNOWN").to_owned();
    state.model = status.get("model").and_then(Value::as_str).unwrap_or("qwen3.5:4b-q4_K_M").to_owned();
    state.thinking = status.get("thinkingVerified").and_then(Value::as_bool).unwrap_or(false);
    state.warmed = status.get("warmupVerified").and_then(Value::as_bool).unwrap_or(false);
    state.leases = status.get("activeLeaseCount").and_then(Value::as_u64).unwrap_or(0);
}

/// ALL broker I/O runs off the UI frame thread.
#[cfg(not(target_arch = "wasm32"))]
fn action(st: Arc<Mutex<ChatState>>, ctx: Context, task: &'static str) {
    let Ok(mut state) = st.lock() else { return };
    if state.working { return; }
    state.working = true;
    state.error = None;
    drop(state);
    std::thread::spawn(move || {
        let reply = match task {
            "on" => effectcraft_automation::tron_local::activate(),
            "off" => effectcraft_automation::tron_local::release(),
            "minimize" => effectcraft_automation::tron_local::minimize(),
            "heartbeat" => effectcraft_automation::tron_local::heartbeat(),
            "cancel" => effectcraft_automation::tron_local::cancel_job(),
            _ => effectcraft_automation::tron_local::status(),
        };
        if let Ok(mut state) = st.lock() {
            state.working = false;
            state.last_status = Instant::now();
            match reply {
                Ok(value) => {
                    update_status(&mut state, &value);
                    if task == "on" {
                        state.power_on = state.thinking && state.warmed && state.model_state == "READY";
                        if !state.power_on {
                            state.error = Some("TRON reasoning/warm-up not verified".into());
                        }
                    }
                    if task == "heartbeat" && value.get("alive").and_then(Value::as_bool) != Some(true) {
                        state.power_on = false;
                        state.reviewed_id = None;
                        state.reviewed_revision = None;
                        state.review = None;
                        state.error = Some("AFTERTRON_MODEL_LEASE_EXPIRED".into());
                    }
                    if matches!(task, "off" | "minimize") {
                        state.power_on = false;
                        state.review = None;
                        state.reviewed_id = None;
                        state.reviewed_revision = None;
                    }
                }
                Err(e) => {
                    state.error = Some(e);
                    if task == "on" { state.power_on = false; }
                    if task == "status" { state.transport_connected = false; }
                }
            }
        }
        ctx.request_repaint();
    });
}

#[cfg(not(target_arch = "wasm32"))]
fn generate(st: Arc<Mutex<ChatState>>, ctx: Context, prompt: String, context: Value) {
    let Ok(mut state) = st.lock() else { return };
    if state.working || !state.power_on { return; }
    state.working = true;
    state.error = None;
    state.proposal = None;
    state.review = None;
    state.execution = None;
    state.reviewed_id = None;
    state.reviewed_revision = None;
    drop(state);
    std::thread::spawn(move || {
        let reply = effectcraft_automation::tron_local::prepare_after_plan(&prompt, &context);
        if let Ok(mut state) = st.lock() {
            state.working = false;
            state.last_status = Instant::now();
            match reply {
                Ok(data) => {
                    let result = data.get("result").unwrap_or(&data);
                    if result.get("ok").and_then(Value::as_bool) == Some(false) {
                        state.error = result.get("status").and_then(Value::as_str).map(str::to_owned);
                    }
                    state.proposal = Some(data);
                }
                Err(e) => state.error = Some(e),
            }
        }
        ctx.request_repaint();
    });
}

/// Native app UI state is read at each frame, not captured before model
/// inference. The review and approval steps use the same current Session.
pub fn show(ctx: &Context, session: &mut effectcraft_engine::Session) {
    let st = shared(ctx);
    let is_open = st.lock().map(|s| s.open).unwrap_or(false);
    if !is_open {
        egui::Area::new(Id::new("aftertron.tron.launcher"))
            .order(Order::Foreground)
            .anchor(Align2::RIGHT_BOTTOM, vec2(-22.0, -26.0))
            .show(ctx, |ui| {
                if ui.add(egui::Button::new(RichText::new("TRON · Motion AI").strong())
                    .min_size(vec2(148.0, 42.0))).clicked() {
                    if let Ok(mut s) = st.lock() {
                        s.open = true;
                        s.last_status = Instant::now() - Duration::from_secs(20);
                    }
                }
            });
        return;
    }

    let poll = st.lock().map(|s| !s.working && s.last_status.elapsed() > Duration::from_secs(12))
        .unwrap_or(false);
    #[cfg(not(target_arch = "wasm32"))]
    if poll {
        let active = st.lock().map(|s| s.power_on).unwrap_or(false);
        action(st.clone(), ctx.clone(), if active { "heartbeat" } else { "status" });
    }
    #[cfg(target_arch = "wasm32")]
    let _ = poll;

    let real_context = crate::tron_actions::live_context(session);
    let mut task = None;
    let mut motion_prompt = None;
    let mut native_action: Option<bool> = None;
    let bounds = ctx.content_rect();
    egui::Window::new("TRON · Motion Creative Director")
        .id(Id::new("aftertron.tron.window"))
        .order(Order::Foreground)
        .default_pos(egui::pos2((bounds.right() - 468.0).max(bounds.left() + 8.0), bounds.top() + 65.0))
        .default_size(vec2(442.0, 540.0))
        .min_size(vec2(322.0, 295.0))
        .resizable(true)
        .collapsible(false)
        .show(ctx, |ui| {
            let Ok(mut s) = st.lock() else { return };
            ui.horizontal(|ui| {
                ui.heading("TRON");
                ui.weak("AfterTron · Motion / VFX Director");
                if ui.small_button("—").on_hover_text("تصغير").clicked() {
                    s.open = false;
                    task = Some("minimize");
                }
            });
            ui.separator();
            ui.horizontal_wrapped(|ui| {
                ui.colored_label(if s.transport_connected { Color32::LIGHT_GREEN } else { Color32::GRAY },
                    if s.transport_connected { "● Connected" } else { "○ Offline" });
                ui.label(format!("Model: {}", s.model_state));
                ui.weak(format!("Shared leases: {}", s.leases));
            });
            ui.horizontal(|ui| {
                ui.label(&s.model);
                ui.colored_label(if s.thinking && s.warmed { Color32::LIGHT_GREEN } else { Color32::YELLOW },
                    if s.thinking && s.warmed { "Thinking verified" } else { "Thinking unavailable" });
                if s.working { ui.spinner(); }
            });
            ui.horizontal(|ui| {
                #[cfg(not(target_arch = "wasm32"))]
                {
                    if ui.add_enabled(!s.working && !s.power_on, egui::Button::new("ON")).clicked() {
                        task = Some("on");
                    }
                    if ui.add_enabled(!s.working && s.power_on, egui::Button::new("OFF")).clicked() {
                        task = Some("off");
                    }
                    if ui.add_enabled(s.working, egui::Button::new("Cancel")).clicked() {
                        task = Some("cancel");
                    }
                }
                #[cfg(target_arch = "wasm32")]
                ui.add_enabled(false, egui::Button::new("Local AI desktop only"));
            });
            if let Some(err) = &s.error {
                ui.colored_label(Color32::from_rgb(236, 104, 103), err);
            }
            ui.separator();
            ScrollArea::vertical().max_height(320.0).show(ui, |ui| {
                ui.strong("إخراج موشن متكامل، مش حركة عشوائية.");
                ui.label("هنراجع التكوين والطبقات والتوقيت، ونقارن اتجاهين إبداعيين قبل تنفيذ Keyframes أو Effects.");
                if let Some(last) = s.proposal.clone() {
                    let proposal = last.get("result").unwrap_or(&last);
                    ui.separator();
                    ui.label(proposal.get("userExplanation").and_then(Value::as_str).unwrap_or("Motion direction pending."));
                    if let Some(steps) = proposal.get("plan").and_then(|p| p.get("steps")).and_then(Value::as_array) {
                        ui.strong(format!("{} Native motion operations — not yet executed", steps.len()));
                        for step in steps.iter().take(12) {
                            ui.small(format!("{} · {}",
                                step.get("id").and_then(Value::as_str).unwrap_or("?"),
                                step.get("tool").and_then(Value::as_str).unwrap_or("?")));
                        }
                    }
                    if s.power_on && !s.working && proposal.get("plan").is_some() {
                        if ui.button("1 · Native preflight").clicked() { native_action = Some(false); }
                        if s.review.is_some() {
                            ui.colored_label(Color32::LIGHT_GREEN, "تم فحص الخطة — لم تُنفّذ");
                            if ui.button("2 · أوافق على تطبيق الحركة على الـComposition").clicked() {
                                native_action = Some(true);
                            }
                        }
                    }
                }
                if let Some(result) = &s.execution {
                    ui.separator();
                    ui.strong("Native execution receipt");
                    ui.label(result["native"]["status"].as_str().unwrap_or("UNKNOWN"));
                    ui.small("لازم معاينة فريمات حقيقية، مراجعة الـAlpha، وحفظ وإعادة فتح المشروع قبل قبول الجودة.");
                }
            });
            ui.add_space(6.0);
            let enabled = s.power_on && !s.working && real_context.is_some();
            ui.add_enabled(enabled, egui::TextEdit::multiline(&mut s.prompt)
                .hint_text("اكتب فكرة الموشن والهوية البصرية والتوقيت المطلوب…").desired_rows(3));
            if ui.add_enabled(enabled && !s.prompt.trim().is_empty(), egui::Button::new("فكّر واقترح Motion Plan")).clicked() {
                if let Some(context) = real_context.clone() {
                    motion_prompt = Some((s.prompt.clone(), context));
                }
            }
        });

    #[cfg(not(target_arch = "wasm32"))]
    if let Some(task) = task { action(st.clone(), ctx.clone(), task); }
    #[cfg(not(target_arch = "wasm32"))]
    if let Some((prompt, context)) = motion_prompt {
        generate(st.clone(), ctx.clone(), prompt, context);
    }
    if let Some(approved) = native_action {
        if let Ok(mut s) = st.lock() {
            let result = match s.proposal.clone() {
                Some(plan) if s.power_on && !s.working =>
                    if approved {
                        match (s.reviewed_id.clone(), s.reviewed_revision) {
                            (Some(id), Some(rev)) => crate::tron_actions::apply(session, &plan, &id, rev),
                            _ => Err("AFTERTRON_PLAN_NOT_REVIEWED".into())
                        }
                    } else { crate::tron_actions::review(session, &plan) },
                _ => Err("AFTERTRON_MODEL_OFF_OR_NO_PLAN".into())
            };
            match result {
                Ok(v) if approved => {
                    s.execution = Some(v);
                    s.review = None;
                    s.reviewed_id = None;
                    s.reviewed_revision = None;
                    s.error = None;
                }
                Ok(v) => {
                    s.reviewed_id = v.get("planId").and_then(Value::as_str).map(str::to_owned);
                    s.reviewed_revision = v.get("revision").and_then(Value::as_u64);
                    s.review = Some(v);
                    s.error = None;
                }
                Err(e) => {
                    s.error = Some(e);
                    s.review = None;
                    s.reviewed_id = None;
                    s.reviewed_revision = None;
                }
            }
        }
    }
    #[cfg(target_arch = "wasm32")]
    let _ = (task, motion_prompt);
    ctx.request_repaint_after(Duration::from_secs(2));
}

#[cfg(not(target_arch = "wasm32"))]
pub fn release_on_exit() {
    let _ = effectcraft_automation::tron_local::release();
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn model_is_never_preloaded_on_window_construction() {
        let s = ChatState::default();
        assert!(!s.open);
        assert!(!s.power_on);
        assert!(!s.transport_connected);
        assert_eq!(s.model_state, "NOT_LOADED");
    }
}
