<script setup lang="ts">
import { computed } from "vue";
import { ArrowDown, ArrowUp, ListChecks, Settings } from "@lucide/vue";
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
  <section class="status-pane" aria-label="运行概览">
    <div class="status-strip">
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
