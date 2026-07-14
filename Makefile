.PHONY: all server admin client dev docker-build docker-up docker-down clean

# Default target
all: server admin

# ── Server ──────────────────────────────────────────────
server:
	cd server && cargo build --release

server-dev:
	cd server && RUST_LOG=info cargo run

# ── Admin Dashboard ─────────────────────────────────────
admin-install:
	cd admin && bun install

admin-dev:
	cd admin && bun run dev

admin-build:
	cd admin && bun run build

# ── Client (Tauri) ──────────────────────────────────────
client-install:
	cd client && bun install

client-dev:
	cd client && bun tauri dev

client-build-macos:
	cd client && cargo tauri build --target aarch64-apple-darwin

client-build-linux:
	cd client && cargo tauri build --target x86_64-unknown-linux-gnu

# ── Docker ──────────────────────────────────────────────
docker-build:
	docker compose build

docker-up:
	docker compose up -d

docker-down:
	docker compose down

docker-logs:
	docker compose logs -f

# ── Development (all at once) ───────────────────────────
dev:
	@echo "Starting development environment..."
	@echo "Run these in separate terminals:"
	@echo "  make server-dev"
	@echo "  make admin-dev"
	@echo "  make client-dev"

# ── Clean ──────────────────────────────────────────────
clean:
	cd server && cargo clean
	rm -rf admin/dist client/dist
	rm -rf client/src-tauri/target
