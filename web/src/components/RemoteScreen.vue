<script setup lang="ts">
import { ref, computed, watch, onUnmounted, nextTick } from 'vue'
import type { MachineInfo, MachineId, SignalingMessage } from '@/types/protocol'

const props = defineProps<{
  remoteStream: MediaStream | null
  connected: boolean
  machines: MachineInfo[]
  currentMachine: MachineId | null
}>()

const emit = defineEmits<{
  selectMachine: [machineId: MachineId]
  inputEvent: [event: SignalingMessage]
}>()

const containerRef = ref<HTMLDivElement | null>(null)
const isFullscreen = ref(false)
const cursorX = ref(0)
const cursorY = ref(0)
const showCursor = ref(false)

// Get display dimensions of the connected machine
const currentMachineInfo = computed(() =>
  props.machines.find(m => m.machine_id === props.currentMachine)
)
const displayW = computed(() => currentMachineInfo.value?.display_width ?? 1920)
const displayH = computed(() => currentMachineInfo.value?.display_height ?? 1080)

// Auto-focus when connected
watch(() => props.connected, (val) => {
  if (val) {
    nextTick(() => containerRef.value?.focus())
  }
})

// ── Input handling ──────────────────────────────────────────────

/// Calculate the actual rendered image rectangle within the container
/// (accounts for `object-fit: contain` letterboxing).
function getImageRect(): { left: number; top: number; width: number; height: number } | null {
  const c = containerRef.value
  if (!c) return null
  const cr = c.getBoundingClientRect()
  const dw = displayW.value
  const dh = displayH.value
  const scale = Math.min(cr.width / dw, cr.height / dh)
  const iw = dw * scale
  const ih = dh * scale
  return {
    left: cr.left + (cr.width - iw) / 2,
    top: cr.top + (cr.height - ih) / 2,
    width: iw,
    height: ih,
  }
}

function getRelativeCoords(clientX: number, clientY: number): { x: number; y: number } {
  const r = getImageRect()
  if (!r) return { x: 0, y: 0 }
  return {
    x: (clientX - r.left) / r.width,
    y: (clientY - r.top) / r.height,
  }
}

function onMouseMove(e: MouseEvent) {
  if (!props.currentMachine) return
  const cr = containerRef.value?.getBoundingClientRect()
  if (cr) {
    cursorX.value = e.clientX - cr.left
    cursorY.value = e.clientY - cr.top
    showCursor.value = true
  }
  const { x, y } = getRelativeCoords(e.clientX, e.clientY)
  emit('inputEvent', {
    type: 'mouse_move',
    target: props.currentMachine || '',
    x: Math.round(x * displayW.value),
    y: Math.round(y * displayH.value),
  } as SignalingMessage)
}

function onMouseDown(e: MouseEvent) {
  if (!props.currentMachine) return
  containerRef.value?.focus()  // ensure keyboard focus
  emit('inputEvent', {
    type: 'mouse_button',
    target: props.currentMachine || '',
    button: e.button,
    pressed: true,
  } as SignalingMessage)
}

function onMouseUp(e: MouseEvent) {
  if (!props.currentMachine) return
  emit('inputEvent', {
    type: 'mouse_button',
    target: props.currentMachine || '',
    button: e.button,
    pressed: false,
  } as SignalingMessage)
}

function onWheel(e: WheelEvent) {
  if (!props.currentMachine) return
  e.preventDefault()
  emit('inputEvent', {
    type: 'mouse_scroll',
    target: props.currentMachine || '',
    dx: e.deltaX,
    dy: e.deltaY,
  } as SignalingMessage)
}

function onKeyDown(e: KeyboardEvent) {
  if (!props.currentMachine) return
  emit('inputEvent', {
    type: 'key_event',
    target: props.currentMachine || '',
    pressed: true,
    key_code: e.keyCode,
  } as SignalingMessage)
}

function onKeyUp(e: KeyboardEvent) {
  if (!props.currentMachine) return
  emit('inputEvent', {
    type: 'key_event',
    target: props.currentMachine || '',
    pressed: false,
    key_code: e.keyCode,
  } as SignalingMessage)
}

function toggleFullscreen() {
  if (!containerRef.value) return
  if (!document.fullscreenElement) {
    containerRef.value.requestFullscreen()
    isFullscreen.value = true
  } else {
    document.exitFullscreen()
    isFullscreen.value = false
  }
}

onUnmounted(() => {
  if (document.fullscreenElement) {
    document.exitFullscreen()
  }
})
</script>

<template>
  <div class="remote-screen">
    <!-- Machine list (when not connected to a machine) -->
    <div v-if="!currentMachine" class="machine-list">
      <h3>Available Machines</h3>
      <div v-if="machines.length === 0" class="empty-state">
        <p>No machines online</p>
        <p class="hint">Run <code>rem0te-client</code> on a remote machine to get started.</p>
      </div>
      <div v-else class="machine-grid">
        <button
          v-for="machine in machines"
          :key="machine.machine_id"
          class="machine-card"
          :class="{ online: machine.online }"
          @click="emit('selectMachine', machine.machine_id)"
        >
          <span class="machine-icon">{{ machine.os === 'macos' ? '🍎' : '🐧' }}</span>
          <div class="machine-info">
            <strong>{{ machine.machine_name }}</strong>
            <span>{{ machine.os_version }}</span>
            <span>{{ machine.display_width }}×{{ machine.display_height }}</span>
          </div>
          <span class="status-dot" :class="{ online: machine.online }"></span>
        </button>
      </div>
    </div>

    <!-- Remote display -->
    <div
      v-else
      ref="containerRef"
      class="screen-container"
      :class="{ connected, fullscreen: isFullscreen }"
      tabindex="0"
      @mousemove="onMouseMove"
      @mousedown="onMouseDown"
      @mouseup="onMouseUp"
      @wheel.prevent="onWheel"
      @keydown="onKeyDown"
      @keyup="onKeyUp"
    >
      <!-- Loading state -->
      <div v-if="!connected" class="screen-placeholder">
        <div class="spinner"></div>
        <p>Connecting to remote machine...</p>
      </div>

      <!-- Video element (native WebRTC, AV1 decoded by browser) -->
      <video
        v-if="connected && remoteStream"
        ref="videoRef"
        :srcObject="remoteStream"
        class="screen-video"
        autoplay
        playsinline
        muted
      ></video>

      <!-- Cursor overlay -->
      <div
        v-if="currentMachine && showCursor"
        class="cursor-overlay"
        :style="{ left: cursorX + 'px', top: cursorY + 'px' }"
      >
        <svg width="16" height="22" viewBox="0 0 16 22" style="pointer-events:none">
          <polygon points="0,0 11,8 6,8 9,17 5,17 3,9 0,11" fill="white" stroke="black" stroke-width="1"/>
        </svg>
      </div>

      <!-- Stream info overlay -->
      <div v-if="connected" class="stream-overlay">
        <button class="fullscreen-btn" @click="toggleFullscreen" title="Toggle fullscreen">
          {{ isFullscreen ? '↙️' : '↗️' }}
        </button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.remote-screen {
  width: 100%;
  height: 100%;
  display: flex;
  align-items: center;
  justify-content: center;
}

/* Machine list */
.machine-list {
  padding: 40px;
  text-align: center;
  max-width: 600px;
  width: 100%;
}

.machine-list h3 {
  margin: 0 0 24px;
}

.machine-grid {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.machine-card {
  display: flex;
  align-items: center;
  gap: 16px;
  padding: 16px;
  background: var(--bg-secondary);
  border: 1px solid var(--border-color);
  border-radius: 8px;
  cursor: pointer;
  text-align: left;
  color: var(--text-primary);
  font-family: inherit;
  font-size: 0.95rem;
  transition: border-color 0.2s, background 0.2s;
}

.machine-card:hover {
  border-color: var(--accent-color);
  background: var(--bg-tertiary);
}

.machine-icon {
  font-size: 1.5rem;
}

.machine-info {
  flex: 1;
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.machine-info span {
  font-size: 0.8rem;
  color: var(--text-secondary);
}

.status-dot {
  width: 10px;
  height: 10px;
  border-radius: 50%;
  background: var(--color-offline);
}

.status-dot.online {
  background: var(--color-online);
}

.empty-state {
  color: var(--text-secondary);
}

.empty-state .hint {
  font-size: 0.85rem;
}

.empty-state code {
  background: var(--bg-tertiary);
  padding: 2px 6px;
  border-radius: 4px;
}

/* Screen container */
.screen-container {
  position: relative;
  width: 100%;
  height: 100%;
  background: #000;
  display: flex;
  align-items: center;
  justify-content: center;
  outline: none;
  cursor: none;
}

.screen-placeholder {
  text-align: center;
  color: var(--text-secondary);
}

.spinner {
  width: 40px;
  height: 40px;
  border: 3px solid var(--border-color);
  border-top-color: var(--accent-color);
  border-radius: 50%;
  animation: spin 0.8s linear infinite;
  margin: 0 auto 16px;
}

@keyframes spin {
  to { transform: rotate(360deg); }
}

.screen-video {
  width: 100%;
  height: 100%;
  object-fit: contain;
}

.stream-overlay {
  position: absolute;
  top: 12px;
  right: 12px;
  z-index: 10;
}

.fullscreen-btn {
  background: rgba(0, 0, 0, 0.5);
  border: 1px solid rgba(255, 255, 255, 0.2);
  color: #fff;
  padding: 8px 12px;
  border-radius: 6px;
  cursor: pointer;
  font-size: 1rem;
  transition: background 0.2s;
}

.fullscreen-btn:hover {
  background: rgba(0, 0, 0, 0.7);
}

/* Cursor overlay */
.cursor-overlay {
  position: absolute;
  pointer-events: none;
  z-index: 100;
  transform: translate(0, 0);
  filter: drop-shadow(1px 1px 1px rgba(0,0,0,0.5));
}
</style>
