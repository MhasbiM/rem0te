import { ref, type Ref } from 'vue'
import type { MachineId, SignalingMessage } from '@/types/protocol'

export interface UseWebRTCReturn {
  /** MediaStream from the remote agent's video track (AV1 via WebRTC). */
  remoteStream: Ref<MediaStream | null>
  connected: Ref<boolean>
  connect: (machineId: MachineId) => Promise<void>
  disconnect: () => void
  sendInputEvent: (event: SignalingMessage) => void
  handleSignalingMessage: (msg: SignalingMessage) => void
}

export function useWebRTC(
  sendSignaling: (msg: SignalingMessage) => void,
): UseWebRTCReturn {
  const remoteStream = ref<MediaStream | null>(null)
  const connected = ref(false)

  let pc: RTCPeerConnection | null = null
  let inputChannel: RTCDataChannel | null = null
  let currentMachineId: MachineId | null = null

  const pendingInputEvents: SignalingMessage[] = []

  const iceServers: RTCConfiguration = {
    iceServers: [
      { urls: 'stun:stun.l.google.com:19302' },
    ],
  }

  async function connect(machineId: MachineId) {
    currentMachineId = machineId

    pc = new RTCPeerConnection(iceServers)

    // ── Incoming video track (native WebRTC, AV1 decoded by browser) ──
    pc.ontrack = (event) => {
      console.log('[webrtc] remote track received:', event.track.kind, event.track.id)
      if (event.track.kind === 'video') {
        // Create a MediaStream from the remote track for <video srcObject>
        const stream = event.streams[0] ?? new MediaStream([event.track])
        remoteStream.value = stream
        if (!connected.value) {
          connected.value = true
        }

        // Detect when track ends
        event.track.onended = () => {
          console.log('[webrtc] video track ended')
          connected.value = false
        }
      }
    }

    // ── ICE candidate handler ───────────────────────────────────
    pc.onicecandidate = (event) => {
      if (event.candidate) {
        sendSignaling({
          type: 'ice_candidate_to_agent',
          target_machine: machineId,
          candidate: event.candidate.candidate,
          sdp_mid: event.candidate.sdpMid ?? null,
          sdp_m_line_index: event.candidate.sdpMLineIndex ?? null,
        } as SignalingMessage)
      }
    }

    // ── Connection state ────────────────────────────────────────
    pc.onconnectionstatechange = () => {
      console.log('[webrtc] connection state:', pc?.connectionState)
      if (pc?.connectionState === 'failed' || pc?.connectionState === 'disconnected') {
        connected.value = false
      }
    }

    // ── Create data channel for input events ────────────────────
    inputChannel = pc.createDataChannel('input', {
      ordered: true,
    })

    inputChannel.onopen = () => {
      console.log('[webrtc] input channel open — flushing', pendingInputEvents.length, 'pending events')
      for (const ev of pendingInputEvents) {
        inputChannel!.send(JSON.stringify(ev))
      }
      pendingInputEvents.length = 0
    }

    inputChannel.onclose = () => {
      console.log('[webrtc] input channel closed')
    }

    // ── Create SDP offer ────────────────────────────────────────
    const offer = await pc.createOffer()
    await pc.setLocalDescription(offer)

    // Send offer via signaling
    sendSignaling({
      type: 'web_rtc_answer',
      target_machine: machineId,
      sdp: pc.localDescription!.sdp,
    } as SignalingMessage)
  }

  function disconnect() {
    inputChannel?.close()
    pendingInputEvents.length = 0
    pc?.close()
    pc = null
    inputChannel = null
    currentMachineId = null
    connected.value = false
    remoteStream.value = null
  }

  function sendInputEvent(event: SignalingMessage) {
    if (inputChannel && inputChannel.readyState === 'open') {
      inputChannel.send(JSON.stringify(event))
    } else {
      // Channel not open yet — queue for later
      pendingInputEvents.push(event)
      if (pendingInputEvents.length === 1) {
        console.log('[webrtc] input channel not open, queueing events')
      }
    }
  }

  async function handleSignalingMessage(msg: SignalingMessage) {
    if (!pc) return

    try {
      switch (msg.type) {
        case 'web_rtc_answer_from_agent': {
          const answer: RTCSessionDescriptionInit = {
            type: 'answer',
            sdp: msg.sdp,
          }
          await pc.setRemoteDescription(new RTCSessionDescription(answer))
          console.log('[webrtc] remote description set (answer)')
          break
        }

        case 'ice_candidate_from_agent': {
          const candidate: RTCIceCandidateInit = {
            candidate: msg.candidate,
            sdpMid: msg.sdp_mid ?? undefined,
            sdpMLineIndex: msg.sdp_m_line_index ?? undefined,
          }
          await pc.addIceCandidate(new RTCIceCandidate(candidate))
          break
        }
      }
    } catch (e) {
      console.error('[webrtc] error handling signaling:', e)
    }
  }

  return {
    remoteStream,
    connected,
    connect,
    disconnect,
    sendInputEvent,
    handleSignalingMessage,
  }
}
