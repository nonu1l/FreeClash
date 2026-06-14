<script setup lang="ts">
import { computed } from "vue";
import { RotateCcw, Zap } from "@lucide/vue";
import type { AppSnapshot } from "../../types";

const props = defineProps<{
  snapshot: AppSnapshot | null;
  busy: string | null;
}>();

const emit = defineEmits<{
  toggleGlobal: [enabled: boolean];
  restartCore: [];
}>();

const coreLabel = computed(() => {
  if (!props.snapshot) return "未连接";
  return props.snapshot.status.core_running ? "运行中" : "未运行";
});

function onToggle(event: Event) {
  emit("toggleGlobal", (event.target as HTMLInputElement).checked);
}
</script>

<template>
  <header class="top-bar">
    <div class="top-brand">
      <div class="brand-mark">
        <Zap :size="20" />
      </div>
      <div>
        <h1>FreeClash</h1>
        <p>应用代理通道工作台</p>
      </div>
    </div>

    <div class="top-status">
      <div class="top-status-item status-pill">
        <span>核心</span>
        <strong :class="snapshot?.status.core_running ? 'ok' : 'warn'">{{ coreLabel }}</strong>
      </div>
      <div class="top-status-item status-pill">
        <span>链路</span>
        <strong>{{ snapshot?.config.global_proxy_enabled ? "节点模式" : "本地直连" }}</strong>
      </div>
      <label class="switch with-label" title="切换全局代理链路">
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
        class="button secondary"
        :disabled="busy === 'restart'"
        title="重启 mihomo 核心"
        @click="emit('restartCore')"
      >
        <RotateCcw :size="16" />
        重启核心
      </button>
    </div>
  </header>
</template>
