<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, reactive, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import {
  Activity,
  Download,
  Edit3,
  FolderOpen,
  Gauge,
  Play,
  Plus,
  Radio,
  RefreshCw,
  RotateCcw,
  Save,
  Server,
  Square,
  Trash2,
  Upload,
  Wifi,
  X,
  Zap,
} from "@lucide/vue";
import type { AppRule, AppSnapshot, NodeInfo, RuleDraft, RuleStats } from "./types";

const snapshot = ref<AppSnapshot | null>(null);
const loading = ref(true);
const busy = ref<string | null>(null);
const error = ref<string | null>(null);
const subscriptionDraft = ref("");
const editorOpen = ref(false);
const editingId = ref<string | null>(null);
const ruleDraft = reactive<RuleDraft>({
  name: "",
  app_path: "",
  args: "",
  working_dir: "",
  selected_node: null,
  enabled: true,
});

let timer: number | undefined;

const rules = computed(() => snapshot.value?.config.rules ?? []);
const nodes = computed(() => snapshot.value?.nodes ?? []);
const status = computed(() => snapshot.value?.status ?? null);
const statsByRule = computed(() => {
  const map = new Map<string, RuleStats>();
  for (const stat of snapshot.value?.stats ?? []) {
    map.set(stat.rule_id, stat);
  }
  return map;
});

const selectableNodes = computed<NodeInfo[]>(() => {
  const hasDirect = nodes.value.some((node) => node.name === "DIRECT");
  return hasDirect ? nodes.value : [{ name: "DIRECT", node_type: "Builtin", delay: null, is_builtin: true }, ...nodes.value];
});

function setError(value: unknown) {
  error.value = value instanceof Error ? value.message : String(value);
}

async function loadState(quiet = false) {
  try {
    if (!quiet) loading.value = true;
    snapshot.value = await invoke<AppSnapshot>("get_state");
    subscriptionDraft.value = snapshot.value.config.subscription_url ?? "";
    error.value = null;
  } catch (err) {
    setError(err);
  } finally {
    loading.value = false;
  }
}

async function runAction(name: string, action: () => Promise<void>) {
  busy.value = name;
  try {
    await action();
    await loadState(true);
  } catch (err) {
    setError(err);
  } finally {
    busy.value = null;
  }
}

function openCreateRule() {
  editingId.value = null;
  Object.assign(ruleDraft, {
    name: "",
    app_path: "",
    args: "",
    working_dir: "",
    selected_node: selectableNodes.value[0]?.name ?? "DIRECT",
    enabled: true,
  });
  editorOpen.value = true;
}

function openEditRule(rule: AppRule) {
  editingId.value = rule.id;
  Object.assign(ruleDraft, {
    id: rule.id,
    name: rule.name,
    app_path: rule.app_path,
    args: rule.args,
    working_dir: rule.working_dir,
    selected_node: rule.selected_node ?? "DIRECT",
    enabled: rule.enabled,
  });
  editorOpen.value = true;
}

function closeEditor() {
  editorOpen.value = false;
  editingId.value = null;
}

async function saveRule() {
  const payload = {
    name: ruleDraft.name.trim(),
    app_path: ruleDraft.app_path.trim(),
    args: ruleDraft.args.trim(),
    working_dir: ruleDraft.working_dir.trim(),
    selected_node: ruleDraft.selected_node || "DIRECT",
    enabled: ruleDraft.enabled,
  };

  await runAction("save-rule", async () => {
    if (editingId.value) {
      await invoke("update_rule", { ruleId: editingId.value, input: payload });
    } else {
      await invoke("create_rule", { input: payload });
    }
    closeEditor();
  });
}

async function chooseExe() {
  const selected = await open({
    multiple: false,
    directory: false,
    filters: [{ name: "Executable", extensions: ["exe", "cmd", "bat"] }],
  });
  if (typeof selected === "string") {
    ruleDraft.app_path = selected;
    if (!ruleDraft.name) {
      ruleDraft.name = selected.split(/[\\/]/).pop()?.replace(/\.exe$/i, "") ?? "";
    }
  }
}

async function chooseWorkingDir() {
  const selected = await open({ multiple: false, directory: true });
  if (typeof selected === "string") {
    ruleDraft.working_dir = selected;
  }
}

async function saveSubscription() {
  await runAction("subscription", async () => {
    await invoke("set_subscription", { url: subscriptionDraft.value.trim() || null });
  });
}

async function refreshNodes() {
  await runAction("refresh", async () => {
    await invoke("refresh_nodes");
  });
}

async function restartCore() {
  await runAction("restart", async () => {
    await invoke("restart_core");
  });
}

async function startRule(rule: AppRule) {
  await runAction(`start-${rule.id}`, async () => {
    await invoke("start_rule_app", { ruleId: rule.id });
  });
}

async function stopRule(rule: AppRule) {
  await runAction(`stop-${rule.id}`, async () => {
    await invoke("stop_rule_app", { ruleId: rule.id });
  });
}

async function deleteRule(rule: AppRule) {
  await runAction(`delete-${rule.id}`, async () => {
    await invoke("delete_rule", { ruleId: rule.id });
  });
}

async function setRuleNode(rule: AppRule, event: Event) {
  const node = (event.target as HTMLSelectElement).value;
  await runAction(`node-${rule.id}`, async () => {
    await invoke("set_rule_node", { ruleId: rule.id, node });
  });
}

async function testNode(rule: AppRule) {
  const node = rule.selected_node ?? "DIRECT";
  await runAction(`delay-${rule.id}`, async () => {
    await invoke("test_node_delay", { node });
  });
}

function formatBytes(value: number) {
  if (!Number.isFinite(value)) return "0 B";
  const units = ["B", "KB", "MB", "GB", "TB"];
  let size = value;
  let unit = 0;
  while (size >= 1024 && unit < units.length - 1) {
    size /= 1024;
    unit += 1;
  }
  return `${size >= 10 || unit === 0 ? size.toFixed(0) : size.toFixed(1)} ${units[unit]}`;
}

function formatSpeed(value: number) {
  return `${formatBytes(value)}/s`;
}

function nodeLabel(nodeName: string | null) {
  return nodeName || "DIRECT";
}

function statFor(rule: AppRule) {
  return statsByRule.value.get(rule.id);
}

onMounted(async () => {
  await loadState();
  timer = window.setInterval(() => loadState(true), 1200);
});

onBeforeUnmount(() => {
  if (timer) window.clearInterval(timer);
});
</script>

<template>
  <main class="app-shell">
    <aside class="sidebar">
      <div class="brand">
        <div class="brand-mark">
          <Zap :size="22" />
        </div>
        <div>
          <h1>FreeClash</h1>
          <p>应用级代理启动器</p>
        </div>
      </div>

      <section class="status-panel">
        <div class="status-row">
          <span>核心</span>
          <strong :class="status?.core_running ? 'ok' : 'warn'">{{ status?.core_running ? "运行中" : "未运行" }}</strong>
        </div>
        <div class="status-row">
          <span>版本</span>
          <strong>{{ status?.core_version || "未知" }}</strong>
        </div>
        <div class="status-row">
          <span>规则</span>
          <strong>{{ rules.length }}</strong>
        </div>
      </section>

      <section class="subscription">
        <label for="subscription-url">订阅地址</label>
        <textarea id="subscription-url" v-model="subscriptionDraft" rows="5" spellcheck="false" placeholder="https://example.com/sub"></textarea>
        <div class="button-row">
          <button class="primary" type="button" :disabled="busy === 'subscription'" @click="saveSubscription">
            <Save :size="16" />
            保存
          </button>
          <button type="button" :disabled="busy === 'refresh'" @click="refreshNodes">
            <RefreshCw :size="16" />
            刷新
          </button>
        </div>
      </section>

      <section class="paths">
        <div>
          <span>Core</span>
          <p>{{ status?.core_path || "-" }}</p>
        </div>
        <div>
          <span>Config</span>
          <p>{{ status?.config_path || "-" }}</p>
        </div>
      </section>
    </aside>

    <section class="workspace">
      <header class="toolbar">
        <div>
          <h2>软件代理规则</h2>
          <p>从这里启动的软件会继承该规则的 HTTP/HTTPS 代理。</p>
        </div>
        <div class="toolbar-actions">
          <button type="button" :disabled="busy === 'restart'" @click="restartCore" title="重启 mihomo 核心">
            <RotateCcw :size="17" />
            重启核心
          </button>
          <button class="primary" type="button" @click="openCreateRule">
            <Plus :size="17" />
            新增规则
          </button>
        </div>
      </header>

      <div v-if="error" class="notice error">
        <X :size="18" />
        <span>{{ error }}</span>
        <button type="button" @click="error = null" title="关闭">
          <X :size="16" />
        </button>
      </div>

      <div v-if="status?.message" class="notice">
        <Activity :size="18" />
        <span>{{ status.message }}</span>
      </div>

      <section class="metrics-strip">
        <div class="metric">
          <Wifi :size="18" />
          <div>
            <span>节点</span>
            <strong>{{ nodes.length }}</strong>
          </div>
        </div>
        <div class="metric">
          <Upload :size="18" />
          <div>
            <span>总上传</span>
            <strong>{{ formatBytes((snapshot?.stats ?? []).reduce((sum, item) => sum + item.upload_total, 0)) }}</strong>
          </div>
        </div>
        <div class="metric">
          <Download :size="18" />
          <div>
            <span>总下载</span>
            <strong>{{ formatBytes((snapshot?.stats ?? []).reduce((sum, item) => sum + item.download_total, 0)) }}</strong>
          </div>
        </div>
        <div class="metric">
          <Radio :size="18" />
          <div>
            <span>活动连接</span>
            <strong>{{ (snapshot?.stats ?? []).reduce((sum, item) => sum + item.active_connections, 0) }}</strong>
          </div>
        </div>
      </section>

      <section v-if="editorOpen" class="editor">
        <div class="editor-title">
          <h3>{{ editingId ? "编辑规则" : "新增规则" }}</h3>
          <button type="button" @click="closeEditor" title="关闭编辑器">
            <X :size="17" />
          </button>
        </div>
        <div class="form-grid">
          <label>
            <span>规则名</span>
            <input v-model="ruleDraft.name" type="text" placeholder="Codex 香港" />
          </label>
          <label>
            <span>节点</span>
            <select v-model="ruleDraft.selected_node">
              <option v-for="node in selectableNodes" :key="node.name" :value="node.name">
                {{ node.name }}{{ node.delay ? ` · ${node.delay}ms` : "" }}
              </option>
            </select>
          </label>
          <label class="wide">
            <span>软件路径</span>
            <div class="input-action">
              <input v-model="ruleDraft.app_path" type="text" placeholder="C:\Program Files\app\app.exe" />
              <button type="button" @click="chooseExe" title="选择 exe">
                <FolderOpen :size="17" />
              </button>
            </div>
          </label>
          <label class="wide">
            <span>启动参数</span>
            <input v-model="ruleDraft.args" type="text" placeholder="可选，例如 --profile work" />
          </label>
          <label class="wide">
            <span>工作目录</span>
            <div class="input-action">
              <input v-model="ruleDraft.working_dir" type="text" placeholder="可选，默认使用软件所在目录" />
              <button type="button" @click="chooseWorkingDir" title="选择工作目录">
                <FolderOpen :size="17" />
              </button>
            </div>
          </label>
          <label class="toggle">
            <input v-model="ruleDraft.enabled" type="checkbox" />
            <span>启用规则</span>
          </label>
        </div>
        <div class="button-row right">
          <button type="button" @click="closeEditor">取消</button>
          <button class="primary" type="button" :disabled="busy === 'save-rule'" @click="saveRule">
            <Save :size="16" />
            保存规则
          </button>
        </div>
      </section>

      <section class="rules">
        <div v-if="loading" class="empty">
          <Gauge :size="28" />
          <span>正在载入运行状态</span>
        </div>
        <div v-else-if="rules.length === 0" class="empty">
          <Server :size="30" />
          <strong>还没有软件规则</strong>
          <span>新增规则后，从 FreeClash 启动的软件会自动使用对应节点。</span>
          <button class="primary" type="button" @click="openCreateRule">
            <Plus :size="17" />
            新增第一条规则
          </button>
        </div>

        <article v-for="rule in rules" :key="rule.id" class="rule-card">
          <div class="rule-head">
            <div>
              <h3>{{ rule.name }}</h3>
              <p>{{ rule.app_path }}</p>
            </div>
            <div class="rule-actions">
              <button type="button" :disabled="busy === `start-${rule.id}` || !rule.enabled" @click="startRule(rule)" title="启动软件">
                <Play :size="16" />
              </button>
              <button type="button" :disabled="busy === `stop-${rule.id}`" @click="stopRule(rule)" title="停止软件">
                <Square :size="16" />
              </button>
              <button type="button" @click="openEditRule(rule)" title="编辑规则">
                <Edit3 :size="16" />
              </button>
              <button type="button" :disabled="busy === `delete-${rule.id}`" @click="deleteRule(rule)" title="删除规则">
                <Trash2 :size="16" />
              </button>
            </div>
          </div>

          <div class="rule-config">
            <label>
              <span>当前节点</span>
              <select :value="nodeLabel(rule.selected_node)" @change="setRuleNode(rule, $event)">
                <option v-for="node in selectableNodes" :key="node.name" :value="node.name">
                  {{ node.name }}{{ node.delay ? ` · ${node.delay}ms` : "" }}
                </option>
              </select>
            </label>
            <button type="button" :disabled="busy === `delay-${rule.id}`" @click="testNode(rule)" title="测试延迟">
              <Gauge :size="16" />
              测速
            </button>
            <div class="ports">
              <span>本地 {{ rule.meter_port }}</span>
              <span>上游 {{ rule.mihomo_port }}</span>
            </div>
          </div>

          <div class="stats-grid">
            <div>
              <span>上传速度</span>
              <strong>{{ formatSpeed(statFor(rule)?.upload_speed ?? 0) }}</strong>
            </div>
            <div>
              <span>下载速度</span>
              <strong>{{ formatSpeed(statFor(rule)?.download_speed ?? 0) }}</strong>
            </div>
            <div>
              <span>上传流量</span>
              <strong>{{ formatBytes(statFor(rule)?.upload_total ?? 0) }}</strong>
            </div>
            <div>
              <span>下载流量</span>
              <strong>{{ formatBytes(statFor(rule)?.download_total ?? 0) }}</strong>
            </div>
          </div>

          <div class="targets">
            <div class="targets-title">
              <span>最近访问目标</span>
              <strong>{{ statFor(rule)?.active_connections ?? 0 }} 活动</strong>
            </div>
            <ul v-if="(statFor(rule)?.recent_targets ?? []).length > 0">
              <li v-for="conn in (statFor(rule)?.recent_targets ?? []).slice(0, 5)" :key="conn.id">
                <span class="target">{{ conn.target }}</span>
                <span>{{ conn.method }}</span>
                <span>{{ formatBytes(conn.upload) }} ↑</span>
                <span>{{ formatBytes(conn.download) }} ↓</span>
              </li>
            </ul>
            <p v-else>暂无连接</p>
          </div>
        </article>
      </section>
    </section>
  </main>
</template>
