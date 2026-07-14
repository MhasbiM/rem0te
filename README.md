# rem0te — Remote Desktop over WebRTC

Remote desktop solution for Linux (Wayland) and macOS, controlled via web browser.

## Architecture

```
┌──────────────┐     WebRTC      ┌─────────────────┐     WebSocket     ┌──────────────┐
│  Web Browser  │◄──────────────►│  Signaling       │◄───────────────►│  Remote Agent │
│  (Vue 3 SPA)  │                │  Server (Rust)   │                  │  (Rust Binary)│
└──────────────┘                 └─────────────────┘                  └──────────────┘
                                        │
                                        ▼
                                 ┌─────────────┐
                                 │  TURN/STUN   │
                                 │  (optional)  │
                                 └─────────────┘
```

## Project Structure

```
rem0te/
├── crates/
│   ├── shared/        # Protocol types & shared utilities
│   ├── server/        # Signaling/relay server (axum + WebSocket)
│   └── client/        # Remote desktop agent (runs on remote PC)
├── web/               # Vue 3 + TypeScript frontend
├── Cargo.toml         # Rust workspace root
└── README.md
```

## Quick Start

### Prerequisites
- Rust 1.78+
- Node.js 20+
- Linux: PipeWire + xdg-desktop-portal
- macOS: Screen Recording permission

### Server

```bash
cargo run -p rem0te-server
# Server starts on http://0.0.0.0:8080
```

### Client (Remote Agent)

```bash
# Run on the remote machine
cargo run -p rem0te-client -- --server ws://your-server:8080/ws --token your-secret-token
```

### Web Frontend

```bash
cd web
npm install
npm run dev
```

## Technology Stack

| Layer | Technology |
|-------|-----------|
| Language | Rust (static binary) |
| Screen Capture (Linux) | PipeWire + xdg-desktop-portal |
| Screen Capture (macOS) | CoreGraphics / ScreenCaptureKit |
| Input (Linux) | libei / uinput |
| Input (macOS) | CGEvent |
| Streaming | WebRTC (webrtc-rs) |
| Signaling | WebSocket (axum) |
| Frontend | Vue 3 + TypeScript + Vite |

## License

MIT
