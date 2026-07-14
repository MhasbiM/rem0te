# ─── rem0te Makefile ────────────────────────────────────────────────────
# Usage:
#   make server        Start signaling server
#   make client        Start remote agent (connect to local server)
#   make web           Start Vue dev server (http://localhost:5173)
#   make all           Start server + client + web in parallel
#   make build         Build all Rust crates
#   make build-web     Build Vue frontend for production
#   make clean         Remove build artifacts
#   make install-deps  Install all dependencies

# ── Configuration ──────────────────────────────────────────────────────

SERVER_BIN     ?= rem0te-server
CLIENT_BIN     ?= rem0te-client
SERVER_URL     ?= ws://localhost:8080/ws
SERVER_BIND    ?= 0.0.0.0:8080
SERVER_TOKEN   ?= changeme
CLIENT_TOKEN   ?= changeme
CLIENT_NAME    ?= $(shell hostname)

# ── Colors ─────────────────────────────────────────────────────────────
GREEN  := \033[0;32m
CYAN   := \033[0;36m
YELLOW := \033[0;33m
RESET  := \033[0m

# ── Default target ─────────────────────────────────────────────────────

.PHONY: help
help: ## Show this help
	@echo "$(CYAN)rem0te — Remote Desktop over WebRTC$(RESET)"
	@echo ""
	@echo "$(GREEN)Usage: make [target]$(RESET)"
	@echo ""
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | \
		awk 'BEGIN {FS = ":.*?## "}; {printf "  $(YELLOW)%-18s$(RESET) %s\n", $$1, $$2}'

# ── Run targets ────────────────────────────────────────────────────────

.PHONY: server
server: ## Start the signaling server
	@echo "$(GREEN)[server]$(RESET) Starting on $(SERVER_BIND)..."
	cargo run -p $(SERVER_BIN) -- \
		--bind $(SERVER_BIND) \
		--token $(SERVER_TOKEN)

.PHONY: client
client: ## Start the remote agent
	@echo "$(GREEN)[client]$(RESET) Connecting to $(SERVER_URL)..."
	cargo run -p $(CLIENT_BIN) -- \
		--server $(SERVER_URL) \
		--token $(CLIENT_TOKEN) \
		--name $(CLIENT_NAME)

.PHONY: web
web: ## Start the Vue dev server
	@echo "$(GREEN)[web]$(RESET) Starting at http://localhost:5173..."
	cd web && npm run dev

.PHONY: all
all: ## Start server + client + web (requires 3 terminals or tmux)
	@echo "$(CYAN)[all]$(RESET) Starting all services..."
	@echo "  Run 'make tmux' to launch in tmux panes, or open 3 terminals."
	@echo ""
	@$(MAKE) server & \
	sleep 2 && \
	$(MAKE) client & \
	$(MAKE) web & \
	wait

.PHONY: tmux
tmux: ## Start all services in a tmux session
	@tmux new-session -d -s rem0te
	@tmux rename-window -t rem0te:0 'rem0te'
	@tmux send-keys -t rem0te:0.0 'make server' Enter
	@tmux split-window -h -t rem0te:0
	@tmux send-keys -t rem0te:0.1 'make client' Enter
	@tmux split-window -v -t rem0te:0.0
	@tmux send-keys -t rem0te:0.2 'make web' Enter
	@tmux select-layout -t rem0te:0 even-horizontal
	@tmux -2 attach-session -t rem0te

# ── Build targets ──────────────────────────────────────────────────────

.PHONY: build
build: ## Build all Rust crates (release)
	@echo "$(GREEN)[build]$(RESET) Building Rust crates..."
	cargo build --release -p $(SERVER_BIN) -p $(CLIENT_BIN)

.PHONY: build-debug
build-debug: ## Build all Rust crates (debug)
	@echo "$(GREEN)[build-debug]$(RESET) Building Rust crates (debug)..."
	cargo build -p $(SERVER_BIN) -p $(CLIENT_BIN)

.PHONY: build-web
build-web: ## Build Vue frontend for production
	@echo "$(GREEN)[build-web]$(RESET) Building Vue frontend..."
	cd web && npm run build

.PHONY: build-all
build-all: build build-web ## Build everything (Rust release + Vue production)

# ── Utility targets ────────────────────────────────────────────────────

.PHONY: check
check: ## Type-check and lint everything
	@echo "$(GREEN)[check]$(RESET) Checking Rust..."
	cargo check --workspace
	@echo "$(GREEN)[check]$(RESET) Checking TypeScript..."
	cd web && npx vue-tsc --noEmit

.PHONY: test
test: ## Run all tests
	@echo "$(GREEN)[test]$(RESET) Running Rust tests..."
	cargo test --workspace

.PHONY: clean
clean: ## Remove build artifacts
	@echo "$(YELLOW)[clean]$(RESET) Cleaning..."
	cargo clean
	rm -rf web/dist web/node_modules/.vite

.PHONY: install-deps
install-deps: ## Install all dependencies
	@echo "$(GREEN)[deps]$(RESET) Installing npm packages..."
	cd web && npm install
	@echo "$(GREEN)[deps]$(RESET) Rust dependencies will be fetched on first build."

.PHONY: fmt
fmt: ## Format all code
	@echo "$(GREEN)[fmt]$(RESET) Formatting Rust..."
	cargo fmt --all
	@echo "$(GREEN)[fmt]$(RESET) Formatting frontend..."
	cd web && npx prettier --write 'src/**/*.{ts,vue,css}' 2>/dev/null || true

.PHONY: lint
lint: ## Lint all code
	@echo "$(GREEN)[lint]$(RESET) Linting Rust..."
	cargo clippy --workspace -- -D warnings 2>/dev/null || cargo clippy --workspace
	@echo "$(GREEN)[lint]$(RESET) Linting frontend..."
	cd web && npx eslint 'src/**/*.{ts,vue}' 2>/dev/null || true

# ── Dev helpers ────────────────────────────────────────────────────────

.PHONY: dev
dev: check ## Quick dev cycle: check + debug build
	@echo "$(GREEN)[dev]$(RESET) Building debug..."
	cargo build -p $(SERVER_BIN) -p $(CLIENT_BIN)

.PHONY: watch
watch: ## Watch Rust sources and rebuild on change
	@echo "$(GREEN)[watch]$(RESET) Watching for changes..."
	cargo watch -x "check --workspace" -x "build -p $(SERVER_BIN) -p $(CLIENT_BIN)"
