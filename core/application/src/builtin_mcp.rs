//! Curated MCP catalog shown to every install.
//!
//! The catalog is metadata, not a running process. `bundled` entries ship in the
//! package; `official`/`vendor` entries are reviewed starting points that need a
//! local stdio gateway because this platform's MCP client speaks HTTP JSON-RPC.

use serde_json::{json, Value};

pub const BUILTIN_MCP: &[(&str, &str, &str, &str, &str, &str, &str)] = &[
    (
        "digilegacy-platform",
        "DigiLegacy 平台 MCP",
        "bundled",
        "stdio",
        "digilegacy-mcp",
        "随安装包提供的平台 MCP server，暴露 20 个只读/草稿工具；3 个写工具不通过 MCP 暴露，仍走平台审批。把 bin 目录加入 PATH 后即可被 Claude Desktop、Cursor 等 stdio 客户端调用。",
        "https://github.com/Morningstar202604/DigiLegacy",
    ),
    (
        "filesystem",
        "Filesystem",
        "official",
        "stdio",
        "npx -y @modelcontextprotocol/server-filesystem <你的数字遗产资料目录>",
        "官方参考实现：在指定根目录内读写和搜索文件。根目录必须白名单到单独的资料目录，不要指向用户主目录或根盘。",
        "https://github.com/modelcontextprotocol/servers/tree/main/src/filesystem",
    ),
    (
        "markitdown",
        "MarkItDown",
        "vendor",
        "stdio",
        "uvx markitdown-mcp",
        "Microsoft 文档转换：PDF、Word、Excel、PPT、HTML、EPub 等转 Markdown，适合把合同、账单、说明整理进资产库；扫描版 PDF 无文本层时会接近空结果，必须另配 OCR。",
        "https://github.com/microsoft/markitdown",
    ),
    (
        "memory",
        "Memory",
        "official",
        "stdio",
        "npx -y @modelcontextprotocol/server-memory",
        "官方参考实现：本地知识图谱，保存人物、账户、偏好之间的关系。适合记录“谁负责哪一类资产”，不要把密码或助记词写进图谱。",
        "https://github.com/modelcontextprotocol/servers/tree/main/src/memory",
    ),
    (
        "sequential-thinking",
        "Sequential Thinking",
        "official",
        "stdio",
        "npx -y @modelcontextprotocol/server-sequential-thinking",
        "官方参考实现：显式的多步推理草稿区，适合拆解跨境继承、平台申诉、加密资产交接等长链条任务。",
        "https://github.com/modelcontextprotocol/servers/tree/main/src/sequentialthinking",
    ),
    (
        "fetch",
        "Fetch",
        "official",
        "stdio",
        "uvx mcp-server-fetch",
        "官方参考实现：抓取公开网页并转成 Markdown。适合核对平台 Legacy 政策原文；只抓公开页面，不登录账户、不绕过访问控制。",
        "https://github.com/modelcontextprotocol/servers/tree/main/src/fetch",
    ),
];

pub fn catalog() -> Value {
    Value::Array(
        BUILTIN_MCP
            .iter()
            .map(
                |(id, name, source, transport, command, description, docs_url)| {
                    json!({
                        "id": id,
                        "name": name,
                        "source": source,
                        "transport": transport,
                        "command": command,
                        "description": description,
                        "docsUrl": docs_url,
                    })
                },
            )
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_has_one_bundled_and_five_reviewed_entries() {
        assert_eq!(BUILTIN_MCP.len(), 6);
        assert_eq!(
            BUILTIN_MCP
                .iter()
                .filter(|(_, _, source, _, _, _, _)| *source == "bundled")
                .count(),
            1
        );
        for entry in BUILTIN_MCP {
            assert!(entry.6.starts_with("https://"));
            assert!(!entry.4.is_empty());
        }
    }
}
