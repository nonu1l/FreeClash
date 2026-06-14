<script setup lang="ts">
import { History, ListChecks, Rss, Settings } from "@lucide/vue";
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
    <span class="nav-section">工作台</span>
    <button
      type="button"
      :class="{ active: activeView === 'channels' }"
      @click="emit('change', 'channels')"
    >
      <ListChecks :size="18" />
      代理通道
    </button>
    <button
      type="button"
      :class="{ active: activeView === 'subscriptions' }"
      @click="emit('change', 'subscriptions')"
    >
      <Rss :size="18" />
      订阅
    </button>
    <span class="nav-section">工具</span>
    <button
      type="button"
      :class="{ active: activeView === 'connections' }"
      @click="emit('change', 'connections')"
    >
      <History :size="18" />
      连接记录
    </button>
    <button
      type="button"
      :class="{ active: activeView === 'settings' }"
      @click="emit('change', 'settings')"
    >
      <Settings :size="18" />
      设置
    </button>
  </nav>
</template>
