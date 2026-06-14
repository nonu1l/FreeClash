<script setup lang="ts">
import { reactive, watch } from "vue";
import { Save, X } from "@lucide/vue";
import type { Subscription, SubscriptionInput } from "../../types";

const props = defineProps<{
  open: boolean;
  subscription: Subscription | null;
  busy: boolean;
  saveSubscription: (input: SubscriptionInput) => Promise<void>;
}>();

const emit = defineEmits<{
  close: [];
}>();

const draft = reactive<SubscriptionInput>({
  name: "",
  url: "",
});

watch(
  () => [props.open, props.subscription] as const,
  () => {
    if (!props.open) return;
    draft.name = props.subscription?.name ?? "";
    draft.url = props.subscription?.url ?? "";
  },
  { immediate: true },
);

async function submit() {
  await props.saveSubscription({
    name: draft.name.trim(),
    url: draft.url.trim(),
  });
  emit("close");
}
</script>

<template>
  <Teleport to="body">
    <div v-if="open" class="modal-backdrop">
      <section class="modal">
        <header class="modal-header">
          <h3>{{ subscription ? "编辑订阅" : "新增订阅" }}</h3>
          <button type="button" title="关闭" @click="emit('close')">
            <X :size="17" />
          </button>
        </header>

        <div class="form-grid one">
          <label>
            <span>订阅名称</span>
            <input v-model="draft.name" type="text" placeholder="工作订阅" />
          </label>
          <label>
            <span>订阅地址</span>
            <textarea v-model="draft.url" rows="5" spellcheck="false" placeholder="https://example.com/sub"></textarea>
          </label>
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
