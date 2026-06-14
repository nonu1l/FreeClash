<script setup lang="ts">
import { computed, ref } from "vue";
import { Edit3, Plus, RefreshCw, Search, Server, Trash2 } from "@lucide/vue";
import SubscriptionDialog from "../components/dialogs/SubscriptionDialog.vue";
import type { NodeInfo, Subscription, SubscriptionInput } from "../types";
import { nodeDelayLabel } from "../utils/format";

const props = defineProps<{
  subscriptions: Subscription[];
  nodes: NodeInfo[];
  busy: string | null;
  createSubscription: (input: SubscriptionInput) => Promise<void>;
  updateSubscription: (id: string, input: SubscriptionInput) => Promise<void>;
  deleteSubscription: (id: string) => Promise<void>;
  refreshSubscription: (id: string) => Promise<void>;
  refreshNodes: () => Promise<void>;
}>();

const dialogOpen = ref(false);
const editing = ref<Subscription | null>(null);
const expandedId = ref<string | null>(null);
const nodeQuery = ref("");

const nodesBySubscription = computed(() => {
  const map = new Map<string, NodeInfo[]>();
  for (const node of props.nodes) {
    if (!node.provider_id) continue;
    const nodes = map.get(node.provider_id) ?? [];
    nodes.push(node);
    map.set(node.provider_id, nodes);
  }
  return map;
});

function visibleNodes(subscriptionId: string) {
  const keyword = nodeQuery.value.trim().toLowerCase();
  const nodes = nodesBySubscription.value.get(subscriptionId) ?? [];
  if (!keyword) return nodes;
  return nodes.filter((node) =>
    `${node.name} ${node.node_type} ${node.provider_name ?? ""}`.toLowerCase().includes(keyword),
  );
}

function openCreate() {
  editing.value = null;
  dialogOpen.value = true;
}

function openEdit(subscription: Subscription) {
  editing.value = subscription;
  dialogOpen.value = true;
}

async function saveSubscription(input: SubscriptionInput) {
  if (editing.value) {
    await props.updateSubscription(editing.value.id, input);
  } else {
    await props.createSubscription(input);
  }
}

async function toggleSubscription(subscription: Subscription, enabled: boolean) {
  await props.updateSubscription(subscription.id, {
    name: subscription.name,
    url: subscription.url,
    enabled,
  });
}

function onToggleSubscription(subscription: Subscription, event: Event) {
  void toggleSubscription(subscription, (event.target as HTMLInputElement).checked);
}

async function removeSubscription(subscription: Subscription) {
  if (!window.confirm(`删除订阅「${subscription.name}」？`)) return;
  await props.deleteSubscription(subscription.id);
}
</script>

<template>
  <section class="view">
    <header class="view-header">
      <div>
        <h2>订阅</h2>
      </div>
      <div class="toolbar-actions">
        <button type="button" :disabled="busy === 'refresh-nodes'" @click="refreshNodes">
          <RefreshCw :size="17" />
          刷新全部
        </button>
        <button class="primary" type="button" @click="openCreate">
          <Plus :size="17" />
          新增订阅
        </button>
      </div>
    </header>

    <div v-if="subscriptions.length === 0" class="empty">
      <Server :size="30" />
      <strong>还没有订阅</strong>
      <button class="primary" type="button" @click="openCreate">
        <Plus :size="17" />
        新增订阅
      </button>
    </div>

    <div v-else class="subscription-list">
      <article v-for="subscription in subscriptions" :key="subscription.id" class="subscription-item">
        <div class="subscription-main">
          <label class="switch" title="启用订阅">
            <input
              type="checkbox"
              :checked="subscription.enabled"
              :disabled="busy === 'save-subscription'"
              @change="onToggleSubscription(subscription, $event)"
            />
            <span></span>
          </label>
          <button
            class="text-button subscription-title"
            type="button"
            @click="expandedId = expandedId === subscription.id ? null : subscription.id"
          >
            <strong>{{ subscription.name }}</strong>
            <span>{{ nodesBySubscription.get(subscription.id)?.length ?? 0 }} 节点</span>
          </button>
          <p>{{ subscription.url }}</p>
        </div>
        <div class="row-actions">
          <button
            type="button"
            title="刷新订阅"
            :disabled="!subscription.enabled || busy === `refresh-subscription-${subscription.id}`"
            @click="refreshSubscription(subscription.id)"
          >
            <RefreshCw :size="16" />
          </button>
          <button type="button" title="编辑订阅" @click="openEdit(subscription)">
            <Edit3 :size="16" />
          </button>
          <button
            type="button"
            title="删除订阅"
            :disabled="busy === `delete-subscription-${subscription.id}`"
            @click="removeSubscription(subscription)"
          >
            <Trash2 :size="16" />
          </button>
        </div>

        <div v-if="expandedId === subscription.id" class="subscription-nodes">
          <div class="subscription-node-tools">
            <div class="input-icon">
              <Search :size="16" />
              <input v-model="nodeQuery" type="text" placeholder="搜索节点" />
            </div>
          </div>
          <div v-if="visibleNodes(subscription.id).length > 0" class="node-table">
            <div class="node-table-head">
              <span>节点</span>
              <span>类型</span>
              <span>延迟</span>
            </div>
            <div v-for="node in visibleNodes(subscription.id)" :key="node.name" class="node-table-row">
              <strong :title="node.name">{{ node.name }}</strong>
              <span>{{ node.node_type }}</span>
              <em>{{ nodeDelayLabel(node.delay) }}</em>
            </div>
          </div>
          <span v-else class="muted">暂无节点</span>
        </div>
      </article>
    </div>
  </section>

  <SubscriptionDialog
    :open="dialogOpen"
    :subscription="editing"
    :busy="busy === 'save-subscription'"
    :save-subscription="saveSubscription"
    @close="dialogOpen = false"
  />
</template>
