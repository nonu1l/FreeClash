<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, reactive, ref } from "vue";
import {
  ArrowDown,
  ArrowUp,
  Database,
  Gauge,
  Pin,
  PinOff,
  RefreshCw,
  Search,
  Settings,
  X,
} from "@lucide/vue";
import { freeClashApi } from "./api";
import type { AppSnapshot, DelayResult, NodeInfo, PinRuntime, SubscriptionInput } from "./types";

const snapshot = ref<AppSnapshot | null>(null);
const error = ref("");
const busy = ref("");
const showPinnedOnly = ref(false);
const showSubscriptionDialog = ref(false);
const showPortDialog = ref(false);
const editingNodeName = ref("");
const portDraft = ref(0);
const delayOverrides = ref<Record<string, number>>({});
const subscriptionDraft = reactive<SubscriptionInput>({
  name: "",
  url: "",
});

let pollTimer: number | undefined;

const pinsByNode = computed(() => {
  const map = new Map<string, PinRuntime>();
  for (const pin of snapshot.value?.pins ?? []) {
    map.set(pin.node_name, pin);
  }
  return map;
});

const nodes = computed(() => {
  const source = snapshot.value?.nodes ?? [];
  if (!showPinnedOnly.value) return source;
  return source.filter((node) => pinsByNode.value.has(node.name));
});

const totals = computed(() => {
  let uploadSpeed = 0;
  let downloadSpeed = 0;
  let traffic = 0;
  for (const pin of snapshot.value?.pins ?? []) {
    uploadSpeed += pin.stats.upload_speed;
    downloadSpeed += pin.stats.download_speed;
    traffic += pin.stats.upload_total + pin.stats.download_total;
  }
  return { uploadSpeed, downloadSpeed, traffic };
});

const subscriptionLabel = computed(() => snapshot.value?.config.subscription?.name || "订阅设置");

async function refreshState() {
  try {
    snapshot.value = await freeClashApi.getState();
  } catch (err) {
    error.value = stringifyError(err);
  }
}

async function runAction(name: string, action: () => Promise<unknown>, refresh = true) {
  busy.value = name;
  error.value = "";
  try {
    await action();
    if (refresh) await refreshState();
  } catch (err) {
    error.value = stringifyError(err);
  } finally {
    busy.value = "";
  }
}

function openSubscriptionDialog() {
  const subscription = snapshot.value?.config.subscription;
  subscriptionDraft.name = subscription?.name ?? "";
  subscriptionDraft.url = subscription?.url ?? "";
  showSubscriptionDialog.value = true;
}

async function saveSubscription() {
  await runAction("subscription", () => freeClashApi.setSubscription({ ...subscriptionDraft }));
  showSubscriptionDialog.value = false;
}

async function refreshSubscription() {
  await runAction("refresh-subscription", () => freeClashApi.refreshSubscription());
}

async function togglePin(node: NodeInfo) {
  const pinned = pinsByNode.value.has(node.name);
  await runAction(
    `pin-${node.name}`,
    () => (pinned ? freeClashApi.unpinNode(node.name) : freeClashApi.pinNode(node.name)),
  );
}

function openPortDialog(nodeName: string) {
  const pin = pinsByNode.value.get(nodeName);
  if (!pin) return;
  editingNodeName.value = nodeName;
  portDraft.value = pin.port;
  showPortDialog.value = true;
}

async function savePort() {
  await runAction("port", () => freeClashApi.updatePinPort(editingNodeName.value, portDraft.value));
  showPortDialog.value = false;
}

async function testDelay(nodeName: string) {
  await runAction(
    `delay-${nodeName}`,
    async () => {
      const result = await freeClashApi.testNodeDelay(nodeName);
      delayOverrides.value = { ...delayOverrides.value, [result.node]: result.delay };
    },
    false,
  );
  await refreshState();
}

async function testAllDelays() {
  await runAction(
    "delay-all",
    async () => {
      const results = await freeClashApi.testAllNodeDelays();
      const next = { ...delayOverrides.value };
      for (const result of results) next[result.node] = result.delay;
      delayOverrides.value = next;
    },
    false,
  );
  await refreshState();
}

function delayFor(node: NodeInfo) {
  return delayOverrides.value[node.name] ?? node.delay ?? null;
}

function stringifyError(err: unknown) {
  if (err instanceof Error) return err.message;
  return String(err);
}

function formatBytes(bytes: number) {
  if (bytes < 1024) return `${Math.round(bytes)} B`;
  if (bytes < 1024 ** 2) return `${(bytes / 1024).toFixed(1)} KB`;
  if (bytes < 1024 ** 3) return `${(bytes / 1024 ** 2).toFixed(1)} MB`;
  return `${(bytes / 1024 ** 3).toFixed(2)} GB`;
}

function formatSpeed(bytes: number) {
  return `${formatBytes(bytes)}/s`;
}

onMounted(() => {
  void refreshState();
  pollTimer = window.setInterval(refreshState, 1200);
});

onBeforeUnmount(() => {
  if (pollTimer) window.clearInterval(pollTimer);
});
</script>

<template>
  <main class="single-page">
    <section class="node-grid" aria-label="节点列表">
      <article
        v-for="node in nodes"
        :key="node.name"
        class="node-card"
        :class="{
          pinned: pinsByNode.has(node.name),
          warning: pinsByNode.get(node.name)?.port_available === false,
        }"
      >
        <div class="node-card-top">
          <h2 :title="node.name">{{ node.name }}</h2>
          <div class="node-metrics">
            <span title="上行速度"><ArrowUp :size="15" />{{ formatSpeed(pinsByNode.get(node.name)?.stats.upload_speed ?? 0) }}</span>
            <span title="下行速度"><ArrowDown :size="15" />{{ formatSpeed(pinsByNode.get(node.name)?.stats.download_speed ?? 0) }}</span>
            <span title="总流量"><Database :size="15" />{{ formatBytes((pinsByNode.get(node.name)?.stats.upload_total ?? 0) + (pinsByNode.get(node.name)?.stats.download_total ?? 0)) }}</span>
            <button
              type="button"
              class="icon-pin"
              :class="{ active: pinsByNode.has(node.name) }"
              :title="pinsByNode.has(node.name) ? '取消 Pin 并关闭端口' : 'Pin 节点并生成端口'"
              :disabled="busy === `pin-${node.name}`"
              @click="togglePin(node)"
            >
              <PinOff v-if="pinsByNode.has(node.name)" :size="17" />
              <Pin v-else :size="17" />
            </button>
          </div>
        </div>

        <div class="node-card-bottom">
          <button
            type="button"
            class="port-pill"
            :class="{ editable: pinsByNode.has(node.name) }"
            :disabled="!pinsByNode.has(node.name)"
            :title="pinsByNode.has(node.name) ? '修改本地代理端口' : 'Pin 后自动分配端口'"
            @click="openPortDialog(node.name)"
          >
            <span>HTTP</span>
            <span>SOCKS5</span>
            <strong>{{ pinsByNode.get(node.name)?.port ?? "未分配" }}</strong>
          </button>
          <button
            type="button"
            class="delay-button"
            :disabled="busy === `delay-${node.name}`"
            title="重新测试此节点延迟"
            @click="testDelay(node.name)"
          >
            <Gauge :size="15" />
            <span>{{ delayFor(node) === null ? "延迟" : `${delayFor(node)} ms` }}</span>
          </button>
        </div>

        <p v-if="pinsByNode.get(node.name)?.port_error" class="port-error">
          {{ pinsByNode.get(node.name)?.port_error }}
        </p>
      </article>

      <div v-if="nodes.length === 0" class="empty-state">
        <Search :size="28" />
        <strong>{{ showPinnedOnly ? "暂无 Pin 节点" : "暂无节点" }}</strong>
        <span>{{ snapshot?.config.subscription ? "刷新订阅后再看看。" : "先在底部设置订阅。" }}</span>
      </div>
    </section>

    <p v-if="error" class="error-toast">
      <X :size="16" />
      {{ error }}
      <button type="button" title="关闭" @click="error = ''"><X :size="15" /></button>
    </p>

    <footer class="bottom-toolbar">
      <div class="toolbar-metrics">
        <span title="总流量"><Database :size="16" />{{ formatBytes(totals.traffic) }}</span>
        <span title="总上行速度"><ArrowUp :size="16" />{{ formatSpeed(totals.uploadSpeed) }}</span>
        <span title="总下行速度"><ArrowDown :size="16" />{{ formatSpeed(totals.downloadSpeed) }}</span>
      </div>
      <div class="toolbar-actions">
        <button type="button" :disabled="busy === 'delay-all'" @click="testAllDelays">
          <Gauge :size="16" />
          延迟计算
        </button>
        <button type="button" :disabled="busy === 'refresh-subscription'" @click="refreshSubscription">
          <RefreshCw :size="16" />
          刷新订阅
        </button>
        <button type="button" @click="openSubscriptionDialog">
          <Settings :size="16" />
          {{ subscriptionLabel }}
        </button>
        <button type="button" :class="{ active: showPinnedOnly }" @click="showPinnedOnly = !showPinnedOnly">
          <Pin :size="16" />
          只看 Pin
        </button>
      </div>
    </footer>

    <div v-if="showSubscriptionDialog" class="modal-backdrop" @click.self="showSubscriptionDialog = false">
      <section class="modal compact-modal">
        <header>
          <h3>订阅设置</h3>
          <button type="button" title="关闭" @click="showSubscriptionDialog = false"><X :size="17" /></button>
        </header>
        <label>
          <span>名称</span>
          <input v-model.trim="subscriptionDraft.name" placeholder="可留空，自动使用域名" />
        </label>
        <label>
          <span>URL</span>
          <input v-model.trim="subscriptionDraft.url" placeholder="https://example.com/subscribe" />
        </label>
        <footer>
          <button type="button" @click="showSubscriptionDialog = false">取消</button>
          <button type="button" class="primary" :disabled="busy === 'subscription'" @click="saveSubscription">
            保存
          </button>
        </footer>
      </section>
    </div>

    <div v-if="showPortDialog" class="modal-backdrop" @click.self="showPortDialog = false">
      <section class="modal compact-modal">
        <header>
          <h3>修改端口</h3>
          <button type="button" title="关闭" @click="showPortDialog = false"><X :size="17" /></button>
        </header>
        <label>
          <span>{{ editingNodeName }}</span>
          <input v-model.number="portDraft" type="number" min="1024" max="65535" />
        </label>
        <footer>
          <button type="button" @click="showPortDialog = false">取消</button>
          <button type="button" class="primary" :disabled="busy === 'port'" @click="savePort">保存</button>
        </footer>
      </section>
    </div>
  </main>
</template>
