<script setup lang="ts">
import { computed } from "vue";
import { ArrowDown, ArrowUp, Link2, ListChecks, RotateCcw, Settings } from "@lucide/vue";
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
const globalProxyEnabled = computed(() => props.snapshot?.config.global_proxy_enabled ?? true);
const coreRunning = computed(() => props.snapshot?.status.core_running ?? false);

function toggleGlobal() {
  if (props.busy === "global-proxy") return;
  emit("toggleGlobal", !globalProxyEnabled.value);
}
</script>

<template>
  <section class="status-pane" aria-label="运行概览">
    <div class="status-strip">
      <div class="status-tools">
        <button
          type="button"
          class="status-icon-button link-toggle"
          :class="{ active: globalProxyEnabled, running: coreRunning }"
          :title="globalProxyEnabled ? '关闭代理链路' : '开启代理链路'"
          :aria-pressed="globalProxyEnabled"
          :disabled="busy === 'global-proxy'"
          @click="toggleGlobal"
        >
          <Link2 :size="18" />
        </button>
        <button
          type="button"
          class="status-icon-button"
          title="重启 mihomo 核心"
          :disabled="busy === 'restart'"
          @click="emit('restartCore')"
        >
          <RotateCcw :size="17" />
        </button>
      </div>

      <div class="status-metric">
        <ArrowUp :size="16" class="upload" />
        <strong>{{ formatSpeed(totalUploadSpeed) }}</strong>
      </div>
      <div class="status-metric">
        <ArrowDown :size="16" class="download" />
        <strong>{{ formatSpeed(totalDownloadSpeed) }}</strong>
      </div>
      <div class="status-metric">
        <Settings :size="15" class="traffic" />
        <strong>{{ formatBytes(totalTraffic) }}</strong>
      </div>
      <div class="status-metric">
        <ListChecks :size="15" />
        <strong>{{ enabledChannels }} / {{ channels.length }}</strong>
      </div>
    </div>
  </section>
</template>
