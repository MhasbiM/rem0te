<script setup lang="ts">
defineProps<{
  connected: boolean
  machineName: string
}>()

defineEmits<{
  disconnect: []
  refresh: []
  toggleDialog: []
}>()
</script>

<template>
  <footer class="control-bar">
    <div class="control-left">
      <span v-if="machineName" class="machine-label">
        <span class="dot" :class="{ connected }"></span>
        {{ machineName }}
      </span>
      <span v-else class="machine-label dim">Not connected</span>
    </div>

    <div class="control-center">
      <button class="ctrl-btn" @click="$emit('refresh')" title="Refresh machine list">
        🔄
      </button>
      <button class="ctrl-btn" @click="$emit('toggleDialog')" title="Change server">
        ⚙️
      </button>
    </div>

    <div class="control-right">
      <button class="ctrl-btn disconnect-btn" @click="$emit('disconnect')" title="Disconnect">
        ✕ Disconnect
      </button>
    </div>
  </footer>
</template>

<style scoped>
.control-bar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 8px 20px;
  background: var(--bg-secondary);
  border-top: 1px solid var(--border-color);
  flex-shrink: 0;
  font-size: 0.85rem;
}

.control-left,
.control-center,
.control-right {
  display: flex;
  align-items: center;
  gap: 8px;
}

.machine-label {
  display: flex;
  align-items: center;
  gap: 8px;
  font-weight: 500;
}

.machine-label.dim {
  color: var(--text-secondary);
}

.dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background: var(--color-offline);
}

.dot.connected {
  background: var(--color-online);
  box-shadow: 0 0 6px var(--color-online);
}

.ctrl-btn {
  background: var(--bg-tertiary);
  border: 1px solid var(--border-color);
  color: var(--text-primary);
  padding: 6px 12px;
  border-radius: 6px;
  cursor: pointer;
  font-size: 0.85rem;
  transition: background 0.2s;
  font-family: inherit;
}

.ctrl-btn:hover {
  background: var(--bg-primary);
}

.disconnect-btn {
  color: #ef4444;
  border-color: rgba(239, 68, 68, 0.3);
}

.disconnect-btn:hover {
  background: rgba(239, 68, 68, 0.1);
}
</style>
