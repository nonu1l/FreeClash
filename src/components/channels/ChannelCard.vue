<script setup lang="ts">
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
</script>

<template>
  <article
    class="channel-card"
    :class="{ selected }"
    role="listitem"
    @click="emit('select', channel)"
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
</template>
