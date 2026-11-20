//! Local GGUF model catalog, resumable download and runtime activation.
//!
//! The catalog is intentionally small and curated: the default is
//! MiniCPM5-1B (1.08B, 128K context, standard Llama architecture), with two
//! optional entries that are clearly labelled when they are not strict <2B or
//! need a specialised runtime.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io::Read;
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

const DEFAULT_PORT: u16 = 11435;
const MAX_PORT_TRIES: u16 = 5;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelSpec {
    pub id: &'static str,
    pub name: &'static str,
    pub params: &'static str,
    pub context: u64,
    pub size_bytes: u64,
    pub sha256: &'static str,
    pub license: &'static str,
    pub description: &'static str,
    pub recommended: bool,
    pub strict_under_2b: bool,
    pub runtime: &'static str,
    pub file_name: &'static str,
    pub download_url: &'static str,
    pub model_id: &'static str,
}

pub const CATALOG: &[ModelSpec] = &[
    ModelSpec {
        id: "minicpm5-1b",
        name: "MiniCPM5-1B",
        params: "1.08B 稠密",
        context: 131_072,
        size_bytes: 688_065_920,
        sha256: "81b64d05a23b17b34c475f42b3e72fbde62d4b92cc34541f7a8031d0752deafa",
        license: "Apache-2.0",
        description: "1B 级开源 SOTA，128K 上下文，标准 Llama 架构；中文规划、工具选择和日常生成均衡，默认推荐。",
        recommended: true,
        strict_under_2b: true,
        runtime: "llama.cpp",
        file_name: "MiniCPM5-1B-Q4_K_M.gguf",
        download_url: "https://modelscope.cn/models/OpenBMB/MiniCPM5-1B-GGUF/resolve/master/MiniCPM5-1B-Q4_K_M.gguf",
        model_id: "MiniCPM5-1B-Q4_K_M",
    },
    ModelSpec {
        id: "minicpm5-2b",
        name: "MiniCPM5-2B",
        params: "2.52B 稠密",
        context: 131_072,
        size_bytes: 1_561_318_368,
        sha256: "ec2d5801640099e97d8d7e8003ad4d81f336e757811f03a26173dddf386602fd",
        license: "Apache-2.0",
        description: "2B 级质量档，代码、数学、长文本和工具调用更强；实际参数 2.52B，不属于严格 <2B。",
        recommended: false,
        strict_under_2b: false,
        runtime: "llama.cpp",
        file_name: "MiniCPM5-2B-Q4_K_M.gguf",
        download_url: "https://modelscope.cn/models/OpenBMB/MiniCPM5-2B-GGUF/resolve/master/MiniCPM5-2B-Q4_K_M.gguf",
        model_id: "MiniCPM5-2B-Q4_K_M",
    },
    ModelSpec {
        id: "spark-x2.5-1.7b",
        name: "Spark-X2.5-1.7B",
        params: "1.71B 稠密",
        context: 1_048_576,
        size_bytes: 1_107_457_856,
        sha256: "902bde2522394954ac17821b3e5fd0df02defbc6944f122253f2580acf0503f4",
        license: "Apache-2.0",
        description: "严格 <2B 里上下文最长的候选，原生 1M、200+ 语言；使用 Spark 专用混合注意力，标准 llama.cpp 兼容性待验证。",
        recommended: false,
        strict_under_2b: true,
        runtime: "experimental",
        file_name: "Spark-X2.5-1.7B-Q4_K_M.gguf",
        download_url: "https://modelscope.cn/models/XHToken/Spark-X2.5-1.7B-GGUF/resolve/master/Spark-X2.5-1.7B-Q4_K_M.gguf",
        model_id: "Spark-X2.5-1.7B-Q4_K_M",
    },
];

/// A local-model failure that carries the platform error code the HTTP layer
/// should return. Keeping the mapping here avoids turning every failure into a
/// 502 provider error.
#[derive(Debug, Clone)]
pub struct LocalModelError {
    pub code: &'static str,
    pub message: String,
}

impl LocalModelError {
    fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

#[derive(Debug, Default, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct DownloadState {
    status: String,
    downloaded: u64,
    total: u64,
    error: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ActiveConfig {
    id: String,
    model: String,
    base_url: String,
    api_key: String,
    path: String,
    #[serde(default)]
    pid: Option<u32>,
}

fn active_port(config: &ActiveConfig) -> Option<u16> {
    config
        .base_url
        .trim_end_matches('/')
        .trim_end_matches("/v1")
        .rsplit(':')
        .next()
        .and_then(|value| value.parse::<u16>().ok())
}

fn terminate_pid(pid: u32) {
    let command = fs::read_to_string(format!("/proc/{pid}/cmdline")).unwrap_or_default();
    if command.contains("local_model_server.py") {
        let _ = Command::new("kill")
            .arg("-TERM")
            .arg(pid.to_string())
            .status();
    }
}

struct RuntimeHandle {
    child: Child,
    port: u16,
    id: String,
}

fn hash_file(path: &Path) -> Result<String, String> {
    let mut file = File::open(path).map_err(|e| format!("读取模型文件失败: {e}"))?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; 1024 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|e| format!("读取模型文件失败: {e}"))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hasher
        .finalize()
        .iter()
        .map(|byte| format!("{:02x}", byte))
        .collect())
}

pub struct Manager {
    root: PathBuf,
    downloads: Mutex<HashMap<String, DownloadState>>,
    runtime: Mutex<Option<RuntimeHandle>>,
}

impl Manager {
    fn new(root: PathBuf) -> Self {
        let _ = fs::create_dir_all(&root);
        Self {
            root,
            downloads: Mutex::new(HashMap::new()),
            runtime: Mutex::new(None),
        }
    }

    pub fn global() -> &'static Self {
        static MANAGER: OnceLock<Manager> = OnceLock::new();
        MANAGER.get_or_init(|| {
            let root = std::env::var("DIGILEGACY_MODELS_DIR")
                .ok()
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("data/models"));
            Manager::new(root)
        })
    }

    fn model_dir(&self, spec: &ModelSpec) -> PathBuf {
        self.root.join(spec.id)
    }

    fn model_path(&self, spec: &ModelSpec) -> PathBuf {
        self.model_dir(spec).join(spec.file_name)
    }

    fn marker_path(&self, spec: &ModelSpec) -> PathBuf {
        self.model_path(spec).with_extension("gguf.sha256")
    }

    fn active_path(&self) -> PathBuf {
        self.root.join("active.json")
    }

    fn read_active(&self) -> Option<ActiveConfig> {
        let text = fs::read_to_string(self.active_path()).ok()?;
        serde_json::from_str(&text).ok()
    }

    fn is_verified(&self, spec: &ModelSpec) -> bool {
        let marker = self.marker_path(spec);
        let model = self.model_path(spec);
        if !marker.exists() || !model.exists() {
            return false;
        }
        let size_matches = fs::metadata(&model)
            .map(|metadata| metadata.len() == spec.size_bytes)
            .unwrap_or(false);
        size_matches
            && fs::read_to_string(marker)
                .map(|value| value.trim() == spec.sha256)
                .unwrap_or(false)
    }

    pub fn status(&self) -> serde_json::Value {
        let active = self.read_active();
        let runtime = self.runtime.lock().ok().and_then(|mut guard| {
            guard
                .as_mut()
                .and_then(|handle| match handle.child.try_wait() {
                    Ok(None) => Some((handle.id.clone(), handle.port)),
                    _ => None,
                })
        });
        let runtime =
            runtime.or_else(|| active.as_ref().and_then(|value| self.probe_active(value)));
        let downloads = self.downloads.lock().ok();
        let items: Vec<serde_json::Value> = CATALOG
            .iter()
            .map(|spec| {
                let path = self.model_path(spec);
                let verified = self.is_verified(spec);
                let in_flight = downloads
                    .as_ref()
                    .and_then(|map| map.get(spec.id))
                    .cloned()
                    .unwrap_or_default();
                let downloaded_bytes = if in_flight.status == "downloading" {
                    in_flight.downloaded
                } else {
                    let part = path.with_extension("gguf.part");
                    fs::metadata(&path)
                        .or_else(|_| fs::metadata(&part))
                        .map(|m| m.len())
                        .unwrap_or(0)
                };
                let status = if verified {
                    "ready".to_string()
                } else if in_flight.status == "downloading" {
                    "downloading".to_string()
                } else if in_flight.status == "failed" {
                    "failed".to_string()
                } else if downloaded_bytes > 0 {
                    "partial".to_string()
                } else {
                    "missing".to_string()
                };
                let progress = if in_flight.total > 0 {
                    (in_flight.downloaded as f64 / in_flight.total as f64).min(1.0)
                } else if spec.size_bytes > 0 {
                    (downloaded_bytes as f64 / spec.size_bytes as f64).min(1.0)
                } else {
                    0.0
                };
                let active_here = active
                    .as_ref()
                    .map(|value| value.id == spec.id)
                    .unwrap_or(false)
                    && runtime
                        .as_ref()
                        .map(|(id, _)| id == spec.id)
                        .unwrap_or(false);
                serde_json::json!({
                    "id": spec.id,
                    "name": spec.name,
                    "params": spec.params,
                    "context": spec.context,
                    "sizeBytes": spec.size_bytes,
                    "sha256": spec.sha256,
                    "license": spec.license,
                    "description": spec.description,
                    "recommended": spec.recommended,
                    "strictUnder2b": spec.strict_under_2b,
                    "runtime": spec.runtime,
                    "fileName": spec.file_name,
                    "downloadUrl": spec.download_url,
                    "modelId": spec.model_id,
                    "downloadedBytes": downloaded_bytes,
                    "verified": verified,
                    "status": status,
                    "progress": progress,
                    "error": in_flight.error,
                    "active": active_here,
                })
            })
            .collect();
        serde_json::json!({
            "items": items,
            "root": self.root,
            "active": active,
            "runtime": runtime.map(|(id, port)| serde_json::json!({
                "running": true,
                "id": id,
                "port": port,
                "baseUrl": format!("http://127.0.0.1:{port}/v1"),
            })).unwrap_or_else(|| serde_json::json!({"running": false})),
        })
    }

    pub fn start_download(&self, id: &str) -> Result<serde_json::Value, LocalModelError> {
        let spec = CATALOG
            .iter()
            .find(|spec| spec.id == id)
            .ok_or_else(|| LocalModelError::new("NOT_FOUND", format!("未知本地模型: {id}")))?;
        if self.is_verified(spec) {
            return Ok(serde_json::json!({"id": id, "status": "ready"}));
        }
        let mut downloads = self
            .downloads
            .lock()
            .map_err(|_| LocalModelError::new("INTERNAL", "下载状态锁失败"))?;
        if downloads
            .get(id)
            .map(|state| state.status == "downloading")
            .unwrap_or(false)
        {
            return Ok(serde_json::json!({"id": id, "status": "downloading"}));
        }
        downloads.insert(
            id.to_string(),
            DownloadState {
                status: "downloading".to_string(),
                downloaded: 0,
                total: spec.size_bytes,
                error: None,
            },
        );
        drop(downloads);

        let response_id = id.to_string();
        let thread_id = response_id.clone();
        std::thread::spawn(move || {
            let manager = Manager::global();
            if let Err(error) = manager.download_blocking(&thread_id) {
                if let Ok(mut downloads) = manager.downloads.lock() {
                    if let Some(state) = downloads.get_mut(&thread_id) {
                        state.status = "failed".to_string();
                        state.error = Some(error);
                    }
                }
            }
        });
        Ok(serde_json::json!({"id": response_id, "status": "downloading"}))
    }

    fn update_progress(&self, id: &str, downloaded: u64, total: u64) {
        if let Ok(mut downloads) = self.downloads.lock() {
            if let Some(state) = downloads.get_mut(id) {
                state.downloaded = downloaded;
                state.total = total;
            }
        }
    }

    fn download_blocking(&self, id: &str) -> Result<(), String> {
        let spec = CATALOG
            .iter()
            .find(|spec| spec.id == id)
            .ok_or("未知模型")?;
        let directory = self.model_dir(spec);
        fs::create_dir_all(&directory).map_err(|e| format!("创建模型目录失败: {e}"))?;
        let final_path = self.model_path(spec);
        let part_path = final_path.with_extension("gguf.part");
        if part_path.exists() {
            let _ = fs::remove_file(&part_path);
        }

        // ModelScope's CDN rejects some TLS fingerprints even with a browser UA.
        // curl is the most reliable transport here and is present on the hosts
        // DigiLegacy targets; progress is polled from the growing part file.
        let log_path = directory.join("download.log");
        let log = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)
            .map_err(|e| format!("打开下载日志失败: {e}"))?;
        let mut child = Command::new("curl")
            .arg("-L")
            .arg("--fail")
            .arg("--retry")
            .arg("3")
            .arg("--retry-delay")
            .arg("5")
            .arg("--connect-timeout")
            .arg("30")
            .arg("-o")
            .arg(&part_path)
            .arg(spec.download_url)
            .stdout(Stdio::null())
            .stderr(Stdio::from(log))
            .spawn()
            .map_err(|e| format!("启动 curl 下载失败: {e}"))?;
        let total = spec.size_bytes;
        loop {
            match child.try_wait() {
                Ok(Some(status)) => {
                    if !status.success() {
                        return Err(format!(
                            "下载源返回失败（curl exit {}）；日志 {}",
                            status.code().unwrap_or(-1),
                            log_path.display()
                        ));
                    }
                    break;
                }
                Ok(None) => {
                    let downloaded = fs::metadata(&part_path).map(|m| m.len()).unwrap_or(0);
                    self.update_progress(id, downloaded, total);
                    std::thread::sleep(Duration::from_millis(400));
                }
                Err(error) => return Err(format!("等待下载进程失败: {error}")),
            }
        }

        let downloaded = fs::metadata(&part_path).map(|m| m.len()).unwrap_or(0);
        let digest = hash_file(&part_path)?;
        if digest != spec.sha256 {
            let _ = fs::remove_file(&part_path);
            return Err(format!(
                "SHA256 不匹配: 期望 {}，实际 {}",
                spec.sha256, digest
            ));
        }
        fs::rename(&part_path, &final_path).map_err(|e| format!("落盘失败: {e}"))?;
        fs::write(self.marker_path(spec), format!("{}\n", spec.sha256))
            .map_err(|e| format!("写入校验标记失败: {e}"))?;
        self.update_progress(id, downloaded, total);
        if let Ok(mut downloads) = self.downloads.lock() {
            if let Some(state) = downloads.get_mut(id) {
                state.status = "ready".to_string();
                state.downloaded = downloaded;
                state.total = total;
                state.error = None;
            }
        }
        Ok(())
    }

    pub fn activate(&self, id: &str) -> Result<serde_json::Value, LocalModelError> {
        let spec = CATALOG
            .iter()
            .find(|spec| spec.id == id)
            .ok_or_else(|| LocalModelError::new("NOT_FOUND", format!("未知本地模型: {id}")))?;
        if !self.is_verified(spec) {
            return Err(LocalModelError::new(
                "CONFLICT",
                "模型尚未下载并通过 SHA256 校验",
            ));
        }
        let path = self.model_path(spec);
        if let Some(active) = self.read_active() {
            if active.id == spec.id && self.probe_active(&active).is_some() {
                self.write_active(
                    spec,
                    &path,
                    active_port(&active).unwrap_or(DEFAULT_PORT),
                    active.pid,
                )
                .map_err(|error| LocalModelError::new("IO", error))?;
                return Ok(self.status());
            }
            if let Some(pid) = active.pid {
                terminate_pid(pid);
            }
        }
        if let Some((active_id, port)) = self.running() {
            if active_id == spec.id {
                self.write_active(spec, &path, port, None)
                    .map_err(|error| LocalModelError::new("IO", error))?;
                return Ok(self.status());
            }
        }

        // A marker alone is not proof: verify the bytes before spending time
        // loading a corrupted file into a sidecar.
        let digest = hash_file(&path).map_err(|error| LocalModelError::new("IO", error))?;
        if digest != spec.sha256 {
            let _ = fs::remove_file(self.marker_path(spec));
            return Err(LocalModelError::new(
                "CORRUPT_DATA",
                format!(
                    "模型文件 SHA256 校验失败: 期望 {}，实际 {}；文件已标记为未完成，请重新下载",
                    spec.sha256, digest
                ),
            ));
        }

        self.stop_runtime();
        let script = self
            .find_server_script()
            .map_err(|error| LocalModelError::new("PROVIDER_ERROR", error))?;
        let python = std::env::var("DIGILEGACY_PYTHON").unwrap_or_else(|_| "python3".to_string());
        let port = self
            .free_port(DEFAULT_PORT)
            .map_err(|error| LocalModelError::new("INTERNAL", error))?;
        let log_path = self.root.join("runtime.log");
        let log = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)
            .map_err(|error| LocalModelError::new("IO", format!("打开运行时日志失败: {error}")))?;
        let log_err = log
            .try_clone()
            .map_err(|error| LocalModelError::new("IO", format!("复制日志句柄失败: {error}")))?;
        let mut child = Command::new(python)
            .arg(script)
            .arg("--model-path")
            .arg(&path)
            .arg("--model-id")
            .arg(spec.model_id)
            .arg("--port")
            .arg(port.to_string())
            .arg("--ctx")
            .arg("8192")
            .stdout(Stdio::from(log))
            .stderr(Stdio::from(log_err))
            .spawn()
            .map_err(|error| {
                LocalModelError::new("PROVIDER_ERROR", format!("启动本地推理进程失败: {error}"))
            })?;
        let child_pid = child.id();
        let deadline = Instant::now() + Duration::from_secs(45);
        while Instant::now() < deadline {
            if let Ok(Some(status)) = child.try_wait() {
                let code = status.code().unwrap_or(-1);
                return Err(LocalModelError::new(
                    "PROVIDER_ERROR",
                    format!(
                        "本地推理进程提前退出（exit {code}）；查看 {}",
                        log_path.display()
                    ),
                ));
            }
            if reqwest::blocking::get(format!("http://127.0.0.1:{port}/health"))
                .map(|response| response.status().is_success())
                .unwrap_or(false)
            {
                if let Ok(mut runtime) = self.runtime.lock() {
                    *runtime = Some(RuntimeHandle {
                        child,
                        port,
                        id: spec.id.to_string(),
                    });
                } else {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(LocalModelError::new("INTERNAL", "本地运行时状态锁失败"));
                }
                if let Err(error) = self.write_active(spec, &path, port, Some(child_pid)) {
                    self.stop_runtime();
                    return Err(LocalModelError::new("IO", error));
                }
                return Ok(self.status());
            }
            std::thread::sleep(Duration::from_millis(500));
        }
        let _ = child.kill();
        let _ = child.wait();
        Err(LocalModelError::new(
            "TIMEOUT",
            format!("本地推理服务在 45 秒内未就绪；查看 {}", log_path.display()),
        ))
    }

    fn probe_active(&self, active: &ActiveConfig) -> Option<(String, u16)> {
        let port = active_port(active)?;
        let address = format!("127.0.0.1:{port}");
        std::net::TcpStream::connect_timeout(&address.parse().ok()?, Duration::from_millis(300))
            .ok()?;
        Some((active.id.clone(), port))
    }

    fn write_active(
        &self,
        spec: &ModelSpec,
        path: &Path,
        port: u16,
        pid: Option<u32>,
    ) -> Result<(), String> {
        let active = ActiveConfig {
            id: spec.id.to_string(),
            model: spec.model_id.to_string(),
            base_url: format!("http://127.0.0.1:{port}/v1"),
            api_key: "local".to_string(),
            path: path.to_string_lossy().to_string(),
            pid,
        };
        fs::create_dir_all(&self.root).map_err(|e| format!("创建模型目录失败: {e}"))?;
        fs::write(
            self.active_path(),
            serde_json::to_string_pretty(&active)
                .map_err(|e| format!("序列化启用配置失败: {e}"))?,
        )
        .map_err(|e| format!("写入启用配置失败: {e}"))
    }

    fn running(&self) -> Option<(String, u16)> {
        let mut runtime = self.runtime.lock().ok()?;
        let handle = runtime.as_mut()?;
        match handle.child.try_wait() {
            Ok(None) => Some((handle.id.clone(), handle.port)),
            _ => None,
        }
    }

    pub fn stop(&self) -> serde_json::Value {
        self.stop_runtime();
        let _ = fs::remove_file(self.active_path());
        serde_json::json!({"stopped": true})
    }

    fn stop_runtime(&self) {
        if let Ok(mut runtime) = self.runtime.lock() {
            if let Some(mut handle) = runtime.take() {
                let _ = handle.child.kill();
                let _ = handle.child.wait();
                return;
            }
        }
        if let Some(active) = self.read_active() {
            if let Some(pid) = active.pid {
                terminate_pid(pid);
            }
        }
    }

    fn free_port(&self, start: u16) -> Result<u16, String> {
        for offset in 0..MAX_PORT_TRIES {
            let port = start.saturating_add(offset);
            if let Ok(listener) = TcpListener::bind(("127.0.0.1", port)) {
                drop(listener);
                return Ok(port);
            }
        }
        Err(format!(
            "没有可用的本地推理端口（{start}..{}）",
            start + MAX_PORT_TRIES
        ))
    }

    fn find_server_script(&self) -> Result<PathBuf, String> {
        if let Ok(path) = std::env::var("DIGILEGACY_LOCAL_MODEL_SERVER") {
            if Path::new(&path).exists() {
                return Ok(PathBuf::from(path));
            }
        }
        let mut candidates = vec![PathBuf::from("tools/local_model_server.py")];
        if let Ok(exe) = std::env::current_exe() {
            if let Some(parent) = exe.parent() {
                candidates.push(parent.join("../tools/local_model_server.py"));
                candidates.push(parent.join("../../tools/local_model_server.py"));
            }
        }
        for candidate in candidates {
            if candidate.exists() {
                return Ok(candidate);
            }
        }
        Err("找不到 tools/local_model_server.py；请设置 DIGILEGACY_LOCAL_MODEL_SERVER".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_spec() -> ModelSpec {
        ModelSpec {
            id: "test-model",
            name: "Test",
            params: "1B",
            context: 8,
            size_bytes: 8,
            sha256: "abcd",
            license: "Apache-2.0",
            description: "",
            recommended: false,
            strict_under_2b: true,
            runtime: "llama.cpp",
            file_name: "test.gguf",
            download_url: "",
            model_id: "Test",
        }
    }

    #[test]
    fn marker_requires_matching_file_size() {
        let root = std::env::temp_dir().join(format!("digilegacy-lm-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let manager = Manager::new(root.clone());
        let spec = test_spec();
        let dir = manager.model_dir(&spec);
        fs::create_dir_all(&dir).unwrap();
        fs::write(manager.model_path(&spec), b"short").unwrap();
        fs::write(manager.marker_path(&spec), spec.sha256).unwrap();
        assert!(!manager.is_verified(&spec));

        fs::write(manager.model_path(&spec), b"12345678").unwrap();
        assert!(manager.is_verified(&spec));
        let _ = fs::remove_dir_all(root);
    }
}
