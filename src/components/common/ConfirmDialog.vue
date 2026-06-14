<script setup lang="ts">
defineProps<{
  open: boolean;
  title: string;
  message: string;
  confirmText?: string;
  danger?: boolean;
}>();

const emit = defineEmits<{
  close: [];
  confirm: [];
}>();
</script>

<template>
  <Teleport to="body">
    <div v-if="open" class="modal-backdrop">
      <section class="modal confirm-dialog" role="dialog" aria-modal="true" :aria-label="title">
        <header class="modal-header">
          <div>
            <h3>{{ title }}</h3>
          </div>
        </header>
        <p class="confirm-message">{{ message }}</p>
        <footer class="modal-actions">
          <button type="button" @click="emit('close')">取消</button>
          <button
            type="button"
            :class="danger ? 'danger-button' : 'primary'"
            @click="emit('confirm')"
          >
            {{ confirmText || "确认" }}
          </button>
        </footer>
      </section>
    </div>
  </Teleport>
</template>
