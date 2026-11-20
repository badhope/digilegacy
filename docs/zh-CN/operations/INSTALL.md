# 安装、升级与卸载

发布包对应单节点部署形态：一个 API 进程、CLI 与 MCP 二进制、静态 Web 资源、政策内容与运维文档。

## 构建发布包

```bash
./tools/package/build.sh
```

输出为 `packages/digilegacy-<version>-linux-<arch>.tar.gz` 与同名 `.sha256` 文件。`web/node_modules` 不存在时会使用仓库里的 `web/package-lock.json` 安装依赖，然后走与验证门禁相同的 release 构建。

## 全新安装

```bash
tar -xzf packages/digilegacy-0.0.18-linux-x86_64.tar.gz -C /tmp
cd /tmp/digilegacy-0.0.18
./install.sh --prefix /opt/digilegacy --data-dir /var/lib/digilegacy
/opt/digilegacy/bin/digilegacy-start
```

安装脚本可重复执行用于升级：替换二进制、Web 资源、内容与文档，但保留 `etc/digilegacy.env` 和数据目录。写令牌只生成一次，且不打印。

`DIGILEGACY_ALLOW_PRIVATE_NETWORK` 与 `DIGILEGACY_OUTBOUND_ALLOWLIST` 不由安装脚本写入；部署使用 LAN 集成或明确出站主机表时，请手动加入环境文件。

## Web 控制台

API 不直接托管 SPA，需要反向代理：

可直接修改的同源 server 块随包提供：`<prefix>/share/nginx/digilegacy.conf.example`（源码位置 `packaging/nginx/digilegacy.conf.example`）。把 `root` 指向安装后的 `web/` 目录，并保持 `/api/` 同源。简化示例如下：

```nginx
server {
  listen 443 ssl;
  server_name digilegacy.example.com;

  root /opt/digilegacy/web;
  index index.html;

  location / {
    try_files $uri $uri/ /index.html;
  }

  location /api/ {
    proxy_pass http://127.0.0.1:8787;
    proxy_set_header Host $host;
    proxy_set_header X-Forwarded-Proto $scheme;
  }
}
```

控制台按同源 `/api` 调用；除非明确补了 CORS，不要把 `VITE_API_BASE` 指向另一个源。示例已在 nginx 1.18.0 实测：`/`、`/settings`、`/api/v1/health` 与哈希资源均返回 200，安全/缓存头与样例一致；证据 `docs/operations/measurements/proxy-2026-11-20.json`。

## 导出与检查点

```bash
set -a; . /opt/digilegacy/etc/digilegacy.env; set +a
/opt/digilegacy/bin/digilegacy-cli diagnostics export --allow-write --json
/opt/digilegacy/bin/digilegacy-cli security checkpoint --label pre-upgrade --json
/opt/digilegacy/bin/digilegacy-cli security verify <id> --json
```

`diagnostics export` 是脱敏结构快照，不是完整数据导出。迁移或灾难恢复应在停服后复制数据库与两个密钥文件，或按运行手册的离线检查点流程执行。

## 升级

1. 创建并验证检查点，记录 id。
2. 停服：`digilegacy-stop`。
3. 解压新发布包，用相同的 `--prefix` 与 `--data-dir` 运行 `install.sh`。
4. 启动后检查 `/api/v1/health`、`/api/v1/metrics` 与一条读路由。
5. 新版本失败时停服，按离线运行手册恢复旧二进制或检查点。

## 卸载

```bash
/opt/digilegacy/bin/digilegacy-uninstall
```

默认删除安装目录，但保留数据目录，并把 `db.key` / `config.key` 复制到 `<data>/keys/`。之后用同一 `--data-dir` 重装，安装脚本会先恢复这两个密钥，保留的数据库仍可读。`--purge-data` 会连数据目录一起删除，只应在已验证备份后使用。
