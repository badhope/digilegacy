//! DOM-05 deliverable export: real HTML files for report/checklist/legal/toolkit.
//!
//! Rendering is synchronous and bounded by the local dataset. The job row is
//! durable so the HTTP download route can resolve the file later. Every file
//! is written below one export root and re-checked before download.

use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use chrono::Utc;
use digilegacy_domains::policy::{self, Policy};
use digilegacy_foundation::PlatformError;
use digilegacy_render::{escape_html, render_page};
use digilegacy_store::Store;
use rusqlite::OptionalExtension;
use serde::Deserialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

static JOB_COUNTER: AtomicU64 = AtomicU64::new(0);

const LEGAL_JSON: &str = include_str!("../../../content/legal/templates.json");

#[derive(Debug, Deserialize)]
struct LegalTemplate {
    id: String,
    title: String,
    summary: String,
    sections: Vec<String>,
}

struct AssetLine {
    name: String,
    kind: String,
    platform: String,
    value: String,
    note: String,
    policy: Option<Policy>,
}

struct ExportData {
    holder: String,
    holder_updated: String,
    heirs: Vec<(String, String, bool)>,
    assets: Vec<AssetLine>,
}

struct Labels {
    title: &'static str,
    holder: &'static str,
    heirs: &'static str,
    assets: &'static str,
    documents: &'static str,
    process: &'static str,
    risk: &'static str,
    source: &'static str,
    no_policy: &'static str,
    generated: &'static str,
    provenance: &'static str,
}

fn labels(locale: &str) -> Labels {
    if locale == "en-US" {
        Labels {
            title: "Digital legacy report",
            holder: "Holder",
            heirs: "Heirs",
            assets: "Assets",
            documents: "Required documents",
            process: "Process",
            risk: "Risk notes",
            source: "Source",
            no_policy: "No policy entry found for this platform.",
            generated: "Generated",
            provenance:
                "Policy status is shown as recorded. Unverified entries are not official advice.",
        }
    } else {
        Labels {
            title: "数字遗产报告",
            holder: "持有人",
            heirs: "继承人",
            assets: "资产",
            documents: "所需材料",
            process: "办理流程",
            risk: "风险提示",
            source: "来源",
            no_policy: "该平台尚未收录政策条目。",
            generated: "生成时间",
            provenance: "政策状态按实际记录展示；未核实条目不代表官方口径。",
        }
    }
}

fn export_dir(store: &Store) -> PathBuf {
    if let Ok(path) = std::env::var("DIGILEGACY_EXPORTS") {
        return PathBuf::from(path);
    }
    let parent = store.path().parent().unwrap_or_else(|| Path::new("."));
    parent.join("exports")
}

fn collect(store: &Store) -> Result<ExportData, PlatformError> {
    let policies: HashMap<String, Policy> = policy::all(store)
        .map_err(|e| PlatformError::new("INTERNAL", e.to_string()))?
        .into_iter()
        .map(|p| (p.name.to_lowercase(), p))
        .collect();

    let ((holder, updated), heirs, assets) = store
        .read(|conn| {
            let holder: (String, String) = conn
                .query_row(
                    "SELECT coalesce(full_name,''), coalesce(updated_at,'') FROM user_profile WHERE id=1",
                    [],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .unwrap_or_default();
            let mut heir_stmt = conn.prepare(
                "SELECT name, coalesce(relation,''), is_executor FROM heirs ORDER BY sort_order, id",
            )?;
            let heirs = heir_stmt
                .query_map([], |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, i64>(2)? != 0,
                    ))
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            let mut asset_stmt = conn.prepare(
                "SELECT coalesce(nullif(a.account_description,''), nullif(a.account_identifier,''), '未命名资产'),
                        a.category, coalesce(p.name,''), a.estimated_value, coalesce(a.notes,'')
                 FROM digital_assets a LEFT JOIN platforms p ON p.id=a.platform_id
                 WHERE a.status='active' ORDER BY a.category, a.id",
            )?;
            let assets = asset_stmt
                .query_map([], |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, String>(2)?,
                        r.get::<_, f64>(3)?,
                        r.get::<_, String>(4)?,
                    ))
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            Ok((holder, heirs, assets))
        })
        .map_err(|e| PlatformError::new("INTERNAL", e.to_string()))?;

    let assets = assets
        .into_iter()
        .map(|(name, kind, platform, value, note)| AssetLine {
            policy: policies.get(&platform.to_lowercase()).cloned(),
            name,
            kind,
            platform,
            value: if value == 0.0 {
                String::new()
            } else {
                format!("{value:.2}")
            },
            note,
        })
        .collect();

    Ok(ExportData {
        holder,
        holder_updated: updated,
        heirs,
        assets,
    })
}

fn policy_block(policy: &Policy, labels: &Labels) -> String {
    format!(
        "<div class=\"card\"><h3>{}</h3><p class=\"muted\">{} · {} · {}</p><p><b>{}</b> {}</p><p><b>{}</b> {}</p><p><b>{}</b> {}</p><p class=\"mono\">{}</p>",
        escape_html(&policy.name),
        escape_html(&policy.category),
        escape_html(&policy.estimated_difficulty),
        escape_html(&policy.estimated_time_days),
        labels.documents,
        escape_html(&policy.required_documents),
        labels.process,
        escape_html(&policy.process_description),
        labels.risk,
        escape_html(&policy.risk_warnings),
        labels.source,
    ) + &format!(
        "<p class=\"mono\">{} {}</p></div>",
        escape_html(&policy.provenance.status),
        escape_html(&policy.provenance.source_url)
    )
}

fn build_body(
    kind: &str,
    locale: &str,
    data: &ExportData,
) -> Result<(String, String), PlatformError> {
    let labels = labels(locale);
    let date = Utc::now().to_rfc3339();
    let title = format!("{} · {}", labels.title, kind);
    let mut body = format!(
        "<h1>{}</h1><p class=\"muted\">{} · {}</p>",
        escape_html(labels.title),
        escape_html(labels.title),
        escape_html(&date)
    );

    match kind {
        "report" => {
            body.push_str(&format!(
                "<h2>{}</h2><p>{}</p><p class=\"mono\">{}</p><h2>{}</h2><ul>",
                labels.holder,
                escape_html(if data.holder.is_empty() {
                    "—"
                } else {
                    &data.holder
                }),
                escape_html(&data.holder_updated),
                labels.heirs
            ));
            for (name, relation, executor) in &data.heirs {
                body.push_str(&format!(
                    "<li>{} · {}{}</li>",
                    escape_html(name),
                    escape_html(relation),
                    if *executor { " · executor" } else { "" }
                ));
            }
            body.push_str(&format!("</ul><h2>{}</h2>", labels.assets));
            for asset in &data.assets {
                body.push_str(&format!(
                    "<div class=\"card\"><h3>{}</h3><p class=\"muted\">{} · {} · {}</p><p>{}</p>",
                    escape_html(&asset.name),
                    escape_html(&asset.kind),
                    escape_html(&asset.platform),
                    escape_html(&asset.value),
                    escape_html(&asset.note),
                ));
                if let Some(policy) = &asset.policy {
                    body.push_str(&policy_block(policy, &labels));
                    body.push_str("</div>");
                } else {
                    body.push_str(&format!(
                        "<p class=\"muted\">{}</p></div>",
                        labels.no_policy
                    ));
                }
            }
        }
        "checklist" => {
            body.push_str(&format!("<h2>{}</h2>", labels.documents));
            for asset in &data.assets {
                body.push_str(&format!(
                    "<div class=\"card\"><h3>☐ {}</h3><p class=\"muted\">{} · {}</p>",
                    escape_html(&asset.name),
                    escape_html(
                        asset
                            .policy
                            .as_ref()
                            .map(|p| p.name.as_str())
                            .unwrap_or(&asset.platform)
                    ),
                    escape_html(&asset.kind),
                ));
                if let Some(policy) = &asset.policy {
                    body.push_str(&format!(
                        "<p>{}</p><p><b>{}</b> {}</p></div>",
                        escape_html(&policy.required_documents),
                        labels.process,
                        escape_html(&policy.process_description)
                    ));
                } else {
                    body.push_str(&format!(
                        "<p class=\"muted\">{}</p></div>",
                        labels.no_policy
                    ));
                }
            }
        }
        "legal" => {
            let mut ids = BTreeSet::new();
            for asset in &data.assets {
                if let Some(policy) = &asset.policy {
                    for id in policy.legal_template_ids.split(',') {
                        if !id.trim().is_empty() {
                            ids.insert(id.trim().to_string());
                        }
                    }
                }
            }
            let templates: Vec<LegalTemplate> = serde_json::from_str(LEGAL_JSON)
                .map_err(|e| PlatformError::new("INTERNAL", format!("法律模板解析失败: {e}")))?;
            body.push_str(&format!("<h2>{}</h2>", labels.documents));
            for id in &ids {
                let template = templates.iter().find(|item| &item.id == id);
                match template {
                    Some(template) => {
                        body.push_str(&format!(
                            "<div class=\"card\"><h3>{}</h3><p class=\"muted\">{}</p><ol>",
                            escape_html(&template.title),
                            escape_html(&template.summary)
                        ));
                        for section in &template.sections {
                            body.push_str(&format!("<li>{}</li>", escape_html(section)));
                        }
                        body.push_str("</ol></div>");
                    }
                    None => body.push_str(&format!("<p class=\"mono\">{}</p>", escape_html(id))),
                }
            }
            body.push_str(
                "<p class=\"muted\">模板仅供参考，签署前需由执业律师按当地法律审阅。</p>",
            );
        }
        "toolkit" => {
            for asset in &data.assets {
                let Some(policy) = &asset.policy else {
                    continue;
                };
                body.push_str(&policy_block(policy, &labels));
                if let Some(url) = safe_url(&policy.inheritance_form_url) {
                    body.push_str(&format!(
                        "<p><b>form</b> <a href=\"{}\">{}</a></p>",
                        escape_html(url),
                        escape_html(url)
                    ));
                }
                if let Some(url) = safe_url(&policy.official_policy_url) {
                    body.push_str(&format!(
                        "<p><b>policy</b> <a href=\"{}\">{}</a></p>",
                        escape_html(url),
                        escape_html(url)
                    ));
                }
            }
        }
        _ => {
            return Err(PlatformError::invalid_args(format!(
                "未知交付物类型: {kind}"
            )));
        }
    }

    body.push_str(&format!(
        "<h2>{}</h2><p class=\"mono\">{}</p><p class=\"muted\">{}</p>",
        labels.generated,
        escape_html(&date),
        labels.provenance
    ));
    Ok((title, body))
}

fn safe_url(url: &str) -> Option<&str> {
    let value = url.trim();
    if value.starts_with("https://") || value.starts_with("http://") {
        Some(value)
    } else {
        None
    }
}

fn hash(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

pub fn enqueue(store: &Store, kind: &str, locale: &str) -> Result<String, PlatformError> {
    validate(kind, locale)?;
    let job_id = format!(
        "job_{}_{}_deliver_export",
        Utc::now().timestamp_millis(),
        JOB_COUNTER.fetch_add(1, Ordering::Relaxed)
    );
    let created = Utc::now().to_rfc3339();
    store
        .write(|conn| {
            conn.execute(
                "INSERT INTO jobs (id,kind,status,progress,payload_json,result_json,created_at,updated_at)
                 VALUES (?1,'deliver.export','queued',0,?2,'{}',?3,?3)",
                rusqlite::params![job_id, json!({ "kind": kind, "locale": locale }).to_string(), created],
            )?;
            Ok(())
        })
        .map_err(|e| PlatformError::new("INTERNAL", format!("交付任务入队失败: {e}")))?;
    Ok(job_id)
}

pub fn run_job(store: &Store, job_id: &str) -> Result<Value, PlatformError> {
    let payload = store
        .read(|conn| {
            conn.query_row(
                "SELECT payload_json FROM jobs WHERE id=?1 AND kind='deliver.export'",
                [job_id],
                |row| row.get::<_, String>(0),
            )
        })
        .map_err(|e| PlatformError::new("INTERNAL", format!("读取交付任务失败: {e}")))?;
    let payload: Value = serde_json::from_str(&payload).unwrap_or_else(|_| json!({}));
    let kind = payload.get("kind").and_then(Value::as_str).unwrap_or("");
    let locale = payload
        .get("locale")
        .and_then(Value::as_str)
        .unwrap_or("zh-CN");
    validate(kind, locale)?;

    let data = collect(store)?;
    let (title, body) = build_body(kind, locale, &data)?;
    let html = render_page(&title, &body, locale);
    let bytes = html.as_bytes();
    let now = Utc::now();
    let dir = export_dir(store);
    std::fs::create_dir_all(&dir)
        .map_err(|e| PlatformError::new("IO", format!("创建导出目录失败: {e}")))?;
    let file_name = format!("{job_id}.html");
    let path = dir.join(&file_name);
    let hash = hash(bytes);
    let generated_at = now.to_rfc3339();

    std::fs::write(&path, bytes)
        .map_err(|e| PlatformError::new("IO", format!("写入交付物失败: {e}")))?;
    store
        .write(|conn| {
            conn.execute(
                "INSERT INTO deliverable_exports (kind,locale,generated_at,template_version,file_hash,path)
                 VALUES (?1,?2,?3,'m2-html-v1',?4,?5)",
                rusqlite::params![kind, locale, generated_at, hash, path.to_string_lossy()],
            )?;
            Ok(())
        })
        .map_err(|e| PlatformError::new("INTERNAL", e.to_string()))?;

    Ok(json!({
        "jobId": job_id,
        "path": path.to_string_lossy(),
        "status": "succeeded",
        "progress": 1.0,
        "fileName": file_name,
        "downloadUrl": format!("/api/v1/exports/{job_id}"),
        "generatedAt": generated_at,
        "hash": hash,
    }))
}

pub fn export(store: &Store, kind: &str, locale: &str) -> Result<Value, PlatformError> {
    let job_id = enqueue(store, kind, locale)?;
    match run_job(store, &job_id) {
        Ok(result) => {
            crate::jobs::finish(store, &job_id, result.clone())?;
            Ok(result)
        }
        Err(error) => {
            let _ = crate::jobs::fail(store, &job_id, &error.code, &error.message);
            Err(error)
        }
    }
}

fn validate(kind: &str, locale: &str) -> Result<(), PlatformError> {
    if !matches!(kind, "report" | "checklist" | "legal" | "toolkit") {
        return Err(PlatformError::invalid_args(format!(
            "未知交付物类型: {kind}"
        )));
    }
    if !matches!(locale, "zh-CN" | "en-US") {
        return Err(PlatformError::invalid_args(format!("未知语言: {locale}")));
    }
    Ok(())
}

pub fn read_export(store: &Store, job_id: &str) -> Result<(String, Vec<u8>), PlatformError> {
    let result = store
        .read(|conn| {
            conn.query_row(
                "SELECT status, result_json FROM jobs WHERE id=?1",
                [job_id],
                |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
            )
            .optional()
        })
        .map_err(|e| PlatformError::new("INTERNAL", e.to_string()))?
        .ok_or_else(|| PlatformError::not_found("交付物任务"))?;
    if result.0 != "succeeded" {
        return Err(PlatformError::new("CONFLICT", "交付物尚未完成"));
    }
    let payload: Value = serde_json::from_str(&result.1)
        .map_err(|e| PlatformError::new("INTERNAL", e.to_string()))?;
    let path = payload["path"]
        .as_str()
        .ok_or_else(|| PlatformError::new("INTERNAL", "任务缺少文件路径"))?;
    let file_name = payload["fileName"]
        .as_str()
        .unwrap_or("deliverable.html")
        .to_string();
    let root = export_dir(store)
        .canonicalize()
        .map_err(|e| PlatformError::new("IO", format!("导出目录不可读: {e}")))?;
    let canonical = Path::new(path)
        .canonicalize()
        .map_err(|e| PlatformError::new("IO", format!("交付物文件不可读: {e}")))?;
    if !canonical.starts_with(&root) {
        return Err(PlatformError::new("FORBIDDEN", "交付物路径越界"));
    }
    let bytes = std::fs::read(canonical)
        .map_err(|e| PlatformError::new("IO", format!("读取交付物失败: {e}")))?;
    Ok((file_name, bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use digilegacy_domains::policy;
    use std::sync::Arc;

    fn temp_store(tag: &str) -> Store {
        let path = std::env::temp_dir().join(format!(
            "dgl_deliver_{tag}_{}_{}.db",
            std::process::id(),
            Utc::now().timestamp_micros()
        ));
        Store::open(path, None).unwrap()
    }

    #[test]
    fn async_export_runs_through_job_queue() {
        let store = temp_store("async");
        policy::seed_if_empty(&store).unwrap();
        let app = crate::Application::new(Arc::new(store));
        let job_id = enqueue(app.store(), "report", "zh-CN").unwrap();
        assert_eq!(
            crate::jobs::get(app.store(), &job_id).unwrap().status,
            "queued"
        );
        assert!(crate::jobs::run_one(&app).unwrap());
        let detail = crate::jobs::get(app.store(), &job_id).unwrap();
        assert_eq!(detail.status, "succeeded");
        assert!(detail.result["fileName"]
            .as_str()
            .unwrap()
            .ends_with(".html"));
        let (_, bytes) = read_export(app.store(), &job_id).unwrap();
        assert!(bytes.starts_with(b"<!doctype html>"));
    }

    #[test]
    fn exports_and_downloads_real_html() {
        let store = temp_store("report");
        policy::seed_if_empty(&store).unwrap();
        store
            .write(|conn| {
                conn.execute(
                    "INSERT OR IGNORE INTO platforms (id,name,category) VALUES (0,'未分类','unknown')",
                    [],
                )?;
                conn.execute(
                    "INSERT OR IGNORE INTO platforms (id,name,category) VALUES (1,'支付宝','支付')",
                    [],
                )?;
                conn.execute(
                    "INSERT INTO digital_assets (platform_id,category,created_at,updated_at) VALUES (1,'account','now','now')",
                    [],
                )?;
                Ok(())
            })
            .unwrap();
        let job = export(&store, "report", "zh-CN").unwrap();
        let id = job["jobId"].as_str().unwrap().to_string();
        assert_eq!(job["status"], "succeeded");
        let (file_name, bytes) = read_export(&store, &id).unwrap();
        assert!(file_name.ends_with(".html"));
        assert!(bytes.starts_with(b"<!doctype html>"));
        let html = String::from_utf8(bytes).unwrap();
        assert_eq!(
            html.matches("<div class=\"card\">").count(),
            html.matches("</div>").count()
        );
        assert!(html.contains("支付宝"));
        assert!(html.contains("所需材料"));

        let legal = export(&store, "legal", "zh-CN").unwrap();
        let (_, legal_bytes) = read_export(&store, legal["jobId"].as_str().unwrap()).unwrap();
        let legal_html = String::from_utf8(legal_bytes).unwrap();
        assert!(legal_html.contains("数字遗产继承声明"));
        assert!(legal_html.contains("执业律师"));
        let _ = std::fs::remove_file(store.path());
    }
}
