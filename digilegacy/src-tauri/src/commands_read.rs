//! 只读查询门面：给 HTTP 服务的便捷端点用。
//!
//! 为什么单独一层：`/api/dashboard` 这类端点如果直接调 `db::*`，就会绕过
//! 命令层里的状态校验（例如锁定态占位符、解锁检查）。这里一律走
//! `dispatch::call`，保证 Web 端读到的数据与桌面端完全同源。

use serde_json::Value;

use crate::dispatch;

fn unwrap_data(v: Value) -> Result<Value, String> {
    let success = v.get("success").and_then(Value::as_bool).unwrap_or(false);
    if !success {
        return Err(v
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("查询失败")
            .to_string());
    }
    Ok(v.get("data").cloned().unwrap_or(Value::Null))
}

pub fn dashboard() -> Result<Value, String> {
    unwrap_data(dispatch::call("get_dashboard_stats", Default::default())?)
}

pub fn assets() -> Result<Value, String> {
    unwrap_data(dispatch::call("get_all_assets", Default::default())?)
}

pub fn asset(id: i64) -> Result<Value, String> {
    let mut args = serde_json::Map::new();
    args.insert("id".into(), serde_json::json!(id));
    unwrap_data(dispatch::call("get_asset_by_id", args)?)
}

pub fn platforms() -> Result<Value, String> {
    unwrap_data(dispatch::call("get_all_platforms", Default::default())?)
}

pub fn heirs() -> Result<Value, String> {
    unwrap_data(dispatch::call("get_heirs", Default::default())?)
}

pub fn dms_status() -> Result<Value, String> {
    unwrap_data(dispatch::call("get_dms_status", Default::default())?)
}

pub fn review_profiles() -> Result<Value, String> {
    unwrap_data(dispatch::call("list_review_profiles", Default::default())?)
}

pub fn report_html(locale: &str) -> Result<String, String> {
    let mut args = serde_json::Map::new();
    args.insert("locale".into(), serde_json::json!(locale));
    unwrap_data(dispatch::call("render_report", args)?)?
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| "报告渲染结果为空".to_string())
}

pub fn guide_html(locale: &str) -> Result<String, String> {
    let mut args = serde_json::Map::new();
    args.insert("locale".into(), serde_json::json!(locale));
    unwrap_data(dispatch::call("render_action_guide", args)?)?
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| "执行清单渲染结果为空".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unwrap_data_surfaces_failure_message() {
        let v = serde_json::json!({"success": false, "message": "未解锁", "data": null});
        assert_eq!(unwrap_data(v).unwrap_err(), "未解锁");
    }

    #[test]
    fn unwrap_data_returns_payload_on_success() {
        let v = serde_json::json!({"success": true, "message": "ok", "data": [1,2,3]});
        assert_eq!(unwrap_data(v).unwrap(), serde_json::json!([1, 2, 3]));
    }
}
