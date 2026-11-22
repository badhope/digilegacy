#!/usr/bin/env node
/**
 * Architecture gate for M0. It checks structure and contract edges, not business behavior.
 * Run: node tools/arch-check.mjs
 */
import fs from "node:fs";
import path from "node:path";
import { spawnSync } from "node:child_process";

const root = path.resolve(new URL("..", import.meta.url).pathname);
const failures = [];
const notes = [];

function fail(message) {
  failures.push(message);
}

function read(relative) {
  return fs.readFileSync(path.join(root, relative), "utf8");
}

function exists(relative) {
  return fs.existsSync(path.join(root, relative));
}

function parseMembers() {
  const cargo = read("Cargo.toml");
  const match = cargo.match(/members\s*=\s*\[([\s\S]*?)\]/);
  if (!match) return [];
  return [...match[1].matchAll(/"([^"]+)"/g)].map((item) => item[1]);
}

const members = parseMembers();
if (members.length !== 12) fail(`workspace members: expected 12, got ${members.length}`);
for (const member of members) {
  if (!exists(`${member}/Cargo.toml`)) fail(`workspace member missing Cargo.toml: ${member}`);
}
notes.push(`workspace members: ${members.length}`);

const coreCrates = members.filter((member) => member.startsWith("core/"));
const appCrates = members.filter((member) => member.startsWith("apps/"));
for (const member of coreCrates) {
  const cargo = read(`${member}/Cargo.toml`);
  if (cargo.includes("apps/")) fail(`core crate depends on apps/: ${member}`);
  if (cargo.includes("../../legacy") || cargo.includes("legacy/app")) fail(`core crate references legacy: ${member}`);
}
for (const member of appCrates) {
  const cargo = read(`${member}/Cargo.toml`);
  if (cargo.includes("../../legacy") || cargo.includes("legacy/app")) fail(`app crate references legacy: ${member}`);
}
notes.push(`core crates: ${coreCrates.length}; app crates: ${appCrates.length}`);

const commands = JSON.parse(read("contracts/commands.json")).commands;
const tools = JSON.parse(read("contracts/tools.json")).tools;
const errors = JSON.parse(read("contracts/errors.json")).errors;
const schemas = fs.readdirSync(path.join(root, "contracts/schemas")).filter((name) => name.endsWith(".json"));

if (commands.length !== 49) fail(`commands: expected 49, got ${commands.length}`);
if (tools.length !== 23) fail(`tools: expected 23, got ${tools.length}`);
if (errors.length !== 21) fail(`errors: expected 21, got ${errors.length}`);
if (schemas.length !== 67) fail(`schemas: expected 67, got ${schemas.length}`);
notes.push(`contracts: ${commands.length} commands, ${tools.length} tools, ${schemas.length} schemas, ${errors.length} errors`);

const commandNames = new Set();
for (const command of commands) {
  if (commandNames.has(command.name)) fail(`duplicate command: ${command.name}`);
  commandNames.add(command.name);
  for (const schema of [command.input, command.output]) {
    if (!exists(`contracts/${schema}`)) fail(`missing schema: ${command.name} -> ${schema}`);
  }
  if (!["read", "draft", "write", "key", "network", "desktop"].includes(command.permission)) {
    fail(`invalid permission: ${command.name} -> ${command.permission}`);
  }
  const http = command.transports?.http;
  if (http) {
    if (!/^(GET|POST|PATCH|PUT|DELETE) \/api\/v1\//.test(http)) fail(`invalid HTTP transport: ${command.name} -> ${http}`);
    if (["key", "desktop"].includes(command.permission)) {
      fail(`HTTP must not expose ${command.permission} command: ${command.name}`);
    }
  }
  // Orchestrator commands are invoked by a human; their own write steps still
  // create approvals in the runtime.
  const orchestrators = new Set(["agent.approvals.decide", "agent.runs.start", "agent.runs.cancel", "agent.runs.retry"]);
  if (command.permission === "write" && command.agentApproval !== "required" && !orchestrators.has(command.name)) {
    fail(`write command must require approval: ${command.name}`);
  }
  for (const code of command.errors ?? []) {
    if (!errors.some((error) => error.code === code)) fail(`unknown error code: ${command.name} -> ${code}`);
  }
}

const toolNames = new Set();
for (const tool of tools) {
  if (toolNames.has(tool.name)) fail(`duplicate tool: ${tool.name}`);
  toolNames.add(tool.name);
  if (!commandNames.has(tool.human_equivalent)) fail(`tool points to unknown command: ${tool.name}`);
  if (tool.permission === "write" && tool.approval !== "required") fail(`write tool must require approval: ${tool.name}`);
  if (tool.mcp && !["read", "draft"].includes(tool.permission)) fail(`MCP tool must be read/draft: ${tool.name}`);
  for (const schema of [tool.input_schema, tool.output_schema]) {
    if (!exists(`contracts/${schema}`)) fail(`missing tool schema: ${tool.name} -> ${schema}`);
  }
}

const router = read("web/src/app/router.ts");
for (const legacy of ["/dashboard", "/discover", "/profile", "/dms", "/legal", "/report", "/heir", "/review"]) {
  if (router.includes(`path: "${legacy}"`)) fail(`legacy route resurrected: ${legacy}`);
}
notes.push("legacy route gate: 8 routes remain unresolved by catch-all");

function walk(dir) {
  const files = [];
  for (const entry of fs.readdirSync(path.join(root, dir), { withFileTypes: true })) {
    const relative = path.join(dir, entry.name);
    if (entry.isDirectory()) files.push(...walk(relative));
    else if (/\.(ts|vue|rs|mjs)$/.test(entry.name)) files.push(relative);
  }
  return files;
}

for (const file of walk("web/src")) {
  const content = read(file);
  if (content.includes("legacy/app") || content.includes("legacy\\app")) fail(`web references legacy app: ${file}`);
}

const generatedFiles = [
  "web/src/platform/generated/ipc.ts",
  "web/src/platform/generated/mock.ts",
  "web/src/platform/generated/tools.ts",
  "core/contracts/src/generated.rs",
  "core/contracts/src/errors.rs",
  "apps/cli/src/generated.rs",
  "apps/server/src/generated.rs",
  "apps/mcp/tools.generated.json",
  "contracts/generated/tool-manifest.json",
  "contracts/openapi.json",
];
for (const file of generatedFiles) {
  if (!exists(file)) fail(`missing generated artifact: ${file}`);
}
notes.push(`generated artifacts: ${generatedFiles.length}`);

const generate = spawnSync(process.execPath, ["tools/generate.mjs", "--check"], {
  cwd: root,
  encoding: "utf8",
});
if (generate.status !== 0) {
  fail(`contracts are stale:\n${generate.stdout || generate.stderr}`);
}

if (failures.length) {
  console.error(`arch-check: FAILED (${failures.length})`);
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("arch-check: OK");
for (const note of notes) console.log(`- ${note}`);
