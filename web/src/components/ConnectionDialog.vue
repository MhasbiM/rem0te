<script setup lang="ts">
import { ref } from 'vue'

defineProps<{
  serverUrl: string
  connecting: boolean
}>()

const emit = defineEmits<{
  connect: [url: string]
}>()

const url = ref('ws://localhost:8080/ws')
const token = ref('changeme')

function handleSubmit() {
  emit('connect', url.value)
}
</script>

<template>
  <div class="connection-dialog">
    <div class="dialog-card">
      <div class="dialog-icon">🖥️</div>
      <h2>Connect to Server</h2>
      <p class="dialog-desc">
        Enter the signaling server URL to start controlling remote machines.
      </p>

      <form @submit.prevent="handleSubmit" class="dialog-form">
        <div class="form-group">
          <label for="server-url">Server WebSocket URL</label>
          <input
            id="server-url"
            v-model="url"
            type="text"
            placeholder="ws://localhost:8080/ws"
            class="input"
            autofocus
          />
        </div>

        <button type="submit" class="btn btn-primary" :disabled="connecting">
          {{ connecting ? 'Connecting...' : 'Connect' }}
        </button>
      </form>

      <p class="dialog-hint">
        Make sure the <code>rem0te-server</code> is running.
      </p>
    </div>
  </div>
</template>

<style scoped>
.connection-dialog {
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 20px;
}

.dialog-card {
  background: var(--bg-secondary);
  border: 1px solid var(--border-color);
  border-radius: 12px;
  padding: 40px;
  max-width: 420px;
  width: 100%;
  text-align: center;
}

.dialog-icon {
  font-size: 3rem;
  margin-bottom: 12px;
}

.dialog-card h2 {
  margin: 0 0 8px;
  font-size: 1.5rem;
}

.dialog-desc {
  color: var(--text-secondary);
  margin: 0 0 24px;
  font-size: 0.9rem;
}

.dialog-form {
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.form-group {
  text-align: left;
}

.form-group label {
  display: block;
  font-size: 0.85rem;
  font-weight: 500;
  margin-bottom: 6px;
  color: var(--text-secondary);
}

.dialog-hint {
  margin-top: 20px;
  font-size: 0.8rem;
  color: var(--text-secondary);
}

.dialog-hint code {
  background: var(--bg-tertiary);
  padding: 2px 6px;
  border-radius: 4px;
  font-size: 0.85em;
}
</style>
