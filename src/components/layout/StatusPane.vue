<script setup lang="ts">
import { computed } from "vue";
import { Activity, Gauge, Network } from "@lucide/vue";
import type { AppSnapshot } from "../../types";
import { formatBytes, formatSpeed } from "../../utils/format";

const props = defineProps<{
  snapshot: AppSnapshot | null;
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
