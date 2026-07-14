import { ref, type Ref } from 'vue'
import type { MachineId, SignalingMessage } from '@/types/protocol'

export interface UseWebRTCReturn {
  remoteStream: Ref<MediaStream | null>
  connected: Ref<boolean>
  connect: (machineId: MachineId) => Promise<void>
  disconnect: () => void
  sendInputEvent: (event: SignalingMessage) => void
  // Called by signaling when SDP/ICE arrives from agent
  handleSignalingMessage: (msg: SignalingMessage) => void
}

export function useWebRTC(
  sendSignaling: (msg: SignalingMessage) => void,
): UseWebRTCReturn {
  const remoteStream = ref<MediaStream | null>(null)
  const connected = ref(false)

  let pc: RTCPeerConnection | null = null
  let dataChannel: RTCDataChannel | null = null
  let currentMachineId: MachineId | null = null

  const iceServers: RTCConfiguration = {
    iceServers: [
      { urls: 'stun:stun.l.google.com:19302' },
    ],
  }

  async function connect(machineId: MachineId) {
    currentMachineId = machineId

    pc = new RTCPeerConnection(iceServers)

    // ── Remote stream handler ───────────────────────────────────
    pc.ontrack = (event) => {
      console.log('[webrtc] remote track received:', event.track.kind)
      if (event.streams && event.streams[0]) {
        remoteStream.value = event.streams[0]
        connected.value = true
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
    dataChannel = pc.createDataChannel('input', {
      ordered: true,
    })

    dataChannel.onopen = () => {
      console.log('[webrtc] data channel open')
    }

    dataChannel.onclose = () => {
      console.log('[webrtc] data channel closed')
    }

    // ── Create SDP offer ────────────────────────────────────────
    const offer = await pc.createOffer({
      offerToReceiveVideo: true,
      offerToReceiveAudio: false,
    })
    await pc.setLocalDescription(offer)

    // Send offer via signaling
    sendSignaling({
      type: 'webrtc_answer',
      target_machine: machineId,
      sdp: pc.localDescription!.sdp,
    } as SignalingMessage)
  }

  function disconnect() {
    dataChannel?.close()
    pc?.close()
    pc = null
    dataChannel = null
    currentMachineId = null
    connected.value = false
    remoteStream.value = null
  }

  function sendInputEvent(event: SignalingMessage) {
    if (dataChannel && dataChannel.readyState === 'open') {
      dataChannel.send(JSON.stringify(event))
    }
  }

  async function handleSignalingMessage(msg: SignalingMessage) {
    if (!pc) return

    try {
      switch (msg.type) {
        case 'webrtc_answer_from_agent': {
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
