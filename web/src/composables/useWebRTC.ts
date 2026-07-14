import { ref, type Ref } from 'vue'
import type { MachineId, SignalingMessage } from '@/types/protocol'

export interface UseWebRTCReturn {
  currentFrameUrl: Ref<string | null>
  connected: Ref<boolean>
  connect: (machineId: MachineId) => Promise<void>
  disconnect: () => void
  sendInputEvent: (event: SignalingMessage) => void
  handleSignalingMessage: (msg: SignalingMessage) => void
}

export function useWebRTC(
  sendSignaling: (msg: SignalingMessage) => void,
): UseWebRTCReturn {
  const currentFrameUrl = ref<string | null>(null)
  const connected = ref(false)

  let pc: RTCPeerConnection | null = null
  let inputChannel: RTCDataChannel | null = null
  let currentMachineId: MachineId | null = null

  // Revoke old blob URLs to prevent memory leaks
  let lastObjectUrl: string | null = null

  const iceServers: RTCConfiguration = {
    iceServers: [
      { urls: 'stun:stun.l.google.com:19302' },
    ],
  }

  /** Render a complete JPEG frame to the img element. */
  function renderFrame(data: Uint8Array) {
    if (lastObjectUrl) {
      URL.revokeObjectURL(lastObjectUrl)
    }
    const blob = new Blob([data as BlobPart], { type: 'image/jpeg' })
    lastObjectUrl = URL.createObjectURL(blob)
    currentFrameUrl.value = lastObjectUrl
    if (!connected.value) {
      connected.value = true
    }
  }

  async function connect(machineId: MachineId) {
    currentMachineId = machineId

    pc = new RTCPeerConnection(iceServers)

    // ── Incoming data channel handler (video from agent) ──────────
    pc.ondatachannel = (event) => {
      const channel = event.channel
      console.log('[webrtc] incoming data channel:', channel.label)

      if (channel.label === 'video') {
        channel.binaryType = 'arraybuffer'

        // Chunk reassembly state
        const pendingFrames = new Map<number, {
          total: number
          chunks: Map<number, Uint8Array>
        }>()
        let lastCompleteFrame = 0

        channel.onmessage = (msg) => {
          if (msg.data instanceof ArrayBuffer) {
            const buf = new Uint8Array(msg.data)
            if (buf.length < 8) return // too small

            // Parse header: frame_id(u32 BE) + chunk_idx(u16 BE) + total(u16 BE)
            const view = new DataView(buf.buffer)
            const frameId = view.getUint32(0, false)
            const chunkIdx = view.getUint16(4, false)
            const total = view.getUint16(6, false)
            const payload = buf.slice(8)

            // Single-chunk frame (common case): render immediately
            if (total === 1) {
              renderFrame(payload)
              lastCompleteFrame = frameId
              return
            }

            // Multi-chunk: collect
            let frame = pendingFrames.get(frameId)
            if (!frame || frame.total !== total) {
              frame = { total, chunks: new Map() }
              pendingFrames.set(frameId, frame)
            }
            frame.chunks.set(chunkIdx, payload)

            // Check if complete
            if (frame.chunks.size === total) {
              // Concatenate chunks in order
              const parts: Uint8Array[] = []
              for (let i = 0; i < total; i++) {
                const c = frame.chunks.get(i)
                if (c) parts.push(c)
              }
              const complete = new Uint8Array(parts.reduce((s, p) => s + p.length, 0))
              let offset = 0
              for (const p of parts) {
                complete.set(p, offset)
                offset += p.length
              }

              pendingFrames.delete(frameId)
              if (frameId > lastCompleteFrame) {
                renderFrame(complete)
                lastCompleteFrame = frameId
              }

              // Cleanup old incomplete frames (>5 frames behind)
              for (const [id] of pendingFrames) {
                if (id < lastCompleteFrame - 5) pendingFrames.delete(id)
              }
            }
          }
        }

        channel.onopen = () => {
          console.log('[webrtc] video channel open')
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
      console.log('[webrtc] input channel open')
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
    if (lastObjectUrl) {
      URL.revokeObjectURL(lastObjectUrl)
      lastObjectUrl = null
    }
    pc?.close()
    pc = null
    inputChannel = null
    currentMachineId = null
    connected.value = false
    currentFrameUrl.value = null
  }

  function sendInputEvent(event: SignalingMessage) {
    if (inputChannel && inputChannel.readyState === 'open') {
      inputChannel.send(JSON.stringify(event))
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
    currentFrameUrl,
    connected,
    connect,
    disconnect,
    sendInputEvent,
    handleSignalingMessage,
  }
}
