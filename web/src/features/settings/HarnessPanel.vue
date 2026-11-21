
<script setup lang="ts">
import { computed, ref } from "vue";
import { RouterLink } from "vue-router";
import SectionCard from "@/shared/ui/SectionCard.vue";
import StatusPill from "@/shared/ui/StatusPill.vue";
import { platformClient } from "@/platform/client";
import { useI18n } from "@/platform/i18n";
import { useResource } from "@/platform/useResource";
import type { AgentSkill, BuiltinMcpEntry, ConnectionItem, SkillUpsertInput } from "@/platform/types";

const { locale } = useI18n();
const status = useResource(() => platformClient.harnessStatus());
const connections = useResource(() => platformClient.listConnections());
const skills = useResource(() => platformClient.listSkills());

const copy = computed(() =>
  locale.value === "zh-CN"
    ? {
        overview: "Harness 总览",
        overviewNote: "只读聚合：规划器来源、运行、审批、用量、工具目录与 MCP/Skill 计数。",
        builtinBadge: "内置",
        catalogTitle: "MCP 目录",
        docs: "文档",
        planner: "规划器",
        runs: "运行",
        approvals: "待审批",
        usage: "用量",
        tools: "工具目录",
        mcp: "MCP 服务器",
        skills: "Skills",
        configured: "已配置",
        notConfigured: "未配置",
        sourceStored: "已保存连接",
        sourceEnv: "环境变量",
        sourceLocal: "本地模型",
        sourceNone: "无",
        model: "模型",
        endpoint: "端点",
        timeout: "超时",
        total: "总数",
        running: "进行中",
        waiting: "等待审批",
        failed: "失败",
        pending: "待处理",
        tokens: "token",
        cost: "成本",
        unpriced: "部分未计价",
        mcpTools: "MCP 工具",
        writeTools: "写工具",
        draftTools: "草稿工具",
        servers: "服务器",
        enabled: "已启用",
        openTools: "打开工具目录",
        openApprovals: "打开审批",
        mcpTitle: "MCP 客户端",
        mcpNote: "连接外部 MCP HTTP 服务器，测试时执行 initialize + tools/list；发现的工具数会显示在测试结果里。",
        mcpEmpty: "还没有 MCP 服务器。",
        addMcp: "新增 MCP 服务器",
        mcpLabel: "名称",
        mcpEndpoint: "MCP HTTP 地址",
        mcpToken: "Bearer 凭据（可选）",
        save: "保存",
        saving: "保存中…",
        cancel: "取消",
        test: "测试",
        testing: "测试中…",
        remove: "删除",
        deleteConfirm: "确定删除这条 MCP 配置？",
        tokenMissing: "缺少平台写令牌：到“安全与数据”填写 DIGILEGACY_API_TOKEN。",
        mcpOnly: "当前只支持 HTTP JSON-RPC MCP 服务器；stdio 服务需先自建 HTTP 网关。",
        skillsTitle: "Skill 注册表",
        skillsNote: "启用的 Skill 指令会追加到规划器系统提示词；平台不执行 Skill 里的脚本。",
        skillsEmpty: "还没有 Skill。",
        addSkill: "新增 Skill",
        skillName: "名称",
        skillDescription: "描述",
        skillInstructions: "指令（Markdown）",
        skillEnabled: "启用并注入规划器",
        skillSave: "保存",
        skillEdit: "编辑",
        skillDelete: "删除",
        skillDeleteConfirm: "确定删除这个 Skill？",
        enabledLabel: "启用",
        disabledLabel: "停用",
        noSelection: "选择一条 Skill 进行编辑",
        refresh: "刷新",
        source: "来源",
      }
    : {
        overview: "Harness overview",
        overviewNote: "Read-only totals: planner source, runs, approvals, usage, tool catalog and MCP/Skill counts.",
        builtinBadge: "Built-in",
        catalogTitle: "MCP catalog",
        docs: "Docs",
        planner: "Planner",
        runs: "Runs",
        approvals: "Approvals",
        usage: "Usage",
        tools: "Tool catalog",
        mcp: "MCP servers",
        skills: "Skills",
        configured: "Configured",
        notConfigured: "Not configured",
        sourceStored: "Saved connection",
        sourceEnv: "Environment",
        sourceLocal: "Local model",
        sourceNone: "None",
        model: "Model",
        endpoint: "Endpoint",
        timeout: "Timeout",
        total: "Total",
        running: "Running",
        waiting: "Waiting approval",
        failed: "Failed",
        pending: "Pending",
        tokens: "tokens",
        cost: "Cost",
        unpriced: "Partially unpriced",
        mcpTools: "MCP tools",
        writeTools: "Write tools",
        draftTools: "Draft tools",
        servers: "Servers",
        enabled: "Enabled",
        openTools: "Open tool catalog",
        openApprovals: "Open approvals",
        mcpTitle: "MCP client",
        mcpNote: "Connect an external MCP HTTP server. Test runs initialize + tools/list and reports the discovered tool count.",
        mcpEmpty: "No MCP servers yet.",
        addMcp: "Add MCP server",
        mcpLabel: "Label",
        mcpEndpoint: "MCP HTTP endpoint",
        mcpToken: "Bearer credential (optional)",
        save: "Save",
        saving: "Saving…",
        cancel: "Cancel",
        test: "Test",
        testing: "Testing…",
        remove: "Delete",
        deleteConfirm: "Delete this MCP config?",
        tokenMissing: "Missing platform write token: fill DIGILEGACY_API_TOKEN in Security & data.",
        mcpOnly: "Only HTTP JSON-RPC MCP servers are supported; wrap stdio servers in an HTTP gateway first.",
        skillsTitle: "Skill registry",
        skillsNote: "Enabled skill instructions are appended to the planner system prompt; scripts inside skills are never executed.",
        skillsEmpty: "No skills yet.",
        addSkill: "Add skill",
        skillName: "Name",
        skillDescription: "Description",
        skillInstructions: "Instructions (Markdown)",
        skillEnabled: "Enable and inject into planner",
        skillSave: "Save",
        skillEdit: "Edit",
        skillDelete: "Delete",
        skillDeleteConfirm: "Delete this skill?",
        enabledLabel: "Enabled",
        disabledLabel: "Disabled",
        noSelection: "Select a skill to edit",
        refresh: "Refresh",
        source: "Source",
      },
);

const mcpItems = computed(() => connections.data.value?.items.filter((item) => item.kind === "mcp") ?? []);
const mcpOpen = ref(false);
const mcpSaving = ref(false);
const mcpError = ref("");
const mcpNotice = ref("");
const testingId = ref<string | null>(null);
const mcpForm = ref({ label: "", endpoint: "", token: "" });

const skillOpen = ref(false);
const skillSaving = ref(false);
const skillError = ref("");
const skillNotice = ref("");
const editingSkillId = ref("");
const skillForm = ref<{ name: string; description: string; instructions: string; enabled: boolean }>({
  name: "",
  description: "",
  instructions: "",
  enabled: true,
});

function sourceLabel(source: string): string {
  if (source === "stored") return copy.value.sourceStored;
  if (source === "env") return copy.value.sourceEnv;
  if (source === "local") return copy.value.sourceLocal;
  return copy.value.sourceNone;
}

function stopReason(item: ConnectionItem): string {
  if (!item.enabled) return copy.value.disabledLabel;
  return item.lastStatus;
}

function resetMcpForm() {
  mcpForm.value = { label: "", endpoint: "", token: "" };
  mcpError.value = "";
}

function openAddMcp() {
  resetMcpForm();
  mcpOpen.value = true;
}

async function saveMcp() {
  mcpSaving.value = true;
  mcpError.value = "";
  try {
    const saved = await platformClient.saveConnection({
      kind: "mcp",
      label: mcpForm.value.label.trim() || "mcp",
      endpoint: mcpForm.value.endpoint.trim(),
      token: mcpForm.value.token.trim() || undefined,
      enabled: true,
    });
    if (!saved.success) {
      mcpError.value = saved.message;
      return;
    }
    mcpNotice.value = locale.value === "zh-CN" ? "MCP 配置已保存" : "MCP config saved";
    mcpOpen.value = false;
    resetMcpForm();
    connections.reload();
    status.reload();
  } finally {
    mcpSaving.value = false;
  }
}

async function testMcp(item: ConnectionItem) {
  testingId.value = item.id;
  mcpNotice.value = "";
  try {
    const result = await platformClient.testConnection({ kind: "mcp", id: Number(item.id) });
    mcpNotice.value = `${item.label}: ${result.data?.message ?? result.message}`;
    connections.refresh();
    status.reload();
  } finally {
    testingId.value = null;
  }
}

async function deleteMcp(item: ConnectionItem) {
  if (!window.confirm(copy.value.deleteConfirm)) return;
  const result = await platformClient.deleteConnection(item.id);
  if (!result.success) {
    mcpError.value = result.message;
    return;
  }
  mcpNotice.value = locale.value === "zh-CN" ? "MCP 配置已删除" : "MCP config deleted";
  connections.reload();
  status.reload();
}

function resetSkillForm() {
  skillForm.value = { name: "", description: "", instructions: "", enabled: true };
  editingSkillId.value = "";
  skillError.value = "";
}

function openAddSkill() {
  resetSkillForm();
  skillOpen.value = true;
}

function editSkill(skill: AgentSkill) {
  skillForm.value = {
    name: skill.name,
    description: skill.description,
    instructions: skill.instructions,
    enabled: skill.enabled,
  };
  editingSkillId.value = skill.id;
  skillOpen.value = true;
  skillError.value = "";
}

async function saveSkill() {
  skillSaving.value = true;
  skillError.value = "";
  try {
    const payload: SkillUpsertInput = {
      name: skillForm.value.name.trim(),
      description: skillForm.value.description.trim(),
      instructions: skillForm.value.instructions,
      enabled: skillForm.value.enabled,
    };
    if (editingSkillId.value) payload.id = editingSkillId.value;
    const result = await platformClient.saveSkill(payload);
    if (!result.success) {
      skillError.value = result.message;
      return;
    }
    skillNotice.value = locale.value === "zh-CN" ? "Skill 已保存" : "Skill saved";
    skillOpen.value = false;
    resetSkillForm();
    skills.reload();
    status.reload();
  } finally {
    skillSaving.value = false;
  }
}

async function toggleSkill(skill: AgentSkill) {
  const result = await platformClient.saveSkill({
    id: skill.id,
    name: skill.name,
    description: skill.description,
    instructions: skill.instructions,
    enabled: !skill.enabled,
  });
  if (!result.success) {
    skillError.value = result.message;
    return;
  }
  skills.reload();
  status.reload();
}

async function deleteSkill(skill: AgentSkill) {
  if (!window.confirm(copy.value.skillDeleteConfirm)) return;
  const result = await platformClient.deleteSkill(skill.id);
  if (!result.success) {
    skillError.value = result.message;
    return;
  }
  skillNotice.value = locale.value === "zh-CN" ? "Skill 已删除" : "Skill deleted";
  skills.reload();
  status.reload();
}

function scrollTo(id: string) {
  document.getElementById(id)?.scrollIntoView({ behavior: "smooth", block: "start" });
}

function refreshAll() {
  status.reload();
  connections.reload();
  skills.reload();
}
</script>

<template>
  <div class="harness-panel">
    <SectionCard :title="copy.overview" :subtitle="copy.overviewNote">
      <div class="harness-toolbar">
        <button type="button" class="link-button" @click="refreshAll">{{ copy.refresh }}</button>
        <RouterLink class="link-button" to="/automation/tools">{{ copy.openTools }}</RouterLink>
        <RouterLink class="link-button" to="/automation?tab=approvals">{{ copy.openApprovals }}</RouterLink>
      </div>
      <div v-if="status.loading.value" class="empty-note">{{ locale === 'zh-CN' ? '加载中…' : 'Loading…' }}</div>
      <div v-else-if="status.error.value" class="empty-note">{{ status.error.value }}</div>
      <div v-else-if="status.data.value" class="harness-grid">
        <div class="harness-card">
          <span class="harness-label">{{ copy.planner }}</span>
          <strong>{{ status.data.value.planner.configured ? status.data.value.planner.model || copy.configured : copy.notConfigured }}</strong>
          <small>{{ copy.source }}: {{ sourceLabel(status.data.value.planner.source) }}</small>
          <small v-if="status.data.value.planner.endpointHost" class="mono">{{ copy.endpoint }}: {{ status.data.value.planner.endpointHost }}</small>
          <small v-if="status.data.value.planner.timeoutSecs">{{ copy.timeout }}: {{ status.data.value.planner.timeoutSecs }}s</small>
        </div>
        <div class="harness-card">
          <span class="harness-label">{{ copy.runs }}</span>
          <strong>{{ status.data.value.runs.total }}</strong>
          <small>{{ copy.running }}: {{ status.data.value.runs.running }}</small>
          <small>{{ copy.waiting }}: {{ status.data.value.runs.waitingApproval }}</small>
          <small>{{ copy.failed }}: {{ status.data.value.runs.failed }}</small>
        </div>
        <div class="harness-card">
          <span class="harness-label">{{ copy.approvals }}</span>
          <strong>{{ status.data.value.approvals.pending }}</strong>
          <small>{{ copy.pending }}</small>
        </div>
        <div class="harness-card">
          <span class="harness-label">{{ copy.usage }}</span>
          <strong>{{ status.data.value.usage.tokensIn.toLocaleString() }} / {{ status.data.value.usage.tokensOut.toLocaleString() }}</strong>
          <small>{{ copy.tokens }}</small>
          <small>{{ copy.cost }}: {{ (status.data.value.usage.costMicros / 1000000).toFixed(4) }}</small>
          <small v-if="!status.data.value.usage.priced">{{ copy.unpriced }}</small>
        </div>
        <div class="harness-card">
          <span class="harness-label">{{ copy.tools }}</span>
          <strong>{{ status.data.value.tools.total }}</strong>
          <small>{{ copy.mcpTools }}: {{ status.data.value.tools.mcp }}</small>
          <small>{{ copy.writeTools }}: {{ status.data.value.tools.write }}</small>
          <small>{{ copy.draftTools }}: {{ status.data.value.tools.draft }}</small>
        </div>
        <div class="harness-card">
          <span class="harness-label">{{ copy.mcp }}</span>
          <strong>{{ status.data.value.mcp.servers }}</strong>
          <small>{{ copy.servers }}</small>
          <small>{{ copy.enabled }}: {{ status.data.value.mcp.enabled }}</small>
        </div>
        <div class="harness-card">
          <span class="harness-label">{{ copy.skills }}</span>
          <strong>{{ status.data.value.skills.total }}</strong>
          <small>{{ copy.enabled }}: {{ status.data.value.skills.enabled }}</small>
        </div>
      </div>
    </SectionCard>



    <div id="builtin-mcp">
      <SectionCard :title="copy.catalogTitle">
        <p class="harness-hint">{{ copy.mcpOnly }}</p>
        <div class="builtin-list">
          <details v-for="entry in status.data.value?.builtinMcp ?? []" :key="entry.id" class="builtin-entry" :open="entry.id === 'digilegacy-platform'">
            <summary><strong>{{ entry.name }}</strong><span class="mono">{{ entry.source }} · {{ entry.transport }}</span></summary>
            <p>{{ entry.description }}</p>
            <code>{{ entry.command }}</code>
            <a class="link-button" :href="entry.docsUrl" target="_blank" rel="noreferrer">{{ copy.docs }}</a>
          </details>
        </div>
      </SectionCard>
    </div>

    <SectionCard :title="copy.mcpTitle" :subtitle="copy.mcpNote">
      <div class="harness-toolbar">
        <button type="button" class="link-button" @click="openAddMcp">{{ copy.addMcp }}</button>
      </div>
      <form v-if="mcpOpen" class="harness-form" @submit.prevent="saveMcp">
        <label>
          <span>{{ copy.mcpLabel }}</span>
          <input v-model="mcpForm.label" maxlength="80" autocomplete="off" placeholder="remote-mcp" />
        </label>
        <label>
          <span>{{ copy.mcpEndpoint }}</span>
          <input v-model="mcpForm.endpoint" required autocomplete="off" placeholder="https://mcp.example.com/mcp" />
        </label>
        <label>
          <span>{{ copy.mcpToken }}</span>
          <input v-model="mcpForm.token" type="password" autocomplete="new-password" />
        </label>
        <p class="harness-hint">{{ copy.mcpOnly }}</p>
        <p v-if="mcpError" class="harness-error">{{ mcpError }}</p>
        <div class="harness-actions">
          <button type="button" class="link-button" @click="mcpOpen = false">{{ copy.cancel }}</button>
          <button type="submit" class="solid-button" :disabled="mcpSaving">
            {{ mcpSaving ? copy.saving : copy.save }}
          </button>
        </div>
      </form>
      <div v-if="!mcpItems.length" class="empty-note">{{ copy.mcpEmpty }}</div>
      <ul v-else class="harness-list">
        <li v-for="item in mcpItems" :key="item.id" class="harness-row">
          <div>
            <strong>{{ item.label }}</strong>
            <small class="mono">#{{ item.id }} · {{ stopReason(item) }}</small>
          </div>
          <div class="harness-row-actions">
            <button type="button" class="link-button" :disabled="testingId === item.id" @click="testMcp(item)">
              {{ testingId === item.id ? copy.testing : copy.test }}
            </button>
            <button type="button" class="link-button" @click="deleteMcp(item)">{{ copy.remove }}</button>
          </div>
        </li>
      </ul>
      <p v-if="mcpNotice" class="harness-notice">{{ mcpNotice }}</p>
    </SectionCard>

    <div id="builtin-skills">
    <SectionCard :title="copy.skillsTitle" :subtitle="copy.skillsNote">
      <div class="harness-toolbar">
        <button type="button" class="link-button" @click="openAddSkill">{{ copy.addSkill }}</button>
      </div>
      <form v-if="skillOpen" class="harness-form" @submit.prevent="saveSkill">
        <label>
          <span>{{ copy.skillName }}</span>
          <input v-model="skillForm.name" required maxlength="80" autocomplete="off" />
        </label>
        <label>
          <span>{{ copy.skillDescription }}</span>
          <input v-model="skillForm.description" maxlength="500" autocomplete="off" />
        </label>
        <label>
          <span>{{ copy.skillInstructions }}</span>
          <textarea v-model="skillForm.instructions" required maxlength="20000" rows="6" />
        </label>
        <label class="harness-check">
          <input v-model="skillForm.enabled" type="checkbox" />
          <span>{{ copy.skillEnabled }}</span>
        </label>
        <p v-if="skillError" class="harness-error">{{ skillError }}</p>
        <div class="harness-actions">
          <button type="button" class="link-button" @click="skillOpen = false">{{ copy.cancel }}</button>
          <button type="submit" class="solid-button" :disabled="skillSaving">
            {{ skillSaving ? copy.saving : copy.skillSave }}
          </button>
        </div>
      </form>
      <div v-if="!skills.data.value?.items.length" class="empty-note">{{ copy.skillsEmpty }}</div>
      <ul v-else class="harness-list">
        <li v-for="skill in skills.data.value.items" :key="skill.id" class="harness-row">
          <div>
            <strong>{{ skill.name }} <span v-if="skill.builtin" class="builtin-tag">{{ copy.builtinBadge }}</span></strong>
            <small>{{ skill.description || '—' }}</small>
            <small class="mono">{{ skill.enabled ? copy.enabledLabel : copy.disabledLabel }}</small>
          </div>
          <div class="harness-row-actions">
            <StatusPill :tone="skill.enabled ? 'ready' : 'planned'" :label="skill.enabled ? copy.enabledLabel : copy.disabledLabel" />
            <button type="button" class="link-button" @click="toggleSkill(skill)">
              {{ skill.enabled ? copy.disabledLabel : copy.enabledLabel }}
            </button>
            <button type="button" class="link-button" @click="editSkill(skill)">{{ copy.skillEdit }}</button>
            <button type="button" class="link-button" @click="deleteSkill(skill)">{{ copy.skillDelete }}</button>
          </div>
        </li>
      </ul>
      <p v-if="skillNotice" class="harness-notice">{{ skillNotice }}</p>
    </SectionCard>
    </div>
  </div>
</template>

<style scoped>
.builtin-list { display: flex; flex-direction: column; gap: 8px; }
.builtin-entry { padding: 10px 12px; border: 1px solid var(--line); border-radius: var(--radius-sm); background: var(--surface-soft); }
.builtin-entry summary { display: flex; justify-content: space-between; gap: 10px; cursor: pointer; }
.builtin-entry summary span { color: var(--ink-muted); font-size: 11px; }
.builtin-entry p { margin: 8px 0; color: var(--ink-muted); font-size: 12px; line-height: 1.55; }
.builtin-entry code { display: block; margin-bottom: 6px; padding: 7px 9px; border: 1px solid var(--line); border-radius: var(--radius-sm); background: var(--surface); font-size: 11px; overflow-wrap: anywhere; }
.harness-panel {
  display: flex;
  flex-direction: column;
  gap: 16px;
}
.harness-toolbar {
  display: flex;
  flex-wrap: wrap;
  gap: 12px;
  margin-bottom: 12px;
}
.harness-grid {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(180px, 1fr));
  gap: 10px;
}
.harness-card {
  display: flex;
  flex-direction: column;
  gap: 4px;
  padding: 12px;
  border: 1px solid var(--line);
  border-radius: var(--radius-sm);
  background: var(--surface-soft);
}
.harness-label {
  color: var(--ink-muted);
  font-size: 11px;
  letter-spacing: 0.08em;
  text-transform: uppercase;
}
.harness-card strong { font-size: 18px; }
.harness-card small { color: var(--ink-muted); font-size: 11px; }
.harness-form {
  display: grid;
  gap: 10px;
  padding: 12px;
  margin-bottom: 12px;
  border: 1px solid var(--line);
  border-radius: var(--radius-sm);
  background: var(--surface-soft);
}
.harness-form label { display: flex; flex-direction: column; gap: 4px; font-size: 12px; }
.harness-form input, .harness-form textarea {
  width: 100%;
  padding: 8px 10px;
  border: 1px solid var(--line);
  border-radius: var(--radius-sm);
  background: var(--surface);
  color: var(--ink);
  font: inherit;
}
.harness-form textarea { resize: vertical; }
.harness-check { flex-direction: row !important; align-items: center; gap: 8px !important; }
.harness-actions { display: flex; justify-content: flex-end; gap: 12px; }
.harness-error { margin: 0; color: var(--danger); font-size: 12px; }
.harness-notice { margin: 8px 0 0; color: var(--ink-muted); font-size: 12px; }
.harness-hint { margin: 0; color: var(--ink-muted); font-size: 11px; }
.harness-list { list-style: none; margin: 0; padding: 0; display: flex; flex-direction: column; gap: 8px; }
.harness-row {
  display: flex;
  justify-content: space-between;
  align-items: center;
  gap: 12px;
  padding: 10px 0;
  border-top: 1px solid var(--line);
}
.harness-row > div:first-child { display: flex; flex-direction: column; gap: 3px; min-width: 0; }
.harness-row small { color: var(--ink-muted); font-size: 11px; overflow-wrap: anywhere; }
.harness-row-actions { display: flex; align-items: center; gap: 10px; flex-wrap: wrap; justify-content: flex-end; }
@media (max-width: 720px) {
  .harness-row { align-items: flex-start; flex-direction: column; }
  .harness-row-actions { justify-content: flex-start; }
}
</style>
