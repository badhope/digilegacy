//! DigiLegacy Web 服务：把同一套核心命令暴露为 HTTP API，并托管前端。
//!
//! 定位（重要，不要对外说成云端服务）：
//! - **单机自托管**。默认只监听 `127.0.0.1`，数据仍在本地 SQLite 文件里。
//! - 默认**只读**。写操作需要显式 `--allow-write`，因为这个进程常被当作
//!   「在另一台设备上查看家里这台电脑的规划」的窗口，误写代价远高于误读。
//! - 无账号体系。绑定到非回环地址时必须同时给 `--token`，否则拒绝启动——
//!   一个能读全部遗产规划且无鉴权的端口是安全事故，不是便利功能。
//!
//! 端点：
//!   GET  /api/health              运行状态与库统计
//!   GET  /api/commands            可调用命令清单
//!   POST /api/invoke              统一调用入口 {cmd, args}
//!   GET  /api/dashboard           仪表板统计（便捷别名）
//!   GET  /api/assets              资产列表
//!   GET  /api/assets/:id          单个资产
//!   GET  /api/platforms           平台库
//!   GET  /api/heirs               继承人
//!   GET  /api/dms                 死手开关状态
//!   GET  /api/report?locale=zh-CN 继承报告 HTML
//!   GET  /api/guide?locale=zh-CN  家人执行清单 HTML
//!   GET  /api/review/profiles     审核场景清单
//!   *                              静态前端（`--web-dir`，默认 ../dist）

use std::net::{IpAddr, SocketAddr};
use std::path::PathBuf;
use std::sync::Arc;

use axum::{
    extract::{rejection::JsonRejection, Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{Html, IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use serde_json::{json, Map, Value};
use tower_http::cors::{Any, CorsLayer};

use digilegacy_lib::dispatch;

/// 服务运行配置。
pub struct Config {
    pub data_dir: Option<PathBuf>,
    /// 前端静态目录；`None` 表示只提供 API。
    pub web_dir: Option<PathBuf>,
    pub bind: String,
    pub port: u16,
    /// 允许写操作（默认只读）。
    pub allow_write: bool,
    /// Bearer token；绑定非回环地址时必填。
    pub token: Option<String>,
}

#[derive(Clone)]
struct AppState {
    allow_write: bool,
    token: Option<String>,
}

/// 写命令白名单：即使 `--allow-write`，也只放行这些。
///
/// 白名单而不是黑名单：新增命令默认不可通过 HTTP 写入，需要显式加进来。
const WRITE_COMMANDS: &[&str] = &[
    "add_asset",
    "update_asset",
    "delete_asset",
    "set_disposal_wish",
    "set_heirs",
    "set_user_profile",
    "set_custody_hints",
    "update_check_in",
    "set_dead_mans_switch",
    "set_dms_pause",
    "record_planning_review",
    "import_platform_patch",
    "clear_platform_patch",
    "import_backup",
    "create_checkpoint",
    "delete_checkpoint",
    "restore_checkpoint",
    "set_asset_credential",
];

/// 永远不允许从 HTTP 触发的命令：本机设备绑定、密钥生命周期、外部网络出站。
///
/// 这几类操作的共同点——**一次误调用会造成不可逆后果，或把密钥送进网络**。
/// 桌面端和 CLI 仍然可以调用它们；HTTP 入口是唯一被收窄的通道。
const DENIED_COMMANDS: &[&str] = &[
    "enable_db_encryption",
    "disable_db_encryption",
    "change_master_password",
    "set_master_password",
    "unlock",
    "start_recovery",
    "complete_recovery",
    "unlock_with_recovery_shares",
    "generate_heir_recovery_kit",
    "clear_heir_recovery",
    "export_kdbx_archive",
    "send_test_email",
    "send_heartbeat_now",
    "set_smtp_config",
    "set_imap_config",
    "poll_imap_checkin",
    "set_heartbeat_config",
    "set_assistant_config",
    "ask_assistant",
    "run_agent_task",
    "run_ai_review",
];

pub fn router(cfg: Config) -> Result<Router, String> {
    let state = Arc::new(AppState {
        allow_write: cfg.allow_write,
        token: cfg.token.clone(),
    });

    let api = Router::new()
        .route("/health", get(health))
        .route("/commands", get(commands))
        .route("/invoke", post(invoke))
        .route("/dashboard", get(dashboard))
        .route("/assets", get(list_assets))
        .route("/assets/{id}", get(get_asset))
        .route("/platforms", get(list_platforms))
        .route("/heirs", get(list_heirs))
        .route("/dms", get(dms_status))
        .route("/report", get(report))
        .route("/guide", get(guide))
        .route("/review/profiles", get(review_profiles))
        .with_state(state);

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods([axum::http::Method::GET, axum::http::Method::POST])
        .allow_headers(Any);

    let mut app = Router::new().nest("/api", api).layer(cors);

    if let Some(dir) = cfg.web_dir.clone() {
        let index_path = dir.join("index.html");
        if !index_path.is_file() {
            return Err(format!(
                "{} 下没有 index.html：先运行 npm run build:web",
                dir.display()
            ));
        }
        // 静态资源 + SPA 回退由同一个处理器负责。
        //
        // 三个坑都踩过，这里一次写清：
        // 1. 不能用 `Router::fallback_service(...)` 再接 `.fallback(...)`——后者会
        //    整个覆盖前者，结果是所有 JS/CSS 都被当成 HTML 返回（content-type
        //    text/html），浏览器直接白屏，控制台只有一句 MIME 类型错误。
        // 2. 不能用 `ServeDir::not_found_service(ServeFile::new(index))`——未匹配
        //    路径的状态码是 404，刷新 /dashboard 会被浏览器与 CDN 当成失败页。
        // 3. 必须显式给 content-type：`tower-http` 的 mime 探测覆盖不到所有
        //    Vite 产物（.mjs/.wasm 尤其容易漏），缺类型同样白屏。
        let web_root = Arc::new(WebRoot {
            dir: dir.clone(),
            index: index_path,
        });
        app = app.merge(
            Router::new()
                .fallback(axum::routing::get(static_or_index))
                .with_state(web_root),
        );
    }

    Ok(app)
}

/// 静态前端根目录。
#[derive(Clone)]
struct WebRoot {
    dir: PathBuf,
    index: PathBuf,
}

/// 按扩展名给出 content-type。
///
/// 只覆盖 Vite 产物实际会出现的类型；其余走 `application/octet-stream`，
/// 浏览器会当下载而不是静默当 HTML 执行——错得明显比错得隐蔽好。
fn content_type_for(path: &std::path::Path) -> &'static str {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .as_deref()
    {
        Some("html") => "text/html; charset=utf-8",
        Some("js") | Some("mjs") => "text/javascript; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("json") => "application/json; charset=utf-8",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("webp") => "image/webp",
        Some("gif") => "image/gif",
        Some("ico") => "image/x-icon",
        Some("woff") => "font/woff",
        Some("woff2") => "font/woff2",
        Some("ttf") => "font/ttf",
        Some("otf") => "font/otf",
        Some("wasm") => "application/wasm",
        Some("map") => "application/json; charset=utf-8",
        Some("txt") => "text/plain; charset=utf-8",
        Some("webmanifest") => "application/manifest+json",
        Some("mp4") => "video/mp4",
        Some("webm") => "video/webm",
        _ => "application/octet-stream",
    }
}

/// 请求路径 → 安全地映射到磁盘路径。
///
/// 拒绝一切能逃出根目录的路径（`..`、绝对路径、Windows 盘符、UNC）。
/// 静态服务是最容易被拿来读 `C:\Windows\win.ini` 的入口，白名单比过滤器可靠：
/// 这里只接受「解析后仍以 root 开头」的结果。
fn resolve_inside(root: &std::path::Path, raw: &str) -> Option<PathBuf> {
    let decoded = percent_decode(raw)?;
    if decoded.contains('\u{0}') {
        return None;
    }
    let rel = decoded.trim_start_matches('/');
    if rel.is_empty() {
        return None;
    }
    // 逐段校验：任何一段是 .. 或盘符样式就直接拒绝，不做「先拼再判」的赌博。
    for seg in rel.split(['/', '\\']) {
        if seg.is_empty() || seg == "." {
            continue;
        }
        if seg == ".." || seg.contains(':') {
            return None;
        }
    }
    let candidate = root.join(rel);
    let canon_root = root.canonicalize().ok()?;
    let canon = candidate.canonicalize().ok()?;
    canon.starts_with(&canon_root).then_some(canon)
}

/// 最小 percent-decode：只处理 `%XX`，其余原样保留。
fn percent_decode(raw: &str) -> Option<String> {
    let bytes = raw.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            if i + 2 >= bytes.len() {
                return None;
            }
            let hi = (bytes[i + 1] as char).to_digit(16)?;
            let lo = (bytes[i + 2] as char).to_digit(16)?;
            out.push((hi * 16 + lo) as u8);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}

/// 静态文件优先；文件不存在且像前端路由（无扩展名）时回退 index.html。
///
/// 带扩展名却找不到 → 真的 404。把 `/assets/missing.js` 也回退成 HTML 会让
/// 浏览器把 HTML 当模块解析，报错信息完全指错方向。
async fn static_or_index(State(root): State<Arc<WebRoot>>, uri: axum::http::Uri) -> Response {
    let path = uri.path();

    if let Some(file) = resolve_inside(&root.dir, path) {
        if file.is_file() {
            return match std::fs::read(&file) {
                Ok(bytes) => {
                    let mime = content_type_for(&file);
                    // 带内容哈希的产物可长期缓存；index.html 必须每次校验，
                    // 否则用户升级后仍会拿到旧页面去请求已被删除的旧 chunk。
                    let cache = if path.starts_with("/assets/") {
                        "public, max-age=31536000, immutable"
                    } else {
                        "no-cache"
                    };
                    (
                        StatusCode::OK,
                        [
                            (axum::http::header::CONTENT_TYPE, mime),
                            (axum::http::header::CACHE_CONTROL, cache),
                        ],
                        bytes,
                    )
                        .into_response()
                }
                Err(_) => err_response(StatusCode::INTERNAL_SERVER_ERROR, "读取静态文件失败"),
            };
        }
    }

    let looks_like_asset = path
        .rsplit('/')
        .next()
        .map(|name| name.contains('.'))
        .unwrap_or(false);
    if looks_like_asset {
        return err_response(StatusCode::NOT_FOUND, "静态资源不存在");
    }

    match std::fs::read(&root.index) {
        Ok(bytes) => (
            StatusCode::OK,
            [
                (axum::http::header::CONTENT_TYPE, "text/html; charset=utf-8"),
                (axum::http::header::CACHE_CONTROL, "no-cache"),
            ],
            bytes,
        )
            .into_response(),
        Err(_) => err_response(StatusCode::INTERNAL_SERVER_ERROR, "读取 index.html 失败"),
    }
}

/// 启动服务（阻塞直到进程退出）。
pub fn serve(cfg: Config) -> Result<(), String> {
    let addr: SocketAddr = format!("{}:{}", cfg.bind, cfg.port)
        .parse()
        .map_err(|e| format!("监听地址不合法: {}", e))?;

    let ip: IpAddr = addr.ip();
    if !ip.is_loopback() && cfg.token.as_deref().unwrap_or("").is_empty() {
        return Err(
            "拒绝在非回环地址上无鉴权启动：请加 --token <随机串>，或改回 127.0.0.1".to_string(),
        );
    }

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|e| format!("创建运行时失败: {}", e))?;

    let app = router(cfg)?;
    rt.block_on(async move {
        let listener = tokio::net::TcpListener::bind(addr)
            .await
            .map_err(|e| format!("绑定 {} 失败: {}", addr, e))?;
        eprintln!("DigiLegacy web listening on http://{}", addr);
        axum::serve(listener, app)
            .await
            .map_err(|e| format!("服务异常退出: {}", e))
    })
}

// ============ 处理函数 ============

fn authorized(state: &AppState, headers: &HeaderMap) -> bool {
    let Some(expected) = state.token.as_deref() else {
        // 未配置 token：只可能绑定在回环地址（启动时已校验）。
        return true;
    };
    if expected.is_empty() {
        return true;
    }
    let Some(raw) = headers.get(axum::http::header::AUTHORIZATION) else {
        return false;
    };
    let Ok(text) = raw.to_str() else {
        return false;
    };
    text.strip_prefix("Bearer ")
        .map(|t| constant_time_eq(t.as_bytes(), expected.as_bytes()))
        .unwrap_or(false)
}

/// 定长比较：避免 token 比较泄漏长度/前缀信息。
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b) {
        diff |= x ^ y;
    }
    diff == 0
}

fn err_response(status: StatusCode, msg: &str) -> Response {
    (status, Json(json!({ "success": false, "message": msg }))).into_response()
}

fn ok_response(v: Value) -> Response {
    Json(v).into_response()
}

async fn health(State(state): State<Arc<AppState>>) -> Response {
    let dir = match digilegacy_lib::paths::data_dir() {
        Ok(d) => d,
        Err(e) => return err_response(StatusCode::INTERNAL_SERVER_ERROR, &e),
    };
    let report = digilegacy_lib::bootstrap::health(&dir);
    let write_enabled = state.allow_write;
    let mut v = serde_json::to_value(&report).unwrap_or(Value::Null);
    if let Some(obj) = v.as_object_mut() {
        obj.insert("write_enabled".into(), json!(write_enabled));
    }
    ok_response(v)
}

async fn commands() -> Response {
    ok_response(json!({
        "success": true,
        "data": dispatch::command_names(),
        "read_only": WRITE_COMMANDS,
        "denied": DENIED_COMMANDS,
    }))
}

#[derive(Deserialize)]
struct InvokeBody {
    cmd: String,
    #[serde(default)]
    args: Map<String, Value>,
}

/// 统一调用入口。
///
/// 状态码语义（前端 `invoke.ts` 依赖，改动前先看它）：
/// - **命令执行完了**（无论业务成功还是业务失败）→ 200 + `{success, message, data}` 信封。
///   业务失败不是 HTTP 错误：`success:false` 是后端明确回话，store 要读它的 `message`
///   展示原因。早先这里把业务失败映射成 400，前端按“非 2xx 即抛错”处理，于是
///   用户看到的是笼统的加载失败，后端那句“继承人姓名不能为空”被丢掉。
/// - **传输/授权层拒绝**（无 token、命令被禁、只读模式、未知命令、工作线程崩溃）
///   才是 4xx/5xx：这些是调用方式错了，不是业务结果。
///
/// 请求体不合法（缺 `cmd`、非法 JSON）同样返回同一套信封而不是 axum 默认的纯文本
/// 422——否则前端 `res.json()` 直接抛错，用户看到的是一句“非 JSON 响应”。
async fn invoke(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    body: Result<Json<InvokeBody>, JsonRejection>,
) -> Response {
    if !authorized(&state, &headers) {
        return err_response(StatusCode::UNAUTHORIZED, "缺少或错误的 Bearer token");
    }

    let Json(body) = match body {
        Ok(b) => b,
        Err(rejection) => {
            return err_response(
                StatusCode::BAD_REQUEST,
                &format!(
                    "请求体不合法：{}（应为 {{\"cmd\": \"命令名\", \"args\": {{...}}}}）",
                    rejection.body_text()
                ),
            );
        }
    };

    if DENIED_COMMANDS.contains(&body.cmd.as_str()) {
        return err_response(
            StatusCode::FORBIDDEN,
            &format!(
                "命令 `{}` 不允许通过 HTTP 触发（密钥与出站操作仅限本机）",
                body.cmd
            ),
        );
    }
    let is_write = WRITE_COMMANDS.contains(&body.cmd.as_str());
    if is_write && !state.allow_write {
        return err_response(
            StatusCode::FORBIDDEN,
            "服务以只读模式运行；写操作需要启动时加 --allow-write",
        );
    }

    match tokio::task::spawn_blocking(move || dispatch::call(&body.cmd, body.args)).await {
        Ok(Ok(v)) => {
            // 命令执行完就回 200：success 由信封表达，不再混进状态码。
            (StatusCode::OK, Json(v)).into_response()
        }
        Ok(Err(e)) => {
            let status = if e.contains(dispatch::ERR_UNSUPPORTED) {
                StatusCode::NOT_IMPLEMENTED
            } else if e.contains(dispatch::ERR_INVALID_ARGS) {
                // 调用方给错了参数：4xx。归到 500 会让监控把用户漏填字段算成服务故障，
                // 用户也会以为「服务器坏了」而不是「我这栏没填」。
                StatusCode::BAD_REQUEST
            } else if e.contains("未知命令") {
                StatusCode::NOT_FOUND
            } else {
                StatusCode::INTERNAL_SERVER_ERROR
            };
            // INVALID_ARGS 前缀是给机器分类用的，给人看时要去掉。
            let msg = e.replace(&format!("{}: ", dispatch::ERR_INVALID_ARGS), "");
            err_response(status, &msg)
        }
        Err(e) => err_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("工作线程异常: {}", e),
        ),
    }
}

async fn dashboard() -> Response {
    match digilegacy_lib::commands_read::dashboard() {
        Ok(v) => ok_response(json!({ "success": true, "data": v })),
        Err(e) => err_response(StatusCode::INTERNAL_SERVER_ERROR, &e),
    }
}

async fn list_assets() -> Response {
    match digilegacy_lib::commands_read::assets() {
        Ok(v) => ok_response(json!({ "success": true, "data": v })),
        Err(e) => err_response(StatusCode::INTERNAL_SERVER_ERROR, &e),
    }
}

async fn get_asset(Path(id): Path<i64>) -> Response {
    match digilegacy_lib::commands_read::asset(id) {
        Ok(v) => ok_response(json!({ "success": true, "data": v })),
        Err(e) => err_response(StatusCode::INTERNAL_SERVER_ERROR, &e),
    }
}

async fn list_platforms() -> Response {
    match digilegacy_lib::commands_read::platforms() {
        Ok(v) => ok_response(json!({ "success": true, "data": v })),
        Err(e) => err_response(StatusCode::INTERNAL_SERVER_ERROR, &e),
    }
}

async fn list_heirs() -> Response {
    match digilegacy_lib::commands_read::heirs() {
        Ok(v) => ok_response(json!({ "success": true, "data": v })),
        Err(e) => err_response(StatusCode::INTERNAL_SERVER_ERROR, &e),
    }
}

async fn dms_status() -> Response {
    match digilegacy_lib::commands_read::dms_status() {
        Ok(v) => ok_response(json!({ "success": true, "data": v })),
        Err(e) => err_response(StatusCode::INTERNAL_SERVER_ERROR, &e),
    }
}

async fn review_profiles() -> Response {
    match digilegacy_lib::commands_read::review_profiles() {
        Ok(v) => ok_response(json!({ "success": true, "data": v })),
        Err(e) => err_response(StatusCode::INTERNAL_SERVER_ERROR, &e),
    }
}

#[derive(Deserialize)]
struct LocaleQuery {
    locale: Option<String>,
}

async fn report(Query(q): Query<LocaleQuery>) -> Response {
    match digilegacy_lib::commands_read::report_html(q.locale.as_deref().unwrap_or("zh-CN")) {
        Ok(html) => Html(html).into_response(),
        Err(e) => err_response(StatusCode::INTERNAL_SERVER_ERROR, &e),
    }
}

async fn guide(Query(q): Query<LocaleQuery>) -> Response {
    match digilegacy_lib::commands_read::guide_html(q.locale.as_deref().unwrap_or("zh-CN")) {
        Ok(html) => Html(html).into_response(),
        Err(e) => err_response(StatusCode::INTERNAL_SERVER_ERROR, &e),
    }
}

// ============ 入口 ============

fn main() {
    let mut args = std::env::args().skip(1);
    let mut cfg = Config {
        data_dir: None,
        web_dir: None,
        bind: "127.0.0.1".to_string(),
        port: 8787,
        allow_write: false,
        token: std::env::var("DIGILEGACY_API_TOKEN")
            .ok()
            .filter(|s| !s.is_empty()),
    };

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--data-dir" => cfg.data_dir = args.next().map(PathBuf::from),
            "--web-dir" => cfg.web_dir = args.next().map(PathBuf::from),
            "--bind" => {
                if let Some(v) = args.next() {
                    cfg.bind = v;
                }
            }
            "--port" => {
                cfg.port = args.next().and_then(|v| v.parse().ok()).unwrap_or(cfg.port);
            }
            "--allow-write" => cfg.allow_write = true,
            "--token" => cfg.token = args.next(),
            "--help" | "-h" => {
                println!(
                    "DigiLegacy web service\n\n\
                     Usage: digilegacy-server [options]\n\n\
                     Options:\n\
                       --data-dir <DIR>   Database directory (default: platform app-data dir)\n\
                       --web-dir <DIR>    Serve the built frontend from DIR\n\
                       --bind <ADDR>      Bind address (default 127.0.0.1)\n\
                       --port <PORT>      Port (default 8787)\n\
                       --allow-write      Permit the write-command whitelist (default: read-only)\n\
                       --token <TOKEN>    Require `Authorization: Bearer <TOKEN>` (mandatory off-loopback)\n\n\
                     Environment: DIGILEGACY_DATA_DIR, DIGILEGACY_API_TOKEN"
                );
                return;
            }
            "--version" | "-V" => {
                println!("{}", env!("CARGO_PKG_VERSION"));
                return;
            }
            other => {
                eprintln!("unknown option: {}", other);
                std::process::exit(2);
            }
        }
    }

    let dir = match digilegacy_lib::bootstrap::open(cfg.data_dir.clone()) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("failed to open database: {}", e);
            std::process::exit(1);
        }
    };
    eprintln!("data directory: {}", dir.display());

    if let Err(e) = serve(cfg) {
        eprintln!("{}", e);
        std::process::exit(1);
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    /// 把 Response 的 body 读成 JSON，供信封断言使用。
    ///
    /// 只断言状态码不够：真正的契约是「无论成功失败，body 都是同一个信封」。
    async fn body_json(resp: Response) -> Value {
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .expect("body 可读");
        serde_json::from_slice(&bytes).expect("body 是 JSON")
    }

    #[tokio::test]
    async fn err_response_is_always_a_full_envelope() {
        // 前端 invoke.ts 只在「有 success 字段」时按信封处理。少了 success，
        // 403/404 会被降级成「请求失败（HTTP xxx）」，后端写的原因就丢了。
        let resp = err_response(StatusCode::FORBIDDEN, "只读模式");
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
        let v = body_json(resp).await;
        assert_eq!(v["success"], json!(false));
        assert_eq!(v["message"], json!("只读模式"));
    }

    #[test]
    fn write_and_denied_lists_do_not_overlap() {
        for cmd in WRITE_COMMANDS {
            assert!(
                !DENIED_COMMANDS.contains(cmd),
                "`{}` 同时出现在写白名单与禁止清单里，语义矛盾",
                cmd
            );
        }
    }

    /// 参数错误必须是 400 且不带机器前缀，否则监控会把用户漏填字段算进服务故障。
    #[tokio::test]
    async fn invalid_args_maps_to_400_and_strips_prefix() {
        // 走真实的 dispatch：add_asset 缺 req 字段就是典型的调用方输入错误。
        let err = dispatch::call("add_asset", Map::new()).expect_err("缺参数必须报错");
        assert!(
            err.contains(dispatch::ERR_INVALID_ARGS),
            "缺参数必须带 INVALID_ARGS 前缀，实际: {err}"
        );

        // 服务端把它映射成 400，并把前缀从给人看的文案里去掉。
        let msg = err.replace(&format!("{}: ", dispatch::ERR_INVALID_ARGS), "");
        let resp = err_response(StatusCode::BAD_REQUEST, &msg);
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
        let v = body_json(resp).await;
        assert_eq!(v["success"], json!(false));
        assert!(
            !v["message"].as_str().unwrap().contains("INVALID_ARGS"),
            "机器前缀不该出现在用户可见文案里"
        );
    }

    #[test]
    fn every_write_command_is_a_real_command() {
        let known = dispatch::command_names();
        for cmd in WRITE_COMMANDS {
            assert!(
                known.contains(cmd),
                "写白名单包含未知命令 `{}`，拼写错误会让该操作静默不可用",
                cmd
            );
        }
    }

    #[test]
    fn every_denied_command_is_a_real_command() {
        let known = dispatch::command_names();
        for cmd in DENIED_COMMANDS {
            assert!(known.contains(cmd), "禁止清单包含未知命令 `{}`", cmd);
        }
    }

    #[test]
    fn constant_time_eq_behaves_like_eq() {
        assert!(constant_time_eq(b"secret", b"secret"));
        assert!(!constant_time_eq(b"secret", b"secreT"));
        assert!(!constant_time_eq(b"secret", b"secret-longer"));
        assert!(!constant_time_eq(b"", b"x"));
    }

    #[test]
    fn token_guard_requires_bearer_prefix() {
        let state = AppState {
            allow_write: false,
            token: Some("abc".into()),
        };
        let mut h = HeaderMap::new();
        assert!(!authorized(&state, &h), "无 header 必须拒绝");
        h.insert(axum::http::header::AUTHORIZATION, "abc".parse().unwrap());
        assert!(!authorized(&state, &h), "缺少 Bearer 前缀必须拒绝");
        h.insert(
            axum::http::header::AUTHORIZATION,
            "Bearer abc".parse().unwrap(),
        );
        assert!(authorized(&state, &h));
    }

    #[test]
    fn no_token_means_loopback_only_but_authorized() {
        let state = AppState {
            allow_write: false,
            token: None,
        };
        assert!(authorized(&state, &HeaderMap::new()));
    }
}
