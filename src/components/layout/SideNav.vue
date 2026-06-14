<script setup lang="ts">
import { Server, Settings, Wifi } from "@lucide/vue";
import type { ActiveView } from "../../types";

defineProps<{
  activeView: ActiveView;
}>();

const emit = defineEmits<{
  change: [view: ActiveView];
}>();
</script>

<template>
  <nav class="side-nav" aria-label="主导航">
    <button
      type="button"
      :class="{ active: activeView === 'channels' }"
      @click="emit('change', 'channels')"
    >
      <Wifi :size="18" stroke-width="2.2" />
      代理
    </button>
    <button
      type="button"
      :class="{ active: activeView === 'subscriptions' }"
      @click="emit('change', 'subscriptions')"
    >
      <Server :size="18" stroke-width="2.2" />
      订阅
    </button>
    <button
      type="button"
      :class="{ active: activeView === 'settings' }"
      @click="emit('change', 'settings')"
    >
      <Settings :size="18" stroke-width="2.2" />
      设置
    </button>
  </nav>
</template>
