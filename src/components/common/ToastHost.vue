<script setup lang="ts">
defineProps<{
  toasts: Array<{
    id: number;
    message: string;
    tone: "success" | "error" | "info";
  }>;
}>();

const emit = defineEmits<{
  dismiss: [id: number];
}>();
</script>

<template>
  <Teleport to="body">
    <div class="toast-host" aria-live="polite" aria-atomic="true">
      <button
        v-for="toast in toasts"
        :key="toast.id"
        type="button"
        class="toast"
        :class="toast.tone"
        @click="emit('dismiss', toast.id)"
      >
        {{ toast.message }}
      </button>
    </div>
  </Teleport>
</template>
