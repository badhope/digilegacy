# Built-in starter pack (0.0.31)

Every install opens with a visible starter pack instead of an empty Harness. The
pack is deliberately split into four layers so "built-in" never means "silently
enabled" or "bundled model weights".

## Models

`MiniCPM5-1B` and `MiniCPM5-2B` are catalog entries with metadata, SHA-256,
license and a ModelScope download URL. Weights are **not** in the release archive.
They are downloaded on demand and are never enabled by default. The panel now
says this explicitly and marks each card as a built-in catalog entry.

Research notes reflected in the panel:

- MiniCPM5-1B: 1.08B dense, 128K context, Apache-2.0. Strong agentic/tool use
  for its class; complex multi-step tool calls remain weak.
- MiniCPM5-2B: 2.52B dense, 128K context, Apache-2.0. Vendor 34-benchmark average
  53.9 vs Qwen3.5-4B at 51.1. GGUF Q4 builds can fall into repetition loops;
  keep `repeat-penalty 1.15` or prefer Q8.

## Tools

The 23 platform tools are compile-time generated from `contracts/tools.json` and
shipped in every binary. The tool catalog page marks them as built-in: 20 MCP-exposed tools
(18 read + 2 draft) and 3 write-only tools. Write tools still require approval
and are deliberately absent from the MCP manifest.

## MCP catalog

`agent.harness.status.builtinMcp` returns six curated entries:

| Entry | Source | Transport | Use |
| --- | --- | --- | --- |
| DigiLegacy Platform MCP | bundled | stdio | The package's own `digilegacy-mcp` binary, 20 MCP-exposed read/draft tools |
| Filesystem | official | stdio | Scoped read/write inside one legacy folder |
| MarkItDown | vendor | stdio | PDF/Word/Excel/PPT to Markdown |
| Memory | official | stdio | Entity/relationship graph |
| Sequential Thinking | official | stdio | Explicit multi-step reasoning |
| Fetch | official | stdio | Public web page to Markdown |

Important boundary: this platform's MCP **client** only speaks HTTP JSON-RPC.
The bundled and official servers are stdio. The catalog therefore shows the real
command and a copy action; it does not pretend a one-click connection exists.
Use an external stdio MCP client, or put a reviewed HTTP gateway in front. The
bundled `digilegacy-mcp` is already usable by external MCP clients after adding
`bin/` to `PATH`.

Security rules for any MCP server:

- A server has the same filesystem permissions as its process.
- For Filesystem, point the root at one legacy-material directory, never `/`
  or the user's home directory.
- Read-only by default; production write only when the workflow requires it.
- Scanned PDFs through MarkItDown return near-empty text without OCR; add
  Tesseract/PaddleOCR or a vision model before ingesting them.

## Built-in skills

The schema seeds eight reviewable skills into `agent_skills` exactly once (introduced in v18 and retained in v19):

- 数字资产盘点 (enabled)
- 执行人交付手册 (enabled)
- 平台遗产政策核对 (enabled)
- 凭据与恢复材料交接
- 纪念与删除意愿
- 年度复核与演练
- 家属通知草稿
- 法律与管辖边界

The three core skills are enabled and fit well below the 12,000-character planner
budget. The other five are visible but disabled to avoid prompt bloat. Users can
edit, disable or delete any of them; the seed runs only when the schema is first
created, so a deleted built-in skill is not resurrected on restart.

Content is grounded in common digital-estate practice: an inventory, an executor
runbook, platform legacy tools, credentials stored outside the will, crypto
handled separately, annual review. It is prompt guidance, not legal advice.

## Visible UI

Settings → Agent Harness shows the built-in dimensions in its overview counts:
23 tools, 6 catalog entries and 8 seeded skills. The **MCP catalog** below the
overview lists every entry with source, transport, description, real command and
official docs link. Model catalog visibility lives in Settings → Connections →
Local models, where each card carries a “built-in catalog” badge. The Skill
registry marks built-in rows.
