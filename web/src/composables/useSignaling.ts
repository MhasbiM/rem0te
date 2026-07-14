import { ref, onUnmounted, type Ref } from 'vue'
import type { SignalingMessage, MachineInfo, MachineId } from '@/types/protocol'

export interface UseSignalingReturn {
  connected: Ref<boolean>
  machines: Ref<MachineInfo[]>
  currentMachine: Ref<MachineId | null>
  error: Ref<string | null>
  connect: (serverUrl: string) => void
  disconnect: () => void
  listMachines: () => void
  connectToMachine: (machineId: MachineId) => void
  sendWebRtcAnswer: (machineId: MachineId, sdp: string) => void
  sendIceCandidate: (machineId: MachineId, candidate: string, sdpMid: string | null, sdpMLineIndex: number | null) => void
  sendInputEvent: (event: SignalingMessage) => void
  onMessage: (handler: (msg: SignalingMessage) => void) => void
}

export function useSignaling(): UseSignalingReturn {
  const connected = ref(false)
  const machines = ref<MachineInfo[]>([])
  const currentMachine = ref<MachineId | null>(null)
  const error = ref<string | null>(null)

  let ws: WebSocket | null = null
  const messageHandlers: Array<(msg: SignalingMessage) => void> = []

  function connect(serverUrl: string) {
    if (ws) {
      ws.close()
    }

    ws = new WebSocket(serverUrl)

    ws.onopen = () => {
      connected.value = true
      error.value = null
      console.log('[signaling] connected to', serverUrl)
    }

    ws.onclose = () => {
      connected.value = false
      currentMachine.value = null
      console.log('[signaling] disconnected')
      // Auto-reconnect after 3 seconds
      setTimeout(() => {
        if (!connected.value) {
          connect(serverUrl)
        }
      }, 3000)
    }

    ws.onerror = (ev) => {
      error.value = 'WebSocket connection error'
      console.error('[signaling] error:', ev)
    }

    ws.onmessage = (ev) => {
      try {
        const msg: SignalingMessage = JSON.parse(ev.data)
        console.log('[signaling] received:', msg.type)

        // Handle built-in message types
        switch (msg.type) {
          case 'machine_list':
            machines.value = msg.machines
            break
          case 'machine_online':
            machines.value = machines.value.map(m =>
              m.machine_id === msg.machine.machine_id ? msg.machine : m
            )
            if (!machines.value.find(m => m.machine_id === msg.machine.machine_id)) {
              machines.value.push(msg.machine)
            }
            break
          case 'machine_offline':
            machines.value = machines.value.filter(m => m.machine_id !== msg.machine_id)
            break
          case 'connected':
            currentMachine.value = msg.machine_id
            error.value = null
            break
          case 'connection_failed':
            error.value = msg.reason
            currentMachine.value = null
            break
          case 'error':
            error.value = msg.message
            break
        }

        // Forward to all registered handlers
        for (const handler of messageHandlers) {
          handler(msg)
        }
      } catch (e) {
        console.error('[signaling] parse error:', e)
      }
    }
  }

  function disconnect() {
    if (ws) {
      send({ type: 'disconnect' } as SignalingMessage)
      ws.close()
      ws = null
    }
    connected.value = false
    currentMachine.value = null
  }

  function send(msg: SignalingMessage) {
    if (ws && ws.readyState === WebSocket.OPEN) {
      ws.send(JSON.stringify(msg))
    } else {
      console.warn('[signaling] not connected, cannot send')
    }
  }

  function listMachines() {
    send({ type: 'list_machines' } as SignalingMessage)
  }

  function connectToMachine(machineId: MachineId) {
    send({ type: 'connect_to_machine', machine_id: machineId } as SignalingMessage)
  }

  function sendWebRtcAnswer(machineId: MachineId, sdp: string) {
    send({ type: 'webrtc_answer', target_machine: machineId, sdp } as SignalingMessage)
  }

  function sendIceCandidate(
    machineId: MachineId,
    candidate: string,
    sdpMid: string | null,
    sdpMLineIndex: number | null,
  ) {
    send({
      type: 'ice_candidate_to_agent',
      target_machine: machineId,
      candidate,
      sdp_mid: sdpMid,
      sdp_m_line_index: sdpMLineIndex,
    } as SignalingMessage)
  }

  function sendInputEvent(event: SignalingMessage) {
    if (currentMachine.value) {
      send({ ...event, target: currentMachine.value } as SignalingMessage)
    }
  }

  function onMessage(handler: (msg: SignalingMessage) => void) {
    messageHandlers.push(handler)
  }

  onUnmounted(() => {
    disconnect()
  })

  return {
    connected,
    machines,
    currentMachine,
    error,
    connect,
    disconnect,
    listMachines,
    connectToMachine,
    sendWebRtcAnswer,
    sendIceCandidate,
    sendInputEvent,
    onMessage,
  }
}
