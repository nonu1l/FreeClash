<script setup lang="ts">
import { computed } from "vue";
import { Zap } from "@lucide/vue";
import type { ActiveView, AppSnapshot } from "../../types";
import SideNav from "./SideNav.vue";
import StatusPane from "./StatusPane.vue";

const props = defineProps<{
  snapshot: AppSnapshot | null;
  activeView: ActiveView;
  busy: string | null;
}>();

const emit = defineEmits<{
  changeView: [view: ActiveView];
  toggleGlobal: [enabled: boolean];
}>();

const globalProxyEnabled = computed(() => props.snapshot?.config.global_proxy_enabled ?? true);

function toggleGlobal() {
  if (props.busy === "global-proxy") return;
  emit("toggleGlobal", !globalProxyEnabled.value);
}
</script>

<template>
  <main class="app-shell">
    <aside class="left-rail">
      <div class="rail-brand">
        <button
          type="button"
          class="brand-mark brand-toggle"
          :class="{ active: globalProxyEnabled }"
          :title="globalProxyEnabled ? '关闭代理链路' : '开启代理链路'"
          :aria-pressed="globalProxyEnabled"
          :disabled="busy === 'global-proxy'"
          @click="toggleGlobal"
        >
          <Zap :size="20" />
        </button>
        <div>
          <h1>FreeClash</h1>
          <p>应用代理</p>
        </div>
      </div>

      <SideNav :active-view="activeView" @change="emit('changeView', $event)" />

      <StatusPane
        :snapshot="snapshot"
      />
    </aside>

    <section class="workspace">
      <slot />
    </section>
  </main>
</template>
