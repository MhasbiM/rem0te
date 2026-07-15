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

# ── Flutter Client ─────────────────────────────────────
flutter-install:
	cd flutter_client && flutter pub get

flutter-dev:
	cd flutter_client && flutter run -d macos

flutter-dev-linux:
	cd flutter_client && flutter run -d linux

flutter-build-macos:
	cd flutter_client && flutter build macos

flutter-build-linux:
	cd flutter_client && flutter build linux

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
