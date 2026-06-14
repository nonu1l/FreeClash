<script setup lang="ts">
import { RotateCcw, Zap } from "@lucide/vue";
import type { ActiveView, AppSnapshot } from "../../types";
import SideNav from "./SideNav.vue";
import StatusPane from "./StatusPane.vue";

defineProps<{
  snapshot: AppSnapshot | null;
  activeView: ActiveView;
  busy: string | null;
}>();

const emit = defineEmits<{
  changeView: [view: ActiveView];
  toggleGlobal: [enabled: boolean];
  setHttpApiConfig: [enabled: boolean, port: number];
  restartCore: [];
}>();

function onToggle(event: Event) {
  emit("toggleGlobal", (event.target as HTMLInputElement).checked);
}
</script>

<template>
  <main class="app-shell">
    <aside class="left-rail">
      <div class="rail-brand">
        <div class="brand-mark">
          <Zap :size="20" />
        </div>
        <div>
          <h1>FreeClash</h1>
          <p>应用代理通道工作台</p>
        </div>
      </div>

      <SideNav :active-view="activeView" @change="emit('changeView', $event)" />

      <section class="rail-link-panel" aria-label="代理链路">
        <div>
          <span>链路</span>
          <strong>{{ snapshot?.config.global_proxy_enabled ? "节点模式" : "本地直连" }}</strong>
        </div>
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
          class="button secondary full-button"
          :disabled="busy === 'restart'"
          title="重启 mihomo 核心"
          @click="emit('restartCore')"
        >
          <RotateCcw :size="16" />
          重启核心
        </button>
      </section>

      <StatusPane
        :snapshot="snapshot"
        :busy="busy"
        @set-http-api-config="emit('setHttpApiConfig', $event.enabled, $event.port)"
      />
    </aside>

    <section class="workspace">
      <slot />
    </section>
  </main>
</template>
