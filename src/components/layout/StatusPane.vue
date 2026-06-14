<script setup lang="ts">
import { computed } from "vue";
import { Activity, Gauge, Network, RotateCcw } from "@lucide/vue";
import type { AppSnapshot } from "../../types";
import { formatBytes, formatSpeed } from "../../utils/format";

const props = defineProps<{
  snapshot: AppSnapshot | null;
  busy: string | null;
}>();

const emit = defineEmits<{
  toggleGlobal: [enabled: boolean];
  restartCore: [];
}>();

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

function onToggle(event: Event) {
  emit("toggleGlobal", (event.target as HTMLInputElement).checked);
}
</script>

<template>
  <section class="status-pane" aria-label="运行概览">
    <div class="status-grid">
      <div class="metric-card core-metric">
        <div class="core-metric-main">
          <span>核心</span>
          <strong :class="snapshot?.status.core_running ? 'ok' : 'warn'">
            {{ snapshot?.status.core_running ? "运行中" : "未运行" }}
          </strong>
        </div>
        <div class="core-metric-actions">
          <label class="switch" title="切换全局代理链路">
            <input
              type="checkbox"
              :checked="snapshot?.config.global_proxy_enabled ?? true"
              :disabled="busy === 'global-proxy'"
              @change="onToggle"
            />
            <span></span>
          </label>
          <button
            type="button"
            title="重启 mihomo 核心"
            :disabled="busy === 'restart'"
            @click="emit('restartCore')"
          >
            <RotateCcw :size="15" />
          </button>
        </div>
      </div>
      <div class="metric-card">
        <span>通道</span>
        <strong>{{ enabledChannels }} / {{ channels.length }}</strong>
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

  </section>
</template>
