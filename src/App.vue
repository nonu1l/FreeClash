<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { Activity, Copy, X } from "@lucide/vue";
import { freeClashApi } from "./api";
import AppShell from "./components/layout/AppShell.vue";
import ConfirmDialog from "./components/common/ConfirmDialog.vue";
import ToastHost from "./components/common/ToastHost.vue";
import RulesView from "./views/RulesView.vue";
import SubscriptionsView from "./views/SubscriptionsView.vue";
import { copyTextToClipboard } from "./utils/clipboard";
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
const settingsHttpPort = ref(19290);
const toasts = ref<Array<{ id: number; message: string; tone: "success" | "error" | "info" }>>([]);
const confirmState = ref<{
  open: boolean;
  title: string;
  message: string;
  confirmText: string;
  danger: boolean;
  resolve: ((value: boolean) => void) | null;
}>({
  open: false,
  title: "",
  message: "",
  confirmText: "确认",
  danger: false,
  resolve: null,
});

let timer: number | undefined;
let toastId = 0;

const subscriptions = computed(() => snapshot.value?.config.subscriptions ?? []);
const channels = computed(() => snapshot.value?.config.channels ?? []);
const nodes = computed(() => snapshot.value?.nodes ?? []);
const stats = computed(() => snapshot.value?.stats ?? []);
const visibleStatusMessage = computed(() => {
  const message = snapshot.value?.status.message?.trim();
  if (!message || message === "mihomo 核心已就绪") return null;
  return message;
});

function setError(value: unknown) {
  error.value = value instanceof Error ? value.message : String(value);
}

function notify(message: string, tone: "success" | "error" | "info" = "success") {
  const id = ++toastId;
  toasts.value = [...toasts.value, { id, message, tone }];
  window.setTimeout(() => dismissToast(id), 2400);
}

function dismissToast(id: number) {
  toasts.value = toasts.value.filter((toast) => toast.id !== id);
}

function confirmAction(
  title: string,
  message: string,
  options: { confirmText?: string; danger?: boolean } = {},
) {
  return new Promise<boolean>((resolve) => {
    confirmState.value = {
      open: true,
      title,
      message,
      confirmText: options.confirmText ?? "确认",
      danger: options.danger ?? false,
      resolve,
    };
  });
}

function closeConfirm(result: boolean) {
  const resolve = confirmState.value.resolve;
  confirmState.value.open = false;
  confirmState.value.resolve = null;
  resolve?.(result);
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

function toggleHttpApi(event: Event) {
  const enabled = (event.target as HTMLInputElement).checked;
  setHttpApiConfig(enabled, settingsHttpPort.value);
}

function applyHttpApiSettings() {
  setHttpApiConfig(snapshot.value?.config.http_api_enabled ?? false, settingsHttpPort.value);
}

async function copyHttpToken() {
  const token = snapshot.value?.config.http_api_token;
  if (!token) return;
  const copied = await copyTextToClipboard(token);
  localStorage.setItem("freeclashApiToken", token);
  localStorage.setItem("freeclashApiBaseUrl", `http://127.0.0.1:${settingsHttpPort.value}`);
  notify(copied ? "已复制 HTTP API token，并写入浏览器调试配置" : "已写入浏览器调试配置，复制 token 失败", copied ? "success" : "error");
}

watch(
  () => snapshot.value?.config.http_api_port,
  (port) => {
    settingsHttpPort.value = port ?? 19290;
  },
  { immediate: true },
);

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
    @restart-core="restartCore"
  >
      <div v-if="error" class="notice error">
        <X :size="18" />
        <span>{{ error }}</span>
        <button type="button" title="关闭" @click="error = null">
          <X :size="16" />
        </button>
      </div>

      <div v-if="visibleStatusMessage" class="notice">
        <Activity :size="18" />
        <span>{{ visibleStatusMessage }}</span>
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
        :confirm-action="confirmAction"
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
            <p>本机调试入口默认关闭，仅监听 127.0.0.1。</p>
          </div>
        </header>
        <section class="settings-panel">
          <div class="settings-row">
            <div>
              <strong>HTTP API 调试接口</strong>
              <span>供浏览器、curl 和自动化测试调用 Tauri 同名命令。</span>
            </div>
            <label class="switch" title="切换 HTTP API">
              <input
                type="checkbox"
                :checked="snapshot?.config.http_api_enabled ?? false"
                :disabled="busy === 'http-api'"
                @change="toggleHttpApi"
              />
              <span></span>
            </label>
          </div>

          <div class="settings-row align-end">
            <label class="field-label">
              <span>监听端口</span>
              <input v-model.number="settingsHttpPort" type="number" min="1" max="65535" />
            </label>
            <button type="button" class="button secondary" :disabled="busy === 'http-api'" @click="applyHttpApiSettings">
              应用端口
            </button>
          </div>

          <div class="settings-row">
            <div>
              <strong>访问地址</strong>
              <span>http://127.0.0.1:{{ settingsHttpPort }}/api/health</span>
            </div>
            <button type="button" class="button secondary" title="复制 HTTP API token" @click="copyHttpToken">
              <Copy :size="16" />
              复制 token
            </button>
          </div>
        </section>
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
        :notify="notify"
        :confirm-action="confirmAction"
      />

      <ConfirmDialog
        :open="confirmState.open"
        :title="confirmState.title"
        :message="confirmState.message"
        :confirm-text="confirmState.confirmText"
        :danger="confirmState.danger"
        @close="closeConfirm(false)"
        @confirm="closeConfirm(true)"
      />

      <ToastHost :toasts="toasts" @dismiss="dismissToast" />
  </AppShell>
</template>
