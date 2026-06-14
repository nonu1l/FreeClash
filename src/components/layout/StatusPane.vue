<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { Activity, Copy, Gauge, Network } from "@lucide/vue";
import type { AppSnapshot } from "../../types";
import { formatBytes, formatSpeed } from "../../utils/format";

const props = defineProps<{
  snapshot: AppSnapshot | null;
  busy: string | null;
}>();

const emit = defineEmits<{
  setHttpApiConfig: [payload: { enabled: boolean; port: number }];
}>();

const httpPort = ref(19290);
const apiExpanded = ref(false);

const channels = computed(() => props.snapshot?.config.channels ?? []);
const enabledChannels = computed(() => channels.value.filter((channel) => channel.enabled).length);
const totalUploadSpeed = computed(() =>
  (props.snapshot?.stats ?? []).reduce((sum, item) => sum + item.upload_speed, 0),
);
const totalDownloadSpeed = computed(() =>
  (props.snapshot?.stats ?? []).reduce((sum, item) => sum + item.download_speed, 0),
);
const totalTraffic = computed(() =>
  (props.snapshot?.stats ?? []).reduce(
    (sum, item) => sum + item.upload_total + item.download_total,
    0,
  ),
);

watch(
  () => props.snapshot?.config.http_api_port,
  (port) => {
    httpPort.value = port ?? 19290;
  },
  { immediate: true },
);

function onHttpToggle(event: Event) {
  emit("setHttpApiConfig", {
    enabled: (event.target as HTMLInputElement).checked,
    port: httpPort.value,
  });
}

function applyHttpPort() {
  emit("setHttpApiConfig", {
    enabled: props.snapshot?.config.http_api_enabled ?? false,
    port: httpPort.value,
  });
}

async function copyHttpToken() {
  const token = props.snapshot?.config.http_api_token;
  if (!token) return;
  await navigator.clipboard.writeText(token);
  localStorage.setItem("freeclashApiToken", token);
  localStorage.setItem("freeclashApiBaseUrl", `http://127.0.0.1:${httpPort.value}`);
}
</script>

<template>
  <section class="status-pane" aria-label="运行状态">
    <span class="nav-section">状态</span>

    <div class="status-grid">
      <div class="metric-card">
        <span>核心</span>
        <strong :class="snapshot?.status.core_running ? 'ok' : 'warn'">
          {{ snapshot?.status.core_running ? "运行中" : "未运行" }}
        </strong>
      </div>
      <div class="metric-card">
        <span>通道</span>
        <strong>{{ enabledChannels }} / {{ channels.length }}</strong>
      </div>
      <div class="metric-card wide">
        <span>当前速度</span>
        <strong>{{ formatSpeed(totalUploadSpeed + totalDownloadSpeed) }}</strong>
      </div>
      <div class="metric-line">
        <Activity :size="16" />
        <span>上传</span>
        <strong>{{ formatSpeed(totalUploadSpeed) }}</strong>
      </div>
      <div class="metric-line">
        <Gauge :size="16" />
        <span>下载</span>
        <strong>{{ formatSpeed(totalDownloadSpeed) }}</strong>
      </div>
      <div class="metric-line">
        <Network :size="16" />
        <span>本次流量</span>
        <strong>{{ formatBytes(totalTraffic) }}</strong>
      </div>
    </div>

    <section class="http-api-box">
      <button class="http-api-head" type="button" @click="apiExpanded = !apiExpanded">
        <span>HTTP API</span>
        <strong>{{ snapshot?.config.http_api_enabled ? "已开启" : "已关闭" }}</strong>
      </button>
      <div class="http-api-toggle">
        <label class="switch" title="切换 HTTP API">
          <input
            type="checkbox"
            :checked="snapshot?.config.http_api_enabled ?? false"
            :disabled="busy === 'http-api'"
            @change="onHttpToggle"
          />
          <span></span>
        </label>
        <input v-model.number="httpPort" type="number" min="1" max="65535" />
        <button type="button" class="button secondary" :disabled="busy === 'http-api'" @click="applyHttpPort">
          应用
        </button>
      </div>
      <div v-if="apiExpanded" class="http-api-details">
        <button type="button" class="button secondary full-button" title="复制 HTTP API token" @click="copyHttpToken">
          <Copy :size="16" />
          复制 token
        </button>
      </div>
    </section>
  </section>
</template>
