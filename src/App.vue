<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { Activity, X } from "@lucide/vue";
import { freeClashApi } from "./api";
import AppShell from "./components/layout/AppShell.vue";
import RulesView from "./views/RulesView.vue";
import SubscriptionsView from "./views/SubscriptionsView.vue";
import type {
  ActiveView,
  AppSnapshot,
  ChannelDiagnostics,
  ChannelInput,
  ChannelProxyTestResult,
  SubscriptionInput,
} from "./types";

const snapshot = ref<AppSnapshot | null>(null);
const activeView = ref<ActiveView>("channels");
const loading = ref(true);
const busy = ref<string | null>(null);
const error = ref<string | null>(null);

let timer: number | undefined;

const subscriptions = computed(() => snapshot.value?.config.subscriptions ?? []);
const channels = computed(() => snapshot.value?.config.channels ?? []);
const nodes = computed(() => snapshot.value?.nodes ?? []);
const stats = computed(() => snapshot.value?.stats ?? []);

function setError(value: unknown) {
  error.value = value instanceof Error ? value.message : String(value);
}

async function loadState(quiet = false) {
  try {
    if (!quiet) loading.value = true;
    snapshot.value = await freeClashApi.getState();
    error.value = null;
  } catch (err) {
    setError(err);
  } finally {
    loading.value = false;
  }
}

async function runAction<T>(name: string, action: () => Promise<T>, reload = true): Promise<T> {
  busy.value = name;
  try {
    const result = await action();
    if (reload) await loadState(true);
    return result;
  } catch (err) {
    setError(err);
    throw err;
  } finally {
    busy.value = null;
  }
}

function setGlobalProxyEnabled(enabled: boolean) {
  void runAction("global-proxy", () => freeClashApi.setGlobalProxyEnabled(enabled));
}

function setHttpApiConfig(enabled: boolean, port: number) {
  void runAction("http-api", () => freeClashApi.setHttpApiConfig(enabled, port));
}

function restartCore() {
  void runAction("restart", () => freeClashApi.restartCore());
}

async function createSubscription(input: SubscriptionInput) {
  await runAction("save-subscription", () => freeClashApi.createSubscription(input));
}

async function updateSubscription(id: string, input: SubscriptionInput) {
  await runAction("save-subscription", () => freeClashApi.updateSubscription(id, input));
}

async function deleteSubscription(id: string) {
  await runAction(`delete-subscription-${id}`, () => freeClashApi.deleteSubscription(id));
}

async function refreshSubscription(id: string) {
  await runAction(`refresh-subscription-${id}`, () => freeClashApi.refreshSubscription(id));
}

async function refreshNodes() {
  await runAction("refresh-nodes", () => freeClashApi.refreshNodes());
}

async function createChannel(input: ChannelInput) {
  await runAction("save-channel", () => freeClashApi.createChannel(input));
}

async function updateChannel(id: string, input: ChannelInput) {
  await runAction("save-channel", () => freeClashApi.updateChannel(id, input));
}

async function deleteChannel(id: string) {
  await runAction(`delete-${id}`, () => freeClashApi.deleteChannel(id));
}

async function duplicateChannel(id: string) {
  await runAction(`duplicate-${id}`, () => freeClashApi.duplicateChannel(id));
}

async function setChannelEnabled(id: string, enabled: boolean) {
  await runAction(`channel-enabled-${id}`, () => freeClashApi.setChannelEnabled(id, enabled));
}

async function diagnoseChannel(id: string) {
  return await runAction<ChannelDiagnostics>(
    `diagnose-${id}`,
    () => freeClashApi.diagnoseChannel(id),
    false,
  );
}

async function testChannelProxy(id: string) {
  return await runAction<ChannelProxyTestResult>(
    `test-channel-${id}`,
    () => freeClashApi.testChannelProxy(id),
    true,
  );
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
  <AppShell
    :snapshot="snapshot"
    :active-view="activeView"
    :busy="busy"
    @change-view="activeView = $event"
    @toggle-global="setGlobalProxyEnabled"
    @set-http-api-config="setHttpApiConfig"
    @restart-core="restartCore"
  >
      <div v-if="error" class="notice error">
        <X :size="18" />
        <span>{{ error }}</span>
        <button type="button" title="关闭" @click="error = null">
          <X :size="16" />
        </button>
      </div>

      <div v-if="snapshot?.status.message" class="notice">
        <Activity :size="18" />
        <span>{{ snapshot.status.message }}</span>
      </div>

      <div v-if="loading" class="empty loading-state">
        <Activity :size="28" />
        <span>正在载入运行状态</span>
      </div>

      <SubscriptionsView
        v-else-if="activeView === 'subscriptions'"
        :subscriptions="subscriptions"
        :nodes="nodes"
        :busy="busy"
        :create-subscription="createSubscription"
        :update-subscription="updateSubscription"
        :delete-subscription="deleteSubscription"
        :refresh-subscription="refreshSubscription"
        :refresh-nodes="refreshNodes"
      />

      <section v-else-if="activeView === 'connections'" class="view">
        <header class="view-header">
          <div>
            <h2>连接记录</h2>
            <p>最近目标和活动连接会在后续版本集中到这里。</p>
          </div>
        </header>
        <div class="empty compact-empty">
          <Activity :size="28" />
          <strong>连接记录视图已预留</strong>
          <span>当前可在通道诊断中查看最近访问目标。</span>
        </div>
      </section>

      <section v-else-if="activeView === 'settings'" class="view">
        <header class="view-header">
          <div>
            <h2>设置</h2>
            <p>HTTP API 调试区已放在左侧状态面板。</p>
          </div>
        </header>
        <div class="empty compact-empty">
          <Activity :size="28" />
          <strong>高级设置视图已预留</strong>
          <span>Core 路径、测试 URL 和端口范围后续会移动到这里。</span>
        </div>
      </section>

      <RulesView
        v-else
        :channels="channels"
        :nodes="nodes"
        :stats="stats"
        :busy="busy"
        :create-channel="createChannel"
        :update-channel="updateChannel"
        :delete-channel="deleteChannel"
        :duplicate-channel="duplicateChannel"
        :set-channel-enabled="setChannelEnabled"
        :diagnose-channel="diagnoseChannel"
        :test-channel-proxy="testChannelProxy"
      />
  </AppShell>
</template>
