//! GemiMCP product manifest (`gemimcp.product-manifest.v1`) and outbound desktop bridge adapter
//! for AfterTron (`after.*` tool namespace).
//!
//! Exposes high-level semantic motion graphics & VFX tools (`after.composition.*`, `after.layer.*`,
//! `after.transform.*`, `after.property.*`, `after.keyframe.*`, `after.graph.*`, `after.expression.*`,
//! `after.mask.*`, `after.matte.*`, `after.effect.*`, `after.text.*`, `after.camera.*`, `after.light.*`,
//! `after.preview.*`, `after.render.*`, `after.ui.*`, `after.history.*`) alongside 1:1 `after.cmd.*`
//! descriptors for every registered engine command, without duplicating the cloud GemiMCP gateway.

use std::path::PathBuf;

use effectcraft_engine::commands::command_specs;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::security::{AuthorizedWorkspace, ReplayProtector};
use crate::tools::{self, Reply};
use crate::{Backend, Error, Result, base64};

/// Schema identifier for the GemiMCP product manifest contract.
pub const GEMIMCP_MANIFEST_SCHEMA: &str = "gemimcp.product-manifest.v1";

/// Product identifier registered with GemiMCP and Gemi Access.
pub const AFTERTRON_PRODUCT_ID: &str = "aftertron";

/// Tool namespace prefix for AfterTron inside GemiMCP.
pub const AFTERTRON_NAMESPACE: &str = "after";

/// Top-level GemiMCP product manifest for AfterTron.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GemiMcpProductManifest {
    pub schema: String,
    pub product_id: String,
    pub product_name: String,
    pub product_version: String,
    pub namespace: String,
    pub upstream_base: String,
    pub auth: GemiMcpAuthContract,
    pub transport: GemiMcpTransportContract,
    pub capabilities: GemiMcpCapabilitiesSummary,
    pub tools: Vec<GemiMcpToolSpec>,
}

/// Authentication & entitlement contract required by the GemiMCP desktop agent.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GemiMcpAuthContract {
    pub issuer: String,
    pub oidc_pkce_required: bool,
    pub required_scopes: Vec<String>,
    pub license_guard: String,
    pub local_control_auth: String,
}

/// Outbound desktop agent transport contract.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GemiMcpTransportContract {
    pub mode: String,
    pub local_bind: String,
    pub supports_cancellation: bool,
    pub supports_optimistic_concurrency: bool,
    pub supports_idempotency_keys: bool,
    pub workspace_root_confinement: bool,
}

/// High-level summary of verified AfterTron capabilities exposed to GemiMCP.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GemiMcpCapabilitiesSummary {
    pub semantic_tool_count: usize,
    pub engine_command_count: usize,
    pub total_tool_count: usize,
    pub registered_effect_count: usize,
    pub domains: Vec<String>,
}

/// Individual tool descriptor in the `after.*` namespace.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GemiMcpToolSpec {
    pub name: String,
    pub domain: String,
    pub title: String,
    pub description: String,
    pub engine_target: String,
    pub read_only: bool,
    pub mutating: bool,
    pub supports_expected_revision: bool,
    pub supports_idempotency_key: bool,
    pub input_schema: Value,
}

/// Incoming envelope from the GemiMCP outbound desktop agent.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GemiMcpToolEnvelope {
    pub request_id: String,
    pub tool: String,
    #[serde(default)]
    pub arguments: Value,
    #[serde(default)]
    pub expected_revision: Option<u64>,
    #[serde(default)]
    pub idempotency_key: Option<String>,
    #[serde(default)]
    pub workspace_roots: Option<GemiMcpWorkspaceRoots>,
}

/// Optional per-request workspace root confinement passed in a `GemiMcpToolEnvelope`.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GemiMcpWorkspaceRoots {
    #[serde(default)]
    pub read_roots: Vec<PathBuf>,
    #[serde(default)]
    pub write_roots: Vec<PathBuf>,
}

/// Outgoing response envelope returned to the GemiMCP outbound desktop agent.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GemiMcpToolResponse {
    pub request_id: String,
    pub ok: bool,
    pub document_revision: u64,
    pub replayed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

struct SemanticSeed {
    name: &'static str,
    domain: &'static str,
    title: &'static str,
    description: &'static str,
    engine_target: &'static str,
    read_only: bool,
    schema: Value,
}

fn semantic_seeds() -> Vec<SemanticSeed> {
    vec![
        SemanticSeed {
            name: "after.project.summary",
            domain: "project",
            title: "Project Summary",
            description: "Inspect project path, dirty flag, bit depth, items, active composition, current time, selected layers, undo stack, and authoritative document revision.",
            engine_target: "project.summary",
            read_only: true,
            schema: json!({"type": "object", "properties": {}, "additionalProperties": false}),
        },
        SemanticSeed {
            name: "after.project.open",
            domain: "project",
            title: "Open or Create Project",
            description: "Open an `.ecproj` project from disk (`path`), open the built-in demo project (`demo: true`), or start a fresh empty project (`new: true`).",
            engine_target: "tool:open_project",
            read_only: false,
            schema: json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string", "description": "Path to an `.ecproj` project file."},
                    "demo": {"type": "boolean", "description": "Open the built-in demo project."},
                    "new": {"type": "boolean", "description": "Reset to a fresh empty project."}
                },
                "additionalProperties": false
            }),
        },
        SemanticSeed {
            name: "after.project.save",
            domain: "project",
            title: "Save Project",
            description: "Save the open project in place or to a new `.ecproj` path (`path`).",
            engine_target: "tool:save_project",
            read_only: false,
            schema: json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string", "description": "Destination `.ecproj` path."}
                },
                "additionalProperties": false
            }),
        },
        SemanticSeed {
            name: "after.project.import_footage",
            domain: "project",
            title: "Import Footage Asset",
            description: "Import an image, video, audio, or sequence file into the project.",
            engine_target: "file.import",
            read_only: false,
            schema: json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string", "description": "File path to import."},
                    "sequence": {"type": "boolean", "description": "Import numbered files as an image sequence."}
                },
                "required": ["path"],
                "additionalProperties": false
            }),
        },
        SemanticSeed {
            name: "after.composition.create",
            domain: "composition",
            title: "Create Composition",
            description: "Create a new motion graphics composition with explicit dimensions, frame rate, duration (seconds), and background color.",
            engine_target: "comp.new",
            read_only: false,
            schema: json!({
                "type": "object",
                "properties": {
                    "name": {"type": "string"},
                    "width": {"type": "integer", "minimum": 1, "maximum": 30000},
                    "height": {"type": "integer", "minimum": 1, "maximum": 30000},
                    "frameRate": {"type": "number", "minimum": 1.0, "maximum": 240.0},
                    "duration": {"type": "number", "minimum": 0.01},
                    "background": {"type": "string", "description": "Hex color `#rrggbb`."}
                },
                "additionalProperties": false
            }),
        },
        SemanticSeed {
            name: "after.composition.inspect",
            domain: "composition",
            title: "Inspect Composition",
            description: "Read a composition's resolution, frame rate, duration, work area, background color, and ordered layer stack.",
            engine_target: "comp.info",
            read_only: true,
            schema: json!({
                "type": "object",
                "properties": {
                    "comp": {"type": ["integer", "string"], "description": "Composition ID or name; omit for active comp."}
                },
                "additionalProperties": false
            }),
        },
        SemanticSeed {
            name: "after.composition.settings",
            domain: "composition",
            title: "Update Composition Settings",
            description: "Modify an existing composition's name, width, height, frameRate, duration, or background color.",
            engine_target: "comp.settings",
            read_only: false,
            schema: json!({
                "type": "object",
                "properties": {
                    "comp": {"type": ["integer", "string"]},
                    "name": {"type": "string"},
                    "width": {"type": "integer"},
                    "height": {"type": "integer"},
                    "frameRate": {"type": "number"},
                    "duration": {"type": "number"},
                    "background": {"type": "string"}
                },
                "additionalProperties": false
            }),
        },
        SemanticSeed {
            name: "after.composition.precompose",
            domain: "composition",
            title: "Pre-compose Layers",
            description: "Pre-compose the selected or specified layers into a nested composition.",
            engine_target: "layer.precompose",
            read_only: false,
            schema: json!({
                "type": "object",
                "properties": {
                    "name": {"type": "string"},
                    "layers": {"type": "array", "items": {"type": ["integer", "string"]}},
                    "moveAttributes": {"type": "boolean"}
                },
                "additionalProperties": false
            }),
        },
        SemanticSeed {
            name: "after.layer.create_text",
            domain: "layer",
            title: "Create Text Layer",
            description: "Add a typography layer (supports English and Arabic RTL text) to the active composition.",
            engine_target: "layer.newText",
            read_only: false,
            schema: json!({
                "type": "object",
                "properties": {
                    "text": {"type": "string"},
                    "name": {"type": "string"},
                    "size": {"type": "number"},
                    "fill": {"type": "string"},
                    "font": {"type": "string"}
                },
                "additionalProperties": false
            }),
        },
        SemanticSeed {
            name: "after.layer.create_solid",
            domain: "layer",
            title: "Create Solid Layer",
            description: "Add a solid color layer to the active composition.",
            engine_target: "layer.newSolid",
            read_only: false,
            schema: json!({
                "type": "object",
                "properties": {
                    "name": {"type": "string"},
                    "color": {"type": "string"},
                    "width": {"type": "integer"},
                    "height": {"type": "integer"}
                },
                "additionalProperties": false
            }),
        },
        SemanticSeed {
            name: "after.layer.create_shape",
            domain: "layer",
            title: "Create Shape Layer",
            description: "Add a vector shape layer to the active composition.",
            engine_target: "layer.newShape",
            read_only: false,
            schema: json!({
                "type": "object",
                "properties": {
                    "name": {"type": "string"},
                    "shape": {"type": "string"},
                    "size": {"type": "array", "items": {"type": "number"}},
                    "fill": {"type": "string"},
                    "stroke": {"type": "string"},
                    "strokeWidth": {"type": "number"}
                },
                "additionalProperties": false
            }),
        },
        SemanticSeed {
            name: "after.layer.create_null",
            domain: "layer",
            title: "Create Null Object Layer",
            description: "Create a Null Object layer for transform parenting and rigging.",
            engine_target: "layer.newNull",
            read_only: false,
            schema: json!({"type": "object", "properties": {"name": {"type": "string"}}, "additionalProperties": false}),
        },
        SemanticSeed {
            name: "after.layer.create_adjustment",
            domain: "layer",
            title: "Create Adjustment Layer",
            description: "Create an adjustment layer that applies its effects to all layers below it.",
            engine_target: "layer.newAdjustment",
            read_only: false,
            schema: json!({"type": "object", "properties": {"name": {"type": "string"}}, "additionalProperties": false}),
        },
        SemanticSeed {
            name: "after.layer.add_footage",
            domain: "layer",
            title: "Add Imported Footage/Asset Layer",
            description: "Place an already-imported project footage or image item (`itemId`, `item`, or `itemName`) as a layer in the active composition.",
            engine_target: "layer.addItem",
            read_only: false,
            schema: json!({
                "type": "object",
                "properties": {
                    "itemId": {"type": "integer"},
                    "item": {"type": ["integer", "string"]},
                    "itemName": {"type": "string"},
                    "name": {"type": "string"},
                    "position": {"type": "array", "items": {"type": "number"}},
                    "scale": {"type": "array", "items": {"type": "number"}},
                    "opacity": {"type": "number"},
                    "blendMode": {"type": "string"}
                },
                "additionalProperties": false
            }),
        },
        SemanticSeed {
            name: "after.layer.inspect",
            domain: "layer",
            title: "Inspect Layer Property Tree",
            description: "Inspect a layer's full or flat property tree (transform, masks, effects, text, shapes) at a composition time.",
            engine_target: "tool:get_layer",
            read_only: true,
            schema: json!({
                "type": "object",
                "properties": {
                    "layer": {"type": ["integer", "string"]},
                    "comp": {"type": ["integer", "string"]},
                    "time": {"type": "number"},
                    "depth": {"type": "integer"},
                    "flat": {"type": "boolean"}
                },
                "required": ["layer"],
                "additionalProperties": false
            }),
        },
        SemanticSeed {
            name: "after.layer.select",
            domain: "layer",
            title: "Select Layers",
            description: "Select one or more layers in the active composition.",
            engine_target: "layer.select",
            read_only: false,
            schema: json!({
                "type": "object",
                "properties": {
                    "layers": {"type": "array", "items": {"type": ["integer", "string"]}}
                },
                "required": ["layers"],
                "additionalProperties": false
            }),
        },
        SemanticSeed {
            name: "after.layer.set_parent",
            domain: "layer",
            title: "Set Layer Parent",
            description: "Parent a child layer to a parent layer (`parent: null` clears parenting).",
            engine_target: "layer.setParent",
            read_only: false,
            schema: json!({
                "type": "object",
                "properties": {
                    "layer": {"type": ["integer", "string"]},
                    "parent": {"type": ["integer", "string", "null"]}
                },
                "required": ["layer"],
                "additionalProperties": false
            }),
        },
        SemanticSeed {
            name: "after.layer.set_blend_mode",
            domain: "layer",
            title: "Set Layer Blend Mode",
            description: "Set the compositing blend mode of a layer (e.g., Normal, Multiply, Screen, Add, Overlay).",
            engine_target: "layer.setBlendMode",
            read_only: false,
            schema: json!({
                "type": "object",
                "properties": {
                    "layer": {"type": ["integer", "string"]},
                    "mode": {"type": "string"}
                },
                "required": ["mode"],
                "additionalProperties": false
            }),
        },
        SemanticSeed {
            name: "after.layer.set_in_out",
            domain: "layer",
            title: "Set Layer In/Out Points",
            description: "Set a layer's `in`, `out`, or `start` time in seconds.",
            engine_target: "layer.timing",
            read_only: false,
            schema: json!({
                "type": "object",
                "properties": {
                    "layer": {"type": ["integer", "string"]},
                    "in": {"type": "number"},
                    "out": {"type": "number"},
                    "start": {"type": "number"},
                    "stretch": {"type": "number"}
                },
                "additionalProperties": false
            }),
        },
        SemanticSeed {
            name: "after.property.get",
            domain: "property",
            title: "Get Property Value & Keyframes",
            description: "Read a property's current or timed value, animation state, keyframes, and expression.",
            engine_target: "tool:get_property",
            read_only: true,
            schema: json!({
                "type": "object",
                "properties": {
                    "layer": {"type": ["integer", "string"]},
                    "path": {"type": "string"},
                    "comp": {"type": ["integer", "string"]},
                    "time": {"type": "number"}
                },
                "required": ["layer", "path"],
                "additionalProperties": false
            }),
        },
        SemanticSeed {
            name: "after.property.set",
            domain: "property",
            title: "Set Property Value or Keyframe",
            description: "Set a static property value, add/update a keyframe at `time`, or attach an `expression`.",
            engine_target: "tool:set_property",
            read_only: false,
            schema: json!({
                "type": "object",
                "properties": {
                    "layer": {"type": ["integer", "string"]},
                    "path": {"type": "string"},
                    "value": {},
                    "comp": {"type": ["integer", "string"]},
                    "time": {"type": "number"},
                    "timeBase": {"type": "string", "enum": ["layer", "comp"]},
                    "expression": {"type": "string"}
                },
                "required": ["layer", "path"],
                "additionalProperties": false
            }),
        },
        SemanticSeed {
            name: "after.keyframe.add",
            domain: "keyframe",
            title: "Add Keyframes & Easing",
            description: "Add one or multiple keyframes to a property and optionally set interpolation (`linear`, `bezier`, `hold`, `easyEase`, `easyEaseIn`, `easyEaseOut`).",
            engine_target: "tool:add_keyframe",
            read_only: false,
            schema: json!({
                "type": "object",
                "properties": {
                    "layer": {"type": ["integer", "string"]},
                    "path": {"type": "string"},
                    "comp": {"type": ["integer", "string"]},
                    "time": {"type": "number"},
                    "timeBase": {"type": "string", "enum": ["layer", "comp"]},
                    "value": {},
                    "keys": {"type": "array", "items": {"type": "object"}},
                    "interpolation": {"type": "string", "enum": ["linear", "bezier", "hold", "easyEase", "easyEaseIn", "easyEaseOut"]}
                },
                "required": ["layer", "path"],
                "additionalProperties": false
            }),
        },
        SemanticSeed {
            name: "after.keyframe.easy_ease",
            domain: "keyframe",
            title: "Apply Easy Ease",
            description: "Apply Easy Ease (`F9`), Easy Ease In, or Easy Ease Out to selected keyframes.",
            engine_target: "keys.easyEase",
            read_only: false,
            schema: json!({
                "type": "object",
                "properties": {
                    "which": {"type": "string", "enum": ["both", "in", "out"]}
                },
                "additionalProperties": false
            }),
        },
        SemanticSeed {
            name: "after.graph.set_editor_open",
            domain: "graph",
            title: "Toggle Graph Editor",
            description: "Open or close the Timeline Graph Editor for value and speed curve editing.",
            engine_target: "timeline.graphEditor",
            read_only: false,
            schema: json!({
                "type": "object",
                "properties": {
                    "open": {"type": "boolean"}
                },
                "additionalProperties": false
            }),
        },
        SemanticSeed {
            name: "after.expression.set",
            domain: "expression",
            title: "Set Property Expression",
            description: "Attach or clear a JavaScript expression on a property.",
            engine_target: "prop.setExpression",
            read_only: false,
            schema: json!({
                "type": "object",
                "properties": {
                    "layer": {"type": ["integer", "string"]},
                    "path": {"type": "string"},
                    "comp": {"type": ["integer", "string"]},
                    "expression": {"type": "string"}
                },
                "required": ["layer", "path", "expression"],
                "additionalProperties": false
            }),
        },
        SemanticSeed {
            name: "after.mask.add",
            domain: "mask",
            title: "Add Layer Mask",
            description: "Create a rectangular or elliptical vector mask on a layer with mode (`add`, `subtract`, `intersect`) and feather.",
            engine_target: "layer.addMask",
            read_only: false,
            schema: json!({
                "type": "object",
                "properties": {
                    "layer": {"type": ["integer", "string"]},
                    "shape": {"type": "string", "enum": ["rect", "ellipse"]},
                    "rect": {"type": "array", "items": {"type": "number"}},
                    "mode": {"type": "string"},
                    "inverted": {"type": "boolean"},
                    "feather": {"type": "number"}
                },
                "additionalProperties": false
            }),
        },
        SemanticSeed {
            name: "after.matte.set_track_matte",
            domain: "matte",
            title: "Set Track Matte",
            description: "Assign an Alpha or Luma Track Matte to a layer from another layer in the composition.",
            engine_target: "layer.setTrackMatte",
            read_only: false,
            schema: json!({
                "type": "object",
                "properties": {
                    "layer": {"type": ["integer", "string"]},
                    "matteLayer": {"type": ["integer", "string", "null"]},
                    "mode": {"type": "string", "enum": ["none", "alpha", "alphaInverted", "luma", "lumaInverted"]}
                },
                "additionalProperties": false
            }),
        },
        SemanticSeed {
            name: "after.effect.list",
            domain: "effect",
            title: "List Visual Effects",
            description: "List all registered visual effects with IDs, names, categories, GPU acceleration, and 32-bpc support.",
            engine_target: "tool:list_effects",
            read_only: true,
            schema: json!({
                "type": "object",
                "properties": {
                    "filter": {"type": "string"}
                },
                "additionalProperties": false
            }),
        },
        SemanticSeed {
            name: "after.effect.apply",
            domain: "effect",
            title: "Apply Effect & Parameters",
            description: "Apply an effect by ID or name to a layer and set initial parameter values atomically in one undo step.",
            engine_target: "tool:add_effect",
            read_only: false,
            schema: json!({
                "type": "object",
                "properties": {
                    "layer": {"type": ["integer", "string"]},
                    "comp": {"type": ["integer", "string"]},
                    "effect": {"type": "string"},
                    "values": {"type": "object", "additionalProperties": true}
                },
                "required": ["layer", "effect"],
                "additionalProperties": false
            }),
        },
        SemanticSeed {
            name: "after.text.set_source",
            domain: "text",
            title: "Update Text Layer Typography",
            description: "Update a text layer's string, font family, style, size, fill/stroke colors, tracking, leading, and alignment (supports Arabic RTL).",
            engine_target: "layer.setText",
            read_only: false,
            schema: json!({
                "type": "object",
                "properties": {
                    "layer": {"type": ["integer", "string"]},
                    "text": {"type": "string"},
                    "font": {"type": "string"},
                    "style": {"type": "string"},
                    "size": {"type": "number"},
                    "fill": {"type": "string"},
                    "stroke": {"type": "string"},
                    "strokeWidth": {"type": "number"},
                    "tracking": {"type": "number"}
                },
                "additionalProperties": false
            }),
        },
        SemanticSeed {
            name: "after.text.list_fonts",
            domain: "text",
            title: "List Available Fonts",
            description: "List bundled (including GemiFontSans) and system font families available for text layers.",
            engine_target: "tool:list_fonts",
            read_only: true,
            schema: json!({
                "type": "object",
                "properties": {
                    "query": {"type": "string"},
                    "variable": {"type": "boolean"},
                    "rescan": {"type": "boolean"}
                },
                "additionalProperties": false
            }),
        },
        SemanticSeed {
            name: "after.camera.create",
            domain: "camera",
            title: "Create 3D Camera Layer",
            description: "Add a 3D camera layer to the active composition.",
            engine_target: "layer.newCamera",
            read_only: false,
            schema: json!({
                "type": "object",
                "properties": {
                    "name": {"type": "string"},
                    "focalLength": {"type": "number"}
                },
                "additionalProperties": false
            }),
        },
        SemanticSeed {
            name: "after.light.create",
            domain: "light",
            title: "Create 3D Light Layer",
            description: "Add a 3D light layer (Point, Spot, Parallel, Ambient) to the active composition.",
            engine_target: "layer.newLight",
            read_only: false,
            schema: json!({
                "type": "object",
                "properties": {
                    "name": {"type": "string"},
                    "kind": {"type": "string"},
                    "intensity": {"type": "number"},
                    "color": {"type": "string"}
                },
                "additionalProperties": false
            }),
        },
        SemanticSeed {
            name: "after.preview.render_frame",
            domain: "preview",
            title: "Render Composition Preview Frame",
            description: "Render an exact composition frame at `time` (seconds) to PNG and return image metadata and optional base64/file output.",
            engine_target: "tool:render_frame",
            read_only: true,
            schema: json!({
                "type": "object",
                "properties": {
                    "comp": {"type": ["integer", "string"]},
                    "time": {"type": "number"},
                    "max_side": {"type": "integer"},
                    "path": {"type": "string"},
                    "inline": {"type": "boolean"},
                    "transparent": {"type": "boolean"}
                },
                "additionalProperties": false
            }),
        },
        SemanticSeed {
            name: "after.preview.set_time",
            domain: "preview",
            title: "Set Composition Current Time (CTI)",
            description: "Move the composition playhead (CTI) to an exact time in seconds or frame number.",
            engine_target: "time.set",
            read_only: false,
            schema: json!({
                "type": "object",
                "properties": {
                    "time": {"type": "number"},
                    "frame": {"type": "integer"}
                },
                "additionalProperties": false
            }),
        },
        SemanticSeed {
            name: "after.render.queue_add",
            domain: "render",
            title: "Add Composition to Render Queue",
            description: "Queue a composition for output rendering with format and destination path.",
            engine_target: "renderQueue.add",
            read_only: false,
            schema: json!({
                "type": "object",
                "properties": {
                    "comp": {"type": ["integer", "string"]},
                    "output": {"type": "string"},
                    "format": {"type": "string"}
                },
                "additionalProperties": false
            }),
        },
        SemanticSeed {
            name: "after.render.queue_list",
            domain: "render",
            title: "List Render Queue Items",
            description: "Inspect all items in the Render Queue, including status, format, and output paths.",
            engine_target: "renderQueue.list",
            read_only: true,
            schema: json!({"type": "object", "properties": {}, "additionalProperties": false}),
        },
        SemanticSeed {
            name: "after.ui.inspect",
            domain: "ui",
            title: "Inspect UI & Editor State",
            description: "Read the editor state, active workspace, open panels, selection, and application capabilities.",
            engine_target: "tool:get_state",
            read_only: true,
            schema: json!({"type": "object", "properties": {}, "additionalProperties": false}),
        },
        SemanticSeed {
            name: "after.ui.screenshot",
            domain: "ui",
            title: "Capture Live Window or Panel Screenshot",
            description: "Capture a PNG screenshot of the live AfterTron window or a specific panel (`Composition`, `Timeline`, `EffectControls`, `Project`).",
            engine_target: "tool:screenshot",
            read_only: true,
            schema: json!({
                "type": "object",
                "properties": {
                    "panel": {"type": "string"},
                    "id": {"type": "string"},
                    "max_side": {"type": "integer"},
                    "path": {"type": "string"}
                },
                "additionalProperties": false
            }),
        },
        SemanticSeed {
            name: "after.history.undo",
            domain: "history",
            title: "Undo Edits",
            description: "Undo the last `steps` edits (default 1) and return the updated undo stack.",
            engine_target: "tool:undo",
            read_only: false,
            schema: json!({
                "type": "object",
                "properties": {
                    "steps": {"type": "integer", "minimum": 1}
                },
                "additionalProperties": false
            }),
        },
        SemanticSeed {
            name: "after.history.redo",
            domain: "history",
            title: "Redo Edits",
            description: "Redo the last `steps` undone edits (default 1) and return the updated undo stack.",
            engine_target: "tool:redo",
            read_only: false,
            schema: json!({
                "type": "object",
                "properties": {
                    "steps": {"type": "integer", "minimum": 1}
                },
                "additionalProperties": false
            }),
        },
        SemanticSeed {
            name: "after.command.execute",
            domain: "command",
            title: "Execute Any Engine Command",
            description: "Execute any registered AfterTron engine command by ID with JSON parameters.",
            engine_target: "tool:execute_command",
            read_only: false,
            schema: json!({
                "type": "object",
                "properties": {
                    "command": {"type": "string"},
                    "params": {"type": "object", "additionalProperties": true}
                },
                "required": ["command"],
                "additionalProperties": false
            }),
        },
        SemanticSeed {
            name: "after.command.batch",
            domain: "command",
            title: "Execute Atomic Command Batch",
            description: "Run multiple engine commands atomically as a single undo step (`engine.batch`), with `$N` step result interpolation.",
            engine_target: "tool:batch",
            read_only: false,
            schema: json!({
                "type": "object",
                "properties": {
                    "steps": {"type": "array", "items": {"type": "object"}},
                    "label": {"type": "string"},
                    "atomic": {"type": "boolean"}
                },
                "required": ["steps"],
                "additionalProperties": false
            }),
        },
    ]
}

/// Build the complete `gemimcp.product-manifest.v1` descriptor for AfterTron.
pub fn build_aftertron_product_manifest() -> GemiMcpProductManifest {
    let seeds = semantic_seeds();
    let semantic_count = seeds.len();
    let mut tools = Vec::with_capacity(semantic_count + command_specs().len());

    let mut domains = Vec::new();
    for s in seeds {
        if !domains.iter().any(|d| d == s.domain) {
            domains.push(s.domain.to_string());
        }
        tools.push(GemiMcpToolSpec {
            name: s.name.to_string(),
            domain: s.domain.to_string(),
            title: s.title.to_string(),
            description: s.description.to_string(),
            engine_target: s.engine_target.to_string(),
            read_only: s.read_only,
            mutating: !s.read_only,
            supports_expected_revision: !s.read_only,
            supports_idempotency_key: !s.read_only,
            input_schema: s.schema,
        });
    }

    let specs = command_specs();
    for c in specs {
        let domain = c.id.split('.').next().unwrap_or("cmd").to_string();
        if !domains.contains(&domain) {
            domains.push(domain.clone());
        }
        let read_only = !c.journal;
        tools.push(GemiMcpToolSpec {
            name: format!("after.cmd.{}", c.id),
            domain,
            title: c.label.to_string(),
            description: format!("Engine command `{}` ({}): params `{}`", c.id, c.label, c.params),
            engine_target: c.id.to_string(),
            read_only,
            mutating: !read_only,
            supports_expected_revision: !read_only,
            supports_idempotency_key: !read_only,
            input_schema: effectcraft_engine::commands::params_schema(c),
        });
    }

    let fx_count = effectcraft_engine::effects::registry().len();

    GemiMcpProductManifest {
        schema: GEMIMCP_MANIFEST_SCHEMA.to_string(),
        product_id: AFTERTRON_PRODUCT_ID.to_string(),
        product_name: "AfterTron".to_string(),
        product_version: env!("CARGO_PKG_VERSION").to_string(),
        namespace: AFTERTRON_NAMESPACE.to_string(),
        upstream_base: "storytold/effectcraft".to_string(),
        auth: GemiMcpAuthContract {
            issuer: "https://auth.gemitron.net".to_string(),
            oidc_pkce_required: true,
            required_scopes: vec![
                "openid".to_string(),
                "profile".to_string(),
                "gemitron.mcp.execute".to_string(),
                "gemitron.product.aftertron".to_string(),
            ],
            license_guard: "gemiguard.v1".to_string(),
            local_control_auth: "per-launch-bearer-token-256bit".to_string(),
        },
        transport: GemiMcpTransportContract {
            mode: "outbound-desktop-agent".to_string(),
            local_bind: "127.0.0.1".to_string(),
            supports_cancellation: true,
            supports_optimistic_concurrency: true,
            supports_idempotency_keys: true,
            workspace_root_confinement: true,
        },
        capabilities: GemiMcpCapabilitiesSummary {
            semantic_tool_count: semantic_count,
            engine_command_count: specs.len(),
            total_tool_count: tools.len(),
            registered_effect_count: fx_count,
            domains,
        },
        tools,
    }
}

/// Outbound desktop bridge adapter that dispatches `GemiMcpToolEnvelope` requests against a
/// local `Backend` (headless or authenticated live app bridge) with optimistic concurrency,
/// idempotency replay protection, and workspace confinement.
pub struct OutboundBridgeAdapter {
    backend: Backend,
    replay: ReplayProtector,
}

impl OutboundBridgeAdapter {
    pub fn new(backend: Backend) -> Self {
        Self {
            backend,
            replay: ReplayProtector::default(),
        }
    }

    pub fn backend_mut(&mut self) -> &mut Backend {
        &mut self.backend
    }

    /// Configure persistent workspace confinement roots on the underlying backend.
    pub fn set_workspace(&mut self, workspace: AuthorizedWorkspace) {
        self.backend.confine(workspace.into_confinement());
    }

    /// Execute a `GemiMcpToolEnvelope` and return an authoritative `GemiMcpToolResponse`.
    pub fn handle_envelope(&mut self, env: GemiMcpToolEnvelope) -> GemiMcpToolResponse {
        if let Some(key) = env.idempotency_key.as_deref().filter(|k| !k.is_empty())
            && let Some(cached) = self.replay.check(key)
            && let Ok(resp) = serde_json::from_value::<GemiMcpToolResponse>(cached)
        {
            return GemiMcpToolResponse {
                request_id: env.request_id,
                replayed: true,
                ..resp
            };
        }

        if let Some(roots) = env.workspace_roots {
            match AuthorizedWorkspace::new(roots.read_roots, roots.write_roots) {
                Ok(ws) => self.backend.confine(ws.into_confinement()),
                Err(e) => {
                    let rev = self.backend.document_revision().unwrap_or(0);
                    return GemiMcpToolResponse {
                        request_id: env.request_id,
                        ok: false,
                        document_revision: rev,
                        replayed: false,
                        result: None,
                        error: Some(e.to_string()),
                    };
                }
            }
        }

        let current_rev = match self.backend.document_revision() {
            Ok(r) => r,
            Err(e) => {
                return GemiMcpToolResponse {
                    request_id: env.request_id,
                    ok: false,
                    document_revision: 0,
                    replayed: false,
                    result: None,
                    error: Some(e.to_string()),
                };
            }
        };

        if let Some(expected) = env.expected_revision
            && expected != current_rev
        {
            return GemiMcpToolResponse {
                request_id: env.request_id,
                ok: false,
                document_revision: current_rev,
                replayed: false,
                result: None,
                error: Some(format!("revision mismatch: expected {expected}, actual {current_rev}")),
            };
        }

        let outcome = self.dispatch_tool(&env.tool, &env.arguments);
        let new_rev = self.backend.document_revision().unwrap_or(current_rev);

        let response = match outcome {
            Ok(val) => GemiMcpToolResponse {
                request_id: env.request_id,
                ok: true,
                document_revision: new_rev,
                replayed: false,
                result: Some(val),
                error: None,
            },
            Err(e) => GemiMcpToolResponse {
                request_id: env.request_id,
                ok: false,
                document_revision: new_rev,
                replayed: false,
                result: None,
                error: Some(e.to_string()),
            },
        };

        if response.ok
            && let Some(key) = env.idempotency_key.filter(|k| !k.is_empty())
            && let Ok(serialized) = serde_json::to_value(&response)
        {
            self.replay.record(key, serialized);
        }

        response
    }

    fn dispatch_tool(&mut self, tool_name: &str, args: &Value) -> Result<Value> {
        let args = if args.is_null() { json!({}) } else { args.clone() };
        if let Some(cmd_id) = tool_name.strip_prefix("after.cmd.") {
            return self.backend.exec(cmd_id, args);
        }

        let seeds = semantic_seeds();
        if let Some(seed) = seeds.iter().find(|s| s.name == tool_name) {
            if let Some(mcp_tool) = seed.engine_target.strip_prefix("tool:") {
                return Self::reply_to_json(tools::run(&mut self.backend, mcp_tool, &args)?);
            }
            return self.backend.exec(seed.engine_target, args);
        }

        // Also allow direct MCP tool names (`get_project`, `add_keyframe`, etc.)
        if tools::find(tool_name).is_some() {
            return Self::reply_to_json(tools::run(&mut self.backend, tool_name, &args)?);
        }

        Err(Error::BadArgs(format!("unknown AfterTron GemiMCP tool `{tool_name}`")))
    }

    fn reply_to_json(reply: Reply) -> Result<Value> {
        match reply {
            Reply::Json(v) => Ok(v),
            Reply::Image { png, mut info } => {
                if let Some(obj) = info.as_object_mut() {
                    obj.insert("pngBase64".to_string(), json!(base64::encode(&png)));
                }
                Ok(info)
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use effectcraft_engine::Session;

    #[test]
    fn manifest_covers_all_domains_and_engine_commands() {
        let m = build_aftertron_product_manifest();
        assert_eq!(m.schema, "gemimcp.product-manifest.v1");
        assert_eq!(m.product_id, "aftertron");
        assert_eq!(m.namespace, "after");
        assert!(m.auth.oidc_pkce_required);
        assert_eq!(m.transport.mode, "outbound-desktop-agent");
        assert!(m.capabilities.semantic_tool_count >= 25);
        assert_eq!(m.capabilities.engine_command_count, command_specs().len());
        assert_eq!(m.tools.len(), m.capabilities.total_tool_count);

        for required in [
            "after.project.summary",
            "after.composition.create",
            "after.layer.create_text",
            "after.layer.create_shape",
            "after.property.set",
            "after.keyframe.add",
            "after.graph.set_editor_open",
            "after.effect.apply",
            "after.mask.add",
            "after.preview.render_frame",
            "after.history.undo",
            "after.history.redo",
        ] {
            assert!(m.tools.iter().any(|t| t.name == required), "missing semantic tool {required}");
        }
    }

    #[test]
    fn outbound_bridge_adapter_executes_and_enforces_revision_and_idempotency() {
        let mut adapter = OutboundBridgeAdapter::new(Backend::headless(Session::default()));
        let r0 = adapter.handle_envelope(GemiMcpToolEnvelope {
            request_id: "req-1".into(),
            tool: "after.project.summary".into(),
            arguments: json!({}),
            expected_revision: None,
            idempotency_key: None,
            workspace_roots: None,
        });
        assert!(r0.ok);
        let initial_rev = r0.document_revision;

        // Create comp with idempotency key
        let r1 = adapter.handle_envelope(GemiMcpToolEnvelope {
            request_id: "req-2".into(),
            tool: "after.composition.create".into(),
            arguments: json!({"name": "HeroComp", "width": 1920, "height": 1080, "frameRate": 30.0, "duration": 5.0}),
            expected_revision: Some(initial_rev),
            idempotency_key: Some("create-hero-1".into()),
            workspace_roots: None,
        });
        assert!(r1.ok);
        assert!(!r1.replayed);
        assert!(r1.document_revision > initial_rev);

        // Replay with same idempotency key returns cached response without re-creating comp
        let r1_replay = adapter.handle_envelope(GemiMcpToolEnvelope {
            request_id: "req-2b".into(),
            tool: "after.composition.create".into(),
            arguments: json!({"name": "HeroComp", "width": 1920, "height": 1080, "frameRate": 30.0, "duration": 5.0}),
            expected_revision: Some(initial_rev),
            idempotency_key: Some("create-hero-1".into()),
            workspace_roots: None,
        });
        assert!(r1_replay.ok);
        assert!(r1_replay.replayed);
        assert_eq!(r1_replay.document_revision, r1.document_revision);

        // Stale expected_revision fails deterministically
        let r_stale = adapter.handle_envelope(GemiMcpToolEnvelope {
            request_id: "req-3".into(),
            tool: "after.layer.create_text".into(),
            arguments: json!({"text": "AfterTron"}),
            expected_revision: Some(initial_rev),
            idempotency_key: None,
            workspace_roots: None,
        });
        assert!(!r_stale.ok);
        assert!(r_stale.error.unwrap().contains("revision mismatch"));
    }

    #[test]
    fn write_canonical_aftertron_manifest_contract_if_requested() {
        let manifest = build_aftertron_product_manifest();
        let json_str = serde_json::to_string_pretty(&manifest).unwrap();
        assert!(json_str.contains("\"gemimcp.product-manifest.v1\""));
        let contract_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("contracts")
            .join("gemimcp-product-aftertron.v1.json");
        if let Some(parent) = contract_path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(&contract_path, format!("{json_str}\n")).unwrap();
    }
}
