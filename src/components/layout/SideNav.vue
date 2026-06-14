<script setup lang="ts">
import { Globe2, Server, Settings, Wifi } from "@lucide/vue";
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
      <Wifi :size="24" stroke-width="2.25" />
      代理
    </button>
    <button
      type="button"
      :class="{ active: activeView === 'subscriptions' }"
      @click="emit('change', 'subscriptions')"
    >
      <Server :size="24" stroke-width="2.25" />
      订阅
    </button>
    <button
      type="button"
      :class="{ active: activeView === 'connections' }"
      @click="emit('change', 'connections')"
    >
      <Globe2 :size="24" stroke-width="2.25" />
      连接
    </button>
    <button
      type="button"
      :class="{ active: activeView === 'settings' }"
      @click="emit('change', 'settings')"
    >
      <Settings :size="24" stroke-width="2.25" />
      设置
    </button>
  </nav>
</template>
