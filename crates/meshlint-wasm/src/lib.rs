use meshlint_core::{
    LintOptions, fix_mesh_bytes as core_fix_mesh, lint_mesh_bytes as core_lint_mesh,
};
use wasm_bindgen::prelude::*;

#[wasm_bindgen(js_name = lintMesh)]
pub fn lint_mesh(bytes: &[u8], format: &str, options: JsValue) -> Result<JsValue, JsValue> {
    let options = parse_options(options)?;
    let report = core_lint_mesh(bytes, format, options).map_err(to_js_error)?;
    serde_wasm_bindgen::to_value(&report).map_err(to_js_error)
}

#[wasm_bindgen(js_name = fixMesh)]
pub fn fix_mesh(bytes: &[u8], format: &str, options: JsValue) -> Result<JsValue, JsValue> {
    let options = parse_options(options)?;
    let report = core_fix_mesh(bytes, format, options).map_err(to_js_error)?;
    serde_wasm_bindgen::to_value(&report).map_err(to_js_error)
}

fn parse_options(value: JsValue) -> Result<LintOptions, JsValue> {
    if value.is_null() || value.is_undefined() {
        Ok(LintOptions::default())
    } else {
        serde_wasm_bindgen::from_value(value).map_err(to_js_error)
    }
}

fn to_js_error(error: impl std::fmt::Display) -> JsValue {
    JsValue::from_str(&error.to_string())
}
