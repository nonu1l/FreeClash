<script setup lang="ts">
import type { ActiveView, AppSnapshot } from "../../types";
import SideNav from "./SideNav.vue";
import StatusPane from "./StatusPane.vue";
import TopBar from "./TopBar.vue";

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
</script>

<template>
  <main class="app-shell">
    <TopBar
      :snapshot="snapshot"
      :busy="busy"
      @toggle-global="emit('toggleGlobal', $event)"
      @restart-core="emit('restartCore')"
    />

    <aside class="left-rail">
      <SideNav :active-view="activeView" @change="emit('changeView', $event)" />
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
