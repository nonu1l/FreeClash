<script setup lang="ts">
import { computed, reactive, ref, watch } from "vue";
import { Save, Search, X } from "@lucide/vue";
import type { ChannelDraft, ChannelInput, NodeFilter, NodeInfo, ProxyChannel } from "../../types";
import { nodeDelayLabel } from "../../utils/format";

const props = defineProps<{
  open: boolean;
  channel: ProxyChannel | null;
  nodes: NodeInfo[];
  busy: boolean;
  saveChannel: (input: ChannelInput) => Promise<void>;
}>();

const emit = defineEmits<{
  close: [];
}>();

const draft = reactive<ChannelDraft>({
  name: "",
  selected_node: "DIRECT",
  enabled: true,
});
const query = ref("");
const filter = ref<NodeFilter>("all");

const selectableNodes = computed<NodeInfo[]>(() => {
  const hasDirect = props.nodes.some((node) => node.name === "DIRECT");
  const direct: NodeInfo = {
    name: "DIRECT",
    node_type: "Builtin",
    delay: null,
    is_builtin: true,
    provider_id: null,
    provider_name: null,
  };
  return hasDirect ? props.nodes : [direct, ...props.nodes];
});

const groupedNodes = computed(() => {
  const keyword = query.value.trim().toLowerCase();
  const groups = new Map<string, NodeInfo[]>();
  for (const node of selectableNodes.value) {
    if (keyword) {
      const text = `${node.name} ${node.provider_name ?? ""}`.toLowerCase();
      if (!text.includes(keyword)) continue;
    }
    if (filter.value === "available" && !node.is_builtin && node.delay === null) continue;
    if (filter.value === "untested" && (node.is_builtin || node.delay !== null)) continue;
    if (filter.value === "high" && (node.delay === null || node.delay < 800)) continue;

    const group = node.is_builtin ? "内置" : node.provider_name || "未归类";
    const nodes = groups.get(group) ?? [];
    nodes.push(node);
    groups.set(group, nodes);
  }
  return Array.from(groups, ([name, nodes]) => ({ name, nodes }));
});

watch(
  () => [props.open, props.channel] as const,
  () => {
    if (!props.open) return;
    draft.name = props.channel?.name ?? "";
    draft.selected_node = props.channel?.selected_node ?? "DIRECT";
    draft.enabled = props.channel?.enabled ?? true;
    query.value = "";
    filter.value = "all";
  },
  { immediate: true },
);

async function submit() {
  await props.saveChannel({
    name: draft.name.trim(),
    selected_node: draft.selected_node || "DIRECT",
    enabled: props.channel?.enabled ?? true,
  });
  emit("close");
}
</script>

<template>
  <Teleport to="body">
    <div v-if="open" class="modal-backdrop">
      <section class="modal channel-modal">
        <header class="modal-header">
          <h3>{{ channel ? "编辑代理" : "新增代理" }}</h3>
          <button type="button" title="关闭" @click="emit('close')">
            <X :size="17" />
          </button>
        </header>

        <div class="form-grid">
          <label>
            <span>代理名</span>
            <input v-model="draft.name" type="text" placeholder="Chrome 香港" />
          </label>

          <div class="node-picker wide">
            <div class="node-picker-head">
              <label>
                <span>节点搜索</span>
                <div class="input-icon">
                  <Search :size="16" />
                  <input v-model="query" type="text" placeholder="搜索节点或订阅来源" />
                </div>
              </label>
              <label>
                <span>筛选</span>
                <select v-model="filter">
                  <option value="all">全部</option>
                  <option value="available">可用</option>
                  <option value="untested">未测速</option>
                  <option value="high">高延迟</option>
                </select>
              </label>
            </div>

            <label>
              <span>选择节点</span>
              <select v-model="draft.selected_node" size="7">
                <optgroup v-for="group in groupedNodes" :key="group.name" :label="group.name">
                  <option v-for="node in group.nodes" :key="node.name" :value="node.name">
                    {{ node.name }} · {{ nodeDelayLabel(node.delay) }}
                  </option>
                </optgroup>
              </select>
            </label>
          </div>
        </div>

        <footer class="modal-actions">
          <button type="button" @click="emit('close')">取消</button>
          <button class="primary" type="button" :disabled="busy" @click="submit">
            <Save :size="16" />
            保存
          </button>
        </footer>
      </section>
    </div>
  </Teleport>
</template>
