<script setup lang="ts">
import { computed, ref } from "vue";
import { Import, RefreshCw, Search, Trash2 } from "@lucide/vue";
import type { NodeInfo, Subscription, SubscriptionInput } from "../types";
import { nodeDelayLabel } from "../utils/format";

const props = defineProps<{
  subscriptions: Subscription[];
  nodes: NodeInfo[];
  busy: string | null;
  createSubscription: (input: SubscriptionInput) => Promise<Subscription>;
  deleteSubscription: (id: string) => Promise<void>;
  refreshSubscription: (id: string) => Promise<void>;
  refreshNodes: () => Promise<void>;
  confirmAction: (
    title: string,
    message: string,
    options?: { confirmText?: string; danger?: boolean },
  ) => Promise<boolean>;
}>();

const expandedId = ref<string | null>(null);
const nodeQuery = ref("");
const importUrl = ref("");

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

async function importSubscription() {
  const url = importUrl.value.trim();
  if (!url) return;
  const subscription = await props.createSubscription({
    name: subscriptionNameFromUrl(url),
    url,
  });
  importUrl.value = "";
  await props.refreshSubscription(subscription.id);
}

async function removeSubscription(subscription: Subscription) {
  const confirmed = await props.confirmAction(
    "删除订阅",
    `确定删除订阅「${subscription.name}」？该订阅下的节点将从合并节点池移除。`,
    { confirmText: "删除", danger: true },
  );
  if (!confirmed) return;
  await props.deleteSubscription(subscription.id);
}

function subscriptionNameFromUrl(value: string) {
  let base = "订阅";
  try {
    const url = new URL(value);
    base = url.hostname.replace(/^www\./, "") || base;
  } catch {
    const compact = value.replace(/^https?:\/\//, "").split(/[/?#]/)[0];
    if (compact) base = compact;
  }

  const names = new Set(props.subscriptions.map((subscription) => subscription.name));
  if (!names.has(base)) return base;

  let index = 2;
  while (names.has(`${base} ${index}`)) index += 1;
  return `${base} ${index}`;
}
</script>

<template>
  <section class="view">
    <header class="view-header">
      <div>
        <h2>订阅</h2>
      </div>
      <form class="subscription-import" @submit.prevent="importSubscription">
        <input v-model="importUrl" type="url" placeholder="输入订阅地址" />
        <button
          class="primary"
          type="submit"
          title="导入订阅"
          :disabled="busy === 'save-subscription' || !importUrl.trim()"
        >
          <Import :size="17" />
        </button>
        <button type="button" title="刷新全部" :disabled="busy === 'refresh-nodes'" @click="refreshNodes">
          <RefreshCw :size="17" />
        </button>
      </form>
    </header>

    <div v-if="subscriptions.length === 0" class="empty">
      <strong>还没有订阅</strong>
      <span>在顶部输入订阅地址后导入。</span>
    </div>

    <div v-else class="subscription-list">
      <article v-for="subscription in subscriptions" :key="subscription.id" class="subscription-item">
        <div class="subscription-main">
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
            :disabled="busy === `refresh-subscription-${subscription.id}`"
            @click="refreshSubscription(subscription.id)"
          >
            <RefreshCw :size="16" />
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
</template>
