# rem0te - Open Source Remote Desktop

A self-hosted remote desktop solution similar to RustDesk, built with Rust and React.

## Architecture

```
┌──────────────────────────────────────────────────────────────┐
│                      rem0te Server (Rust)                     │
│  ┌─────────────┐  ┌──────────┐  ┌────────────┐  ┌─────────┐ │
│  │  Signaling   │  │  Relay   │  │  REST API  │  │  Admin  │ │
│  │  (TCP+WS)    │  │  (TCP)   │  │  (Actix)   │  │  (React)│ │
│  └──────┬───────┘  └────┬─────┘  └─────┬──────┘  └────┬────┘ │
│         │               │              │              │       │
└─────────┼───────────────┼──────────────┼──────────────┼───────┘
          │               │              │              │
    ┌─────▼───────────────▼──────┐  ┌───▼──────────────▼──┐
    │     rem0te Client          │  │  rem0te Admin        │
    │  (Tauri + React)           │  │  (React + Vite)      │
    │  ┌──────────┐ ┌──────────┐ │  │  Browser-based       │
    │  │ Screen   │ │ File     │ │  │  Dashboard           │
    │  │ Capture  │ │ Transfer │ │  └─────────────────────┘
    │  └──────────┘ └──────────┘ │
    └────────────────────────────┘
```

## Features

- **Remote Desktop**: macOS → Linux (X11 & Wayland support)
- **File Transfer**: Upload & download files between machines
- **Self-hosted**: All data goes through your own server
- **Admin Dashboard**: Monitor connections, manage users
- **Cross-platform**: macOS and Linux clients

## Project Structure

```
rem0te/
├── server/           # Rust signaling + relay + API server
│   ├── Cargo.toml
│   ├── Dockerfile
│   └── src/
│       ├── main.rs
│       ├── config.rs
│       ├── signaling.rs    # WebSocket & TCP peer discovery
│       ├── relay.rs        # TCP relay for NAT traversal
│       ├── api.rs          # REST API
│       └── api/
│           ├── auth.rs     # JWT authentication
│           ├── users.rs    # User management
│           ├── connections.rs  # Peer monitoring
│           └── file_transfer.rs # File transfer API
├── admin/            # React admin dashboard (Vite + Bun)
│   ├── package.json
│   ├── vite.config.ts
│   └── src/
│       ├── App.tsx
│       ├── pages/
│       │   ├── Login.tsx
│       │   ├── Dashboard.tsx
│       │   ├── Peers.tsx
│       │   ├── Users.tsx
│       │   ├── FileTransfers.tsx
│       │   └── Settings.tsx
│       └── components/
│           ├── AuthContext.tsx
│           └── Layout.tsx
├── flutter_client/    # Flutter desktop app (macOS + Linux)
│   ├── pubspec.yaml
│   └── lib/
│       ├── main.dart
│       ├── pages/
│       │   ├── connect_page.dart
│       │   └── remote_page.dart
│       └── services/
│           ├── signaling_service.dart
│           └── relay_service.dart
├── Cargo.toml        # Rust workspace
└── docker-compose.yml
```

## Quick Start

### Prerequisites

- **Rust** (1.75+): https://rustup.rs
- **Bun** (1.1+): https://bun.sh
- **Tauri CLI**: `cargo install tauri-cli --version "^2.0"`
- **Docker** (optional, for server deployment)

### 1. Run the Server

```bash
# Using Docker (recommended)
docker compose up -d

# Or run natively
cd server
RUST_LOG=info cargo run
```

The server starts on:
- `:8080` — REST API + Admin dashboard (serve admin dist here)
- `:21116` — TCP Signaling
- `:21117` — TCP Relay  
- `:21118` — WebSocket Signaling

### 2. Run the Admin Dashboard

```bash
cd admin
bun install
bun run dev
```

Open http://localhost:3000 and login with:
- **Username**: `admin`
- **Password**: `admin123`

### 3. Run the Flutter Client

```bash
cd flutter_client
flutter pub get
flutter run -d macos    # macOS
flutter run -d linux    # Linux
```

## Platform Support

### Screen Capture

| Platform | Method | Status |
|----------|--------|--------|
| macOS | CoreGraphics (CGDisplay) | ✅ Supported |
| Linux X11 | x11rb (GetImage) | ✅ Supported |
| Linux Wayland | PipeWire / xdg-portal | 🚧 In progress |

### Remote Control

| Feature | macOS Client | Linux Client |
|---------|-------------|-------------|
| View remote screen | ✅ | ✅ |
| Mouse input | ✅ | ✅ |
| Keyboard input | ✅ | ✅ |
| Clipboard sync | 🚧 | 🚧 |
| Multi-monitor | 🚧 | 🚧 |

## Security

- JWT-based authentication for API access
- bcrypt password hashing (12 rounds)
- All traffic routed through your self-hosted relay
- Change default credentials in production:
  ```bash
  export REM0TE_JWT_SECRET="your-secret-here"
  ```

## Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `REM0TE_API_PORT` | `8080` | REST API port |
| `REM0TE_SIGNALING_PORT` | `21116` | TCP signaling port |
| `REM0TE_RELAY_PORT` | `21117` | TCP relay port |
| `REM0TE_WS_PORT` | `21118` | WebSocket signaling |
| `REM0TE_JWT_SECRET` | (dev default) | JWT signing secret |

## Development

```bash
# Server
cd server && cargo build --release

# Admin
cd admin && bun run build

# Flutter Client macOS
cd flutter_client && flutter build macos

# Flutter Client Linux
cd flutter_client && flutter build linux
```

## License

MIT
