<script setup lang="ts">
import { computed, ref } from "vue";
import { Plus, Server } from "@lucide/vue";
import ChannelCard from "../components/channels/ChannelCard.vue";
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
  notify: (message: string, tone?: "success" | "error" | "info") => void;
  confirmAction: (
    title: string,
    message: string,
    options?: { confirmText?: string; danger?: boolean },
  ) => Promise<boolean>;
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
  const confirmed = await props.confirmAction(
    "删除通道",
    `确定删除通道「${channel.name}」？删除后端口和统计记录也会随通道移除。`,
    { confirmText: "删除", danger: true },
  );
  if (!confirmed) return;
  await props.deleteChannel(channel.id);
  props.notify("通道已删除");
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

function toggleChannel(channel: ProxyChannel, enabled: boolean) {
  void props.setChannelEnabled(channel.id, enabled);
}

async function copyAddress(value: string) {
  await navigator.clipboard.writeText(value);
  props.notify("已复制代理地址");
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

    <div v-else class="channel-list" role="list">
      <ChannelCard
        v-for="channel in channels"
        :key="channel.id"
        :channel="channel"
        :stats="statFor(channel)"
        :selected="selectedChannelId === channel.id"
        :busy="busy"
        @select="selectChannel"
        @toggle="toggleChannel"
        @copy-address="copyAddress"
        @diagnose="openDiagnostics"
        @edit="openEdit"
        @duplicate="(item) => duplicateChannel(item.id)"
        @delete="removeChannel"
      />
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
