# 政策来源预审

政策库出厂时每一条 `policy_provenance.status` 都是 `unverified`。来源预审脚本只生成
**人工核对清单**，不是核实：平台首页能打开，不代表页面上有我们引用的继承规则。

## 运行

```bash
python3 tools/policies/source_audit.py
```

脚本是运维工具，依赖 Python `requests`；发布归档中位于 `tools/policies/source_audit.py`。输出：

- `docs/operations/measurements/policy-sources-2026-11-20.json`：机器证据与逐条状态；
- `docs/operations/policy-source-worklist.csv`：可直接用 Excel 打开的人工清单。

常用参数：

```bash
python3 tools/policies/source_audit.py --no-network
python3 tools/policies/source_audit.py --timeout 12 --workers 6
```

## 2026-11-20 预审结果

| 检查项 | 结果 |
| --- | --- |
| 政策条数 | 152 |
| 有 URL / HTTPS | 152 / 152 |
| 平台首页 URL | 150 |
| 具体政策深链 | 2（`iCloud`、`Apple ID` 共用同一 Apple 支持页） |
| 重复 URL 组 | 17 组，覆盖 38 条 |
| 可访问（2xx/3xx） | 134 |
| 被拦截（401/403/429） | 2 |
| 失效（4xx/5xx） | 2 |
| 超时 | 11 |
| 传输错误 | 3 |
| 需人工复核条目 | 152 |

152 条全部进入复核是刻意结果：每条要么是平台首页，要么是重复 URL，要么没有返回干净的
2xx/3xx。`timeout`、`blocked`、`transport error` **不能**直接判定来源失效；跨境访问、
反爬、站点维护都可能造成这些状态。

## 人工核对流程

1. 先按使用量/风险取 Top 30，不要一次核 152 条。
2. 把平台首页替换成具体官方帮助/规则页；不要用搜索引擎结果、博客或聚合站当来源。
3. 打开页面确认页面本身写了 `data_source` 引用的规则；在 `policy_provenance` 里记录
   核对人、日期、证据哈希。
4. 通过 `PATCH /api/v1/policies/{id}/provenance` 或对应 CLI/UI 更新单条。产品初始状态
   是 `unverified`，只有具名人工核对者才能置为 `verified`。
5. 改完 URL 后重跑预审。预审全绿仍不等于核实完成：它只说明 URL 存在、足够具体、不重复、
   当时可访问。

## 边界

- 脚本不改 `content/policies/platforms.json`、不改数据库、不改 provenance 状态。
- 机器预审不能单独作为 G-16 的关闭证据。
- 两条深链也不算完成：它们共用同一个 Apple 页面，仍要人工确认 iCloud/Apple ID 的具体规则。
- 政策内容是用户侧的内容决策。代码能提供溯源和证据，但不能虚构核对人或法律结论。
