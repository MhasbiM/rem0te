<script setup lang="ts">
defineProps<{
  connected: boolean
  streaming: boolean
  error: string | null
}>()
</script>

<template>
  <div class="status-indicator">
    <div v-if="error" class="status-badge error" :title="error">
      ⚠️ {{ error }}
    </div>
    <div v-else-if="streaming" class="status-badge streaming">
      🔴 Live
    </div>
    <div v-else-if="connected" class="status-badge online">
      🟢 Connected
    </div>
    <div v-else class="status-badge offline">
      ⚫ Offline
    </div>
  </div>
</template>

<style scoped>
.status-indicator {
  display: flex;
  align-items: center;
}

.status-badge {
  padding: 4px 12px;
  border-radius: 20px;
  font-size: 0.8rem;
  font-weight: 500;
}

.status-badge.streaming {
  background: rgba(239, 68, 68, 0.15);
  color: #ef4444;
  border: 1px solid rgba(239, 68, 68, 0.3);
}

.status-badge.online {
  background: rgba(34, 197, 94, 0.15);
  color: #22c55e;
  border: 1px solid rgba(34, 197, 94, 0.3);
}

.status-badge.offline {
  background: var(--bg-tertiary);
  color: var(--text-secondary);
  border: 1px solid var(--border-color);
}

.status-badge.error {
  background: rgba(250, 204, 21, 0.15);
  color: #facc15;
  border: 1px solid rgba(250, 204, 21, 0.3);
  max-width: 300px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
</style>
