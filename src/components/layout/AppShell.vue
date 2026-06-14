<script setup lang="ts">
import { Zap } from "@lucide/vue";
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
  restartCore: [];
}>();
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

      <StatusPane
        :snapshot="snapshot"
        :busy="busy"
        @toggle-global="emit('toggleGlobal', $event)"
        @restart-core="emit('restartCore')"
      />
    </aside>

    <section class="workspace">
      <slot />
    </section>
  </main>
</template>
