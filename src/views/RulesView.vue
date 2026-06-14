<script setup lang="ts">
import { computed, ref } from "vue";
import {
  Copy,
  Edit3,
  Plus,
  Route,
  Server,
  Stethoscope,
  Trash2,
} from "@lucide/vue";
import RuleDialog from "../components/dialogs/RuleDialog.vue";
import RuleDiagnosticsDialog from "../components/dialogs/RuleDiagnosticsDialog.vue";
import type {
  ChannelDiagnostics,
  ChannelInput,
  ChannelProxyTestResult,
  ChannelStats,
  NodeInfo,
  ProxyChannel,
} from "../types";
import { formatBytes, formatSpeed } from "../utils/format";

const props = defineProps<{
  channels: ProxyChannel[];
  nodes: NodeInfo[];
  stats: ChannelStats[];
  busy: string | null;
  createChannel: (input: ChannelInput) => Promise<void>;
  updateChannel: (id: string, input: ChannelInput) => Promise<void>;
  deleteChannel: (id: string) => Promise<void>;
  duplicateChannel: (id: string) => Promise<void>;
  setChannelEnabled: (id: string, enabled: boolean) => Promise<void>;
  diagnoseChannel: (id: string) => Promise<ChannelDiagnostics>;
  testChannelProxy: (id: string) => Promise<ChannelProxyTestResult>;
}>();

const dialogOpen = ref(false);
const editing = ref<ProxyChannel | null>(null);
const diagnosticsOpen = ref(false);
const diagnostics = ref<ChannelDiagnostics | null>(null);
const diagnosticsTest = ref<ChannelProxyTestResult | null>(null);
const selectedChannelId = ref<string | null>(null);

const statsByChannel = computed(() => {
  const map = new Map<string, ChannelStats>();
  for (const stat of props.stats) map.set(stat.channel_id, stat);
  return map;
});

function statFor(channel: ProxyChannel) {
  return statsByChannel.value.get(channel.id);
}

function selectedNode(channel: ProxyChannel) {
  return channel.selected_node || "DIRECT";
}

function httpUrl(channel: ProxyChannel) {
  return `http://127.0.0.1:${channel.http_port}`;
}

function socksUrl(channel: ProxyChannel) {
  return `socks5://127.0.0.1:${channel.socks_port}`;
}

function openCreate() {
  editing.value = null;
  dialogOpen.value = true;
}

function openEdit(channel: ProxyChannel) {
  editing.value = channel;
  dialogOpen.value = true;
}

async function saveChannel(input: ChannelInput) {
  if (editing.value) {
    await props.updateChannel(editing.value.id, input);
  } else {
    await props.createChannel(input);
  }
}

async function removeChannel(channel: ProxyChannel) {
  if (!window.confirm(`删除通道「${channel.name}」？`)) return;
  await props.deleteChannel(channel.id);
}

async function openDiagnostics(channel: ProxyChannel) {
  selectedChannelId.value = channel.id;
  diagnostics.value = await props.diagnoseChannel(channel.id);
  diagnosticsTest.value = null;
  diagnosticsOpen.value = true;
}

async function runDiagnosticsTest() {
  if (!diagnostics.value) return;
  const channelId = diagnostics.value.channel_id;
  diagnosticsTest.value = await props.testChannelProxy(channelId);
  diagnostics.value = await props.diagnoseChannel(channelId);
}

function onToggleChannel(channel: ProxyChannel, event: Event) {
  void props.setChannelEnabled(channel.id, (event.target as HTMLInputElement).checked);
}

async function copyAddress(value: string) {
  await navigator.clipboard.writeText(value);
}

function selectChannel(channel: ProxyChannel) {
  selectedChannelId.value = channel.id;
}
</script>

<template>
  <section class="view">
    <header class="view-header">
      <div>
        <h2>代理通道</h2>
      </div>
      <div class="toolbar-actions">
        <button class="primary" type="button" @click="openCreate">
          <Plus :size="17" />
          新增通道
        </button>
      </div>
    </header>

    <div v-if="channels.length === 0" class="empty">
      <Server :size="30" />
      <strong>还没有代理通道</strong>
      <button class="primary" type="button" @click="openCreate">
        <Plus :size="17" />
        新增通道
      </button>
    </div>

    <div v-else class="rules-table channels-table" role="table">
      <div class="rules-row channels-row rules-head" role="row">
        <span>通道名</span>
        <span>节点</span>
        <span>HTTP / SOCKS5 地址</span>
        <span>速度</span>
        <span>流量</span>
        <span>Switch</span>
        <span>操作</span>
      </div>

      <article
        v-for="channel in channels"
        :key="channel.id"
        class="rules-row channels-row"
        :class="{ selected: selectedChannelId === channel.id }"
        role="row"
        @click="selectChannel(channel)"
      >
        <div class="rule-name-cell">
          <strong :title="channel.name">{{ channel.name }}</strong>
        </div>

        <div class="node-cell">
          <strong :title="selectedNode(channel)">{{ selectedNode(channel) }}</strong>
        </div>

        <div class="proxy-addresses">
          <button type="button" title="复制 HTTP 代理地址" @click.stop="copyAddress(httpUrl(channel))">
            <Copy :size="14" />
            <span>{{ httpUrl(channel) }}</span>
          </button>
          <button type="button" title="复制 SOCKS5 代理地址" @click.stop="copyAddress(socksUrl(channel))">
            <Copy :size="14" />
            <span>{{ socksUrl(channel) }}</span>
          </button>
        </div>

        <div>
          <span class="speed-pair">
            <strong>↑ {{ formatSpeed(statFor(channel)?.upload_speed ?? 0) }}</strong>
            <strong>↓ {{ formatSpeed(statFor(channel)?.download_speed ?? 0) }}</strong>
          </span>
        </div>

        <div>
          <strong>{{ formatBytes((statFor(channel)?.upload_total ?? 0) + (statFor(channel)?.download_total ?? 0)) }}</strong>
        </div>

        <div>
          <label class="switch" title="切换通道代理链路">
            <input
              type="checkbox"
              :checked="channel.enabled"
              :disabled="busy === `channel-enabled-${channel.id}`"
              @change="onToggleChannel(channel, $event)"
            />
            <span></span>
          </label>
        </div>

        <div class="row-actions">
          <button type="button" title="诊断" @click.stop="openDiagnostics(channel)">
            <Stethoscope :size="16" />
          </button>
          <button type="button" title="编辑通道" @click.stop="openEdit(channel)">
            <Edit3 :size="16" />
          </button>
          <button
            type="button"
            title="复制通道"
            :disabled="busy === `duplicate-${channel.id}`"
            @click.stop="duplicateChannel(channel.id)"
          >
            <Route :size="16" />
          </button>
          <button
            type="button"
            title="删除通道"
            :disabled="busy === `delete-${channel.id}`"
            @click.stop="removeChannel(channel)"
          >
            <Trash2 :size="16" />
          </button>
        </div>
      </article>
    </div>
  </section>

  <RuleDialog
    :open="dialogOpen"
    :channel="editing"
    :nodes="nodes"
    :busy="busy === 'save-channel'"
    :save-channel="saveChannel"
    @close="dialogOpen = false"
  />

  <RuleDiagnosticsDialog
    :open="diagnosticsOpen"
    :diagnostics="diagnostics"
    :test-result="diagnosticsTest"
    :busy="busy === `test-channel-${diagnostics?.channel_id ?? ''}`"
    :run-test="runDiagnosticsTest"
    @close="diagnosticsOpen = false"
  />
</template>
