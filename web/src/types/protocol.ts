// Protocol types — mirrors crates/shared/src/lib.rs

export type MachineId = string
export type SessionId = string

export interface MachineInfo {
  machine_id: MachineId
  machine_name: string
  os: string
  os_version: string
  display_width: number
  display_height: number
  online: boolean
}

export type SignalingMessage =
  // Agent → Server
  | { type: 'register'; machine_id: string; machine_name: string; os: string; os_version: string; display_width: number; display_height: number; token: string }
  | { type: 'heartbeat' }
  // Server → Agent
  | { type: 'registered'; session_id: SessionId }
  | { type: 'incoming_connection'; session_id: SessionId; web_client_id: SessionId }
  | { type: 'web_rtc_offer'; from_session: SessionId; sdp: string }
  | { type: 'ice_candidate'; from_session: SessionId; candidate: string; sdp_mid: string | null; sdp_m_line_index: number | null }
  | { type: 'peer_disconnected'; session_id: SessionId }
  // Web Client → Server
  | { type: 'list_machines' }
  | { type: 'connect_to_machine'; machine_id: MachineId }
  | { type: 'disconnect' }
  | { type: 'web_rtc_answer'; target_machine: MachineId; sdp: string }
  | { type: 'ice_candidate_to_agent'; target_machine: MachineId; candidate: string; sdp_mid: string | null; sdp_m_line_index: number | null }
  // Server → Web Client
  | { type: 'machine_list'; machines: MachineInfo[] }
  | { type: 'connected'; machine_id: MachineId; session_id: SessionId }
  | { type: 'connection_failed'; machine_id: MachineId; reason: string }
  | { type: 'machine_online'; machine: MachineInfo }
  | { type: 'machine_offline'; machine_id: MachineId }
  | { type: 'web_rtc_answer_from_agent'; machine_id: MachineId; sdp: string }
  | { type: 'ice_candidate_from_agent'; machine_id: MachineId; candidate: string; sdp_mid: string | null; sdp_m_line_index: number | null }
  // Input events
  | { type: 'key_event'; target: MachineId; pressed: boolean; key_code: number }
  | { type: 'mouse_move'; target: MachineId; x: number; y: number }
  | { type: 'mouse_button'; target: MachineId; button: number; pressed: boolean }
  | { type: 'mouse_scroll'; target: MachineId; dx: number; dy: number }
  // Error
  | { type: 'error'; code: string; message: string }
