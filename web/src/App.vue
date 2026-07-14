<script setup lang="ts">
import { ref, onMounted } from 'vue'
import ConnectionDialog from './components/ConnectionDialog.vue'
import RemoteScreen from './components/RemoteScreen.vue'
import ControlBar from './components/ControlBar.vue'
import StatusIndicator from './components/StatusIndicator.vue'
import { useSignaling } from './composables/useSignaling'
import { useWebRTC } from './composables/useWebRTC'
import type { MachineId, SignalingMessage } from './types/protocol'

// ── State ────────────────────────────────────────────────────────
const serverUrl = ref(localStorage.getItem('rem0te-server') || 'ws://localhost:8080/ws')
const showConnectionDialog = ref(true)

// ── Composables ──────────────────────────────────────────────────
const signaling = useSignaling()
const webrtc = useWebRTC((msg: SignalingMessage) => {
  // Sends WebRTC signaling messages through the signaling channel
  if (msg.type === 'web_rtc_answer') {
    signaling.sendWebRtcAnswer(msg.target_machine, msg.sdp)
  } else if (msg.type === 'ice_candidate_to_agent') {
    signaling.sendIceCandidate(msg.target_machine, msg.candidate, msg.sdp_mid, msg.sdp_m_line_index)
  }
})

// Forward signaling messages to WebRTC handler
signaling.onMessage((msg: SignalingMessage) => {
  if (msg.type === 'web_rtc_answer_from_agent' || msg.type === 'ice_candidate_from_agent') {
    webrtc.handleSignalingMessage(msg)
  }
})

// ── Handlers ─────────────────────────────────────────────────────
function handleConnect(url: string) {
  serverUrl.value = url
  localStorage.setItem('rem0te-server', url)
  signaling.connect(url)
  showConnectionDialog.value = false

  // List machines once connected
  setTimeout(() => {
    signaling.listMachines()
  }, 500)
}

function handleDisconnect() {
  webrtc.disconnect()
  signaling.disconnect()
  showConnectionDialog.value = true
}

function handleSelectMachine(machineId: MachineId) {
  signaling.connectToMachine(machineId)
  webrtc.connect(machineId)
}

function handleInputEvent(event: SignalingMessage) {
  webrtc.sendInputEvent(event)
}

// ── Lifecycle ────────────────────────────────────────────────────
onMounted(() => {
  // Auto-connect if we have a saved server URL
  const saved = localStorage.getItem('rem0te-server')
  if (saved) {
    handleConnect(saved)
  }
})
</script>

<template>
  <div class="app">
    <header class="app-header">
      <h1 class="logo">🖥️ rem0te</h1>
      <StatusIndicator
        :connected="signaling.connected.value"
        :streaming="webrtc.connected.value"
        :error="signaling.error.value"
      />
    </header>

    <main class="app-main">
      <!-- Connection Dialog -->
      <ConnectionDialog
        v-if="showConnectionDialog"
        :server-url="serverUrl"
        :connecting="!signaling.connected.value && !showConnectionDialog"
        @connect="handleConnect"
      />

      <!-- Remote Screen -->
      <RemoteScreen
        v-if="signaling.connected.value && !showConnectionDialog"
        :current-frame-url="webrtc.currentFrameUrl.value"
        :connected="webrtc.connected.value"
        :machines="signaling.machines.value"
        :current-machine="signaling.currentMachine.value"
        @select-machine="handleSelectMachine"
        @input-event="handleInputEvent"
      />
    </main>

    <!-- Control Bar -->
    <ControlBar
      v-if="signaling.connected.value && !showConnectionDialog"
      :connected="webrtc.connected.value"
      :machine-name="signaling.currentMachine.value || ''"
      @disconnect="handleDisconnect"
      @refresh="signaling.listMachines()"
      @toggle-dialog="showConnectionDialog = !showConnectionDialog"
    />
  </div>
</template>

<style scoped>
.app {
  display: flex;
  flex-direction: column;
  height: 100vh;
  background: var(--bg-primary);
  color: var(--text-primary);
}

.app-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 12px 20px;
  background: var(--bg-secondary);
  border-bottom: 1px solid var(--border-color);
  flex-shrink: 0;
}

.logo {
  font-size: 1.25rem;
  font-weight: 700;
  margin: 0;
  letter-spacing: -0.5px;
}

.app-main {
  flex: 1;
  overflow: hidden;
  display: flex;
  align-items: center;
  justify-content: center;
}
</style>
