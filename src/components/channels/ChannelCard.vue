<script setup lang="ts">
import { ref } from "vue";
import { Copy, Edit3, Route, Stethoscope, Trash2 } from "@lucide/vue";
import type { ChannelStats, ProxyChannel } from "../../types";
import { formatBytes, formatSpeed } from "../../utils/format";

const props = defineProps<{
  channel: ProxyChannel;
  stats: ChannelStats | undefined;
  selected: boolean;
  busy: string | null;
}>();

const emit = defineEmits<{
  select: [channel: ProxyChannel];
  toggle: [channel: ProxyChannel, enabled: boolean];
  copyAddress: [address: string];
  diagnose: [channel: ProxyChannel];
  edit: [channel: ProxyChannel];
  duplicate: [channel: ProxyChannel];
  delete: [channel: ProxyChannel];
}>();

const menuOpen = ref(false);
const menuX = ref(0);
const menuY = ref(0);

function selectedNode() {
  return props.channel.selected_node || "DIRECT";
}

function httpUrl() {
  return `http://127.0.0.1:${props.channel.http_port}`;
}

function socksUrl() {
  return `socks5://127.0.0.1:${props.channel.socks_port}`;
}

function totalTraffic() {
  return (props.stats?.upload_total ?? 0) + (props.stats?.download_total ?? 0);
}

function onToggle(event: Event) {
  emit("toggle", props.channel, (event.target as HTMLInputElement).checked);
}

function openMenu(event: MouseEvent) {
  emit("select", props.channel);
  menuX.value = Math.min(event.clientX, window.innerWidth - 220);
  menuY.value = Math.min(event.clientY, window.innerHeight - 260);
  menuOpen.value = true;
}

function runMenu(action: () => void) {
  menuOpen.value = false;
  action();
}
</script>

<template>
  <article
    class="channel-card"
    :class="{ selected }"
    role="listitem"
    @click="emit('select', channel)"
    @contextmenu.prevent="openMenu"
  >
    <div class="channel-card-main">
      <div class="channel-title">
        <strong :title="channel.name">{{ channel.name }}</strong>
        <span :title="selectedNode()">{{ selectedNode() }}</span>
      </div>

      <div class="channel-speed">
        <strong>↑ {{ formatSpeed(stats?.upload_speed ?? 0) }}</strong>
        <strong>↓ {{ formatSpeed(stats?.download_speed ?? 0) }}</strong>
      </div>

      <label class="switch" title="切换通道代理链路" @click.stop>
        <input
          type="checkbox"
          :checked="channel.enabled"
          :disabled="busy === `channel-enabled-${channel.id}`"
          @change="onToggle"
        />
        <span></span>
      </label>
    </div>

    <div class="channel-card-sub">
      <div class="channel-addresses">
        <button type="button" title="复制 HTTP 代理地址" @click.stop="emit('copyAddress', httpUrl())">
          <Copy :size="14" />
          <span>{{ httpUrl() }}</span>
        </button>
        <button type="button" title="复制 SOCKS5 代理地址" @click.stop="emit('copyAddress', socksUrl())">
          <Copy :size="14" />
          <span>{{ socksUrl() }}</span>
        </button>
      </div>

      <div class="channel-traffic">
        <span>流量</span>
        <strong>{{ formatBytes(totalTraffic()) }}</strong>
      </div>

      <div class="row-actions channel-actions">
        <button type="button" title="诊断" @click.stop="emit('diagnose', channel)">
          <Stethoscope :size="16" />
        </button>
        <button type="button" title="编辑通道" @click.stop="emit('edit', channel)">
          <Edit3 :size="16" />
        </button>
        <button
          type="button"
          title="复制通道"
          :disabled="busy === `duplicate-${channel.id}`"
          @click.stop="emit('duplicate', channel)"
        >
          <Route :size="16" />
        </button>
        <button
          type="button"
          title="删除通道"
          :disabled="busy === `delete-${channel.id}`"
          @click.stop="emit('delete', channel)"
        >
          <Trash2 :size="16" />
        </button>
      </div>
    </div>
  </article>

  <Teleport to="body">
    <div v-if="menuOpen" class="context-menu-scrim" @click="menuOpen = false" @contextmenu.prevent="menuOpen = false">
      <div class="context-menu" :style="{ left: `${menuX}px`, top: `${menuY}px` }" @click.stop>
        <button type="button" @click="runMenu(() => emit('copyAddress', httpUrl()))">
          <Copy :size="15" />
          复制 HTTP 地址
        </button>
        <button type="button" @click="runMenu(() => emit('copyAddress', socksUrl()))">
          <Copy :size="15" />
          复制 SOCKS5 地址
        </button>
        <button type="button" @click="runMenu(() => emit('diagnose', channel))">
          <Stethoscope :size="15" />
          诊断
        </button>
        <button type="button" @click="runMenu(() => emit('edit', channel))">
          <Edit3 :size="15" />
          编辑
        </button>
        <button type="button" @click="runMenu(() => emit('duplicate', channel))">
          <Route :size="15" />
          复制通道
        </button>
        <button type="button" class="danger-item" @click="runMenu(() => emit('delete', channel))">
          <Trash2 :size="15" />
          删除
        </button>
      </div>
    </div>
  </Teleport>
</template>
