#!/usr/bin/env bash
set -euo pipefail

# ──────────────────────────────────────────────────────────────────────────────
# Panorama — Interactive Install Script for Ubuntu
#
# Installs all system dependencies, builds Rust services, sets up databases,
# creates systemd units, and generates the .env configuration.
#
# Usage:  sudo ./scripts/install.sh
# ──────────────────────────────────────────────────────────────────────────────

PANORAMA_DIR="/srv/panorama"
PANORAMA_USER="panorama"
DATA_DIR="/srv/panorama/data"
LOG_DB="$DATA_DIR/panorama_logs.db"

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
NC='\033[0m'

step=0
total_steps=12

banner() {
    echo ""
    echo -e "${CYAN}╔══════════════════════════════════════════════════════════════╗${NC}"
    echo -e "${CYAN}║              PANORAMA — System Install                      ║${NC}"
    echo -e "${CYAN}╚══════════════════════════════════════════════════════════════╝${NC}"
    echo ""
}

progress() {
    step=$((step + 1))
    echo ""
    echo -e "${GREEN}[$step/$total_steps]${NC} $1"
    echo "────────────────────────────────────────"
}

prompt_yn() {
    local msg="$1" default="${2:-y}"
    if [[ "$default" == "y" ]]; then
        read -rp "$(echo -e "${YELLOW}$msg [Y/n]:${NC} ")" answer
        [[ -z "$answer" || "$answer" =~ ^[Yy] ]]
    else
        read -rp "$(echo -e "${YELLOW}$msg [y/N]:${NC} ")" answer
        [[ "$answer" =~ ^[Yy] ]]
    fi
}

prompt_value() {
    local msg="$1" default="$2"
    read -rp "$(echo -e "${YELLOW}$msg${NC} [${default}]: ")" value
    echo "${value:-$default}"
}

check_root() {
    if [[ $EUID -ne 0 ]]; then
        echo -e "${RED}Error: This script must be run as root (sudo).${NC}"
        exit 1
    fi
}

# ──────────────────────────────────────────────────────────────────────────────
# Main
# ──────────────────────────────────────────────────────────────────────────────

banner
check_root

echo "This script will install Panorama and all its services on this machine."
echo "Target directory: $PANORAMA_DIR"
echo ""
if ! prompt_yn "Continue with installation?"; then
    echo "Aborted."
    exit 0
fi

# ── Step 1: System packages ──────────────────────────────────────────────────
progress "Installing system dependencies"

apt-get update -qq
apt-get install -y -qq \
    build-essential \
    pkg-config \
    libssl-dev \
    curl \
    git \
    sqlite3 \
    docker.io \
    docker-compose \
    nodejs \
    npm \
    python3 \
    python3-pip \
    python3-venv

# ── Step 2: Rust toolchain ───────────────────────────────────────────────────
progress "Installing Rust toolchain"

if command -v rustup &>/dev/null; then
    echo "Rust already installed: $(rustc --version)"
    rustup update stable
else
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable
    source "$HOME/.cargo/env"
fi

# ── Step 3: Create service user and directories ─────────────────────────────
progress "Creating panorama user and directories"

if ! id "$PANORAMA_USER" &>/dev/null; then
    useradd --system --create-home --shell /bin/bash "$PANORAMA_USER"
    usermod -aG docker "$PANORAMA_USER"
fi

mkdir -p "$PANORAMA_DIR" "$DATA_DIR"
mkdir -p "$DATA_DIR/blobs"
mkdir -p /var/log/panorama

# ── Step 4: Clone or copy source ─────────────────────────────────────────────
progress "Setting up source code"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

if [[ -d "$SCRIPT_DIR/Cargo.toml" ]] || [[ -f "$SCRIPT_DIR/Cargo.toml" ]]; then
    echo "Copying from local source: $SCRIPT_DIR"
    rsync -a --exclude target --exclude .git "$SCRIPT_DIR/" "$PANORAMA_DIR/"
else
    REPO_URL=$(prompt_value "Git repository URL" "https://github.com/aVOIDSTARch/panorama.git")
    git clone --recursive "$REPO_URL" "$PANORAMA_DIR"
fi

# ── Step 5: Build Rust services ──────────────────────────────────────────────
progress "Building Rust services (release mode)"

cd "$PANORAMA_DIR"
cargo build --release 2>&1 | tail -5

echo "Built binaries:"
ls -la target/release/{cloak-server,cortex-api,gateway,datastore,wheelhouse,admin-interface,analog-communications} 2>/dev/null || true

# ── Step 6: Build Cerebro (TypeScript) ───────────────────────────────────────
progress "Building Cerebro (TypeScript)"

if [[ -f "$PANORAMA_DIR/services/cerebro/package.json" ]]; then
    cd "$PANORAMA_DIR/services/cerebro"
    npm ci --production=false
    npm run build
    echo "Cerebro built successfully"
else
    echo -e "${YELLOW}Cerebro source not found — skipping (initialize submodules later)${NC}"
fi

# ── Step 7: Cerebro dependencies (Meilisearch + ChromaDB) ───────────────────
progress "Starting Cerebro Docker dependencies"

if [[ -f "$PANORAMA_DIR/services/cerebro/docker-compose.yml" ]]; then
    if prompt_yn "Start Meilisearch + ChromaDB containers?"; then
        cd "$PANORAMA_DIR/services/cerebro"
        docker-compose up -d
    fi
fi

# ── Step 8: Interactive configuration ────────────────────────────────────────
progress "Generating configuration"

echo ""
echo "Configure service ports and credentials."
echo "(Press Enter to accept defaults)"
echo ""

CLOAK_PORT=$(prompt_value "Cloak port" "8300")
CORTEX_PORT=$(prompt_value "Cortex port" "9000")
DATASTORE_PORT=$(prompt_value "Datastore port" "8102")
GATEWAY_PORT=$(prompt_value "Gateway port" "8800")
GATEWAY_ADMIN_PORT=$(prompt_value "Gateway admin port" "8801")
WHEELHOUSE_PORT=$(prompt_value "Wheelhouse port" "8500")
ADMIN_PORT=$(prompt_value "Admin interface port" "8400")
ANALOG_PORT=$(prompt_value "Analog communications port" "8600")

echo ""
echo "External service configuration:"
INFISICAL_URL=$(prompt_value "Infisical URL" "https://infisical.example.com")
INFISICAL_TOKEN=$(prompt_value "Infisical service token" "changeme")
INFISICAL_PROJECT=$(prompt_value "Infisical project ID" "")
INFISICAL_ENV=$(prompt_value "Infisical environment" "production")

echo ""
ADMIN_PASSWORD=$(prompt_value "Admin interface password" "$(openssl rand -hex 16)")
MEILI_KEY=$(prompt_value "Meilisearch master key" "$(openssl rand -hex 16)")

echo ""
echo "Analog Communications (SMS):"
TELNYX_PUBLIC_KEY=$(prompt_value "Telnyx Ed25519 public key (or empty)" "")
ANALOG_OWNER_NUMBER=$(prompt_value "Owner phone number (E.164)" "")
ANALOG_ALLOWED_SENDERS=$(prompt_value "Allowed sender numbers (comma-separated)" "$ANALOG_OWNER_NUMBER")

# Write .env
cat > "$PANORAMA_DIR/.env" <<ENVEOF
# ── Panorama Environment Configuration ──
# Generated by install.sh on $(date -Iseconds)

# Cloak
CLOAK_PORT=$CLOAK_PORT
INFISICAL_URL=$INFISICAL_URL
INFISICAL_TOKEN=$INFISICAL_TOKEN
INFISICAL_PROJECT=$INFISICAL_PROJECT
INFISICAL_ENV=$INFISICAL_ENV
SECRET_CACHE_TTL_SECS=3600
LOG_LEVEL=info

# Cortex
CORTEX_PORT=$CORTEX_PORT
CORTEX_MANIFEST_PATH=$PANORAMA_DIR/cortex-manifest.toml
CLOAK_URL=http://127.0.0.1:$CLOAK_PORT

# Datastore
DATASTORE_PORT=$DATASTORE_PORT
DATASTORE_SQLITE_PATH=$DATA_DIR/datastore.db
DATASTORE_BLOB_PATH=$DATA_DIR/blobs

# Gateway
GATEWAY_PORT=$GATEWAY_PORT
GATEWAY_ADMIN_PORT=$GATEWAY_ADMIN_PORT

# Wheelhouse
WHEELHOUSE_PORT=$WHEELHOUSE_PORT

# Admin Interface
ADMIN_PORT=$ADMIN_PORT
ADMIN_PASSWORD=$ADMIN_PASSWORD
CLOAK_URL=http://127.0.0.1:$CLOAK_PORT
CORTEX_URL=http://127.0.0.1:$CORTEX_PORT
EPISTEME_URL=http://127.0.0.1:8100
CEREBRO_URL=http://127.0.0.1:8101
DATASTORE_URL=http://127.0.0.1:$DATASTORE_PORT
GATEWAY_URL=http://127.0.0.1:$GATEWAY_PORT

# Analog Communications
ANALOG_PORT=$ANALOG_PORT
TELNYX_PUBLIC_KEY=$TELNYX_PUBLIC_KEY
ANALOG_OWNER_NUMBER=$ANALOG_OWNER_NUMBER
ANALOG_ALLOWED_SENDERS=$ANALOG_ALLOWED_SENDERS

# Cerebro dependencies
MEILI_KEY=$MEILI_KEY
ENVEOF

chmod 600 "$PANORAMA_DIR/.env"
echo -e "${GREEN}.env written to $PANORAMA_DIR/.env${NC}"

# ── Step 9: Initialize databases ─────────────────────────────────────────────
progress "Initializing databases"

# The logging DB is auto-created by panorama-logging on first start.
# Touch the data directory so services can write.
touch "$DATA_DIR/datastore.db"
echo "SQLite databases will be initialized on first service start (WAL mode)."

# ── Step 10: Create systemd units ────────────────────────────────────────────
progress "Creating systemd service units"

RUST_SERVICES=(
    "cloak-server:Cloak Auth Server:$CLOAK_PORT"
    "cortex-api:Cortex Service Proxy:$CORTEX_PORT"
    "datastore:Datastore Storage:$DATASTORE_PORT"
    "gateway:Gateway LLM Router:$GATEWAY_PORT"
    "wheelhouse:Wheelhouse Agent Orchestrator:$WHEELHOUSE_PORT"
    "admin-interface:Admin Interface:$ADMIN_PORT"
    "analog-communications:Analog SMS Pipeline:$ANALOG_PORT"
)

for entry in "${RUST_SERVICES[@]}"; do
    IFS=: read -r bin desc port <<< "$entry"
    service_name="panorama-${bin}"

    cat > "/etc/systemd/system/${service_name}.service" <<UNITEOF
[Unit]
Description=Panorama — $desc
After=network.target
Wants=panorama-cloak-server.service

[Service]
Type=simple
User=$PANORAMA_USER
WorkingDirectory=$PANORAMA_DIR
EnvironmentFile=$PANORAMA_DIR/.env
ExecStart=$PANORAMA_DIR/target/release/$bin
Restart=on-failure
RestartSec=5
StandardOutput=journal
StandardError=journal
SyslogIdentifier=$service_name

[Install]
WantedBy=multi-user.target
UNITEOF

    echo "  Created ${service_name}.service"
done

# Cerebro service (Node.js)
cat > "/etc/systemd/system/panorama-cerebro.service" <<UNITEOF
[Unit]
Description=Panorama — Cerebro Knowledge Graph
After=network.target docker.service
Requires=docker.service

[Service]
Type=simple
User=$PANORAMA_USER
WorkingDirectory=$PANORAMA_DIR/services/cerebro
EnvironmentFile=$PANORAMA_DIR/.env
ExecStart=/usr/bin/node dist/api/server.js
Restart=on-failure
RestartSec=5

[Install]
WantedBy=multi-user.target
UNITEOF
echo "  Created panorama-cerebro.service"

systemctl daemon-reload

# ── Step 11: Set ownership ───────────────────────────────────────────────────
progress "Setting file ownership"

chown -R "$PANORAMA_USER:$PANORAMA_USER" "$PANORAMA_DIR"
chown -R "$PANORAMA_USER:$PANORAMA_USER" /var/log/panorama

# ── Step 12: Start services ──────────────────────────────────────────────────
progress "Starting services"

# Boot order matters: Cloak must be up before services that register with it.
BOOT_ORDER=(
    panorama-cloak-server
    panorama-datastore
    panorama-cortex-api
    panorama-cerebro
    panorama-gateway
    panorama-wheelhouse
    panorama-analog-communications
    panorama-admin-interface
)

if prompt_yn "Enable and start all Panorama services now?"; then
    for svc in "${BOOT_ORDER[@]}"; do
        echo -n "  Starting $svc... "
        systemctl enable "$svc" --quiet 2>/dev/null || true
        systemctl start "$svc" 2>/dev/null && echo -e "${GREEN}OK${NC}" || echo -e "${YELLOW}FAILED (check journalctl -u $svc)${NC}"
        sleep 1
    done
else
    echo "Services created but not started. Use:"
    echo "  sudo systemctl start panorama-cloak-server"
    echo "  sudo systemctl start panorama-cortex-api"
    echo "  ... etc"
fi

# ── Done ─────────────────────────────────────────────────────────────────────
echo ""
echo -e "${CYAN}╔══════════════════════════════════════════════════════════════╗${NC}"
echo -e "${CYAN}║              Installation Complete                          ║${NC}"
echo -e "${CYAN}╚══════════════════════════════════════════════════════════════╝${NC}"
echo ""
echo "  Config:        $PANORAMA_DIR/.env"
echo "  Data:          $DATA_DIR/"
echo "  Logs DB:       $LOG_DB"
echo "  Binaries:      $PANORAMA_DIR/target/release/"
echo ""
echo "  Service ports:"
echo "    Cloak:       http://127.0.0.1:$CLOAK_PORT"
echo "    Cortex:      http://127.0.0.1:$CORTEX_PORT"
echo "    Datastore:   http://127.0.0.1:$DATASTORE_PORT"
echo "    Gateway:     http://127.0.0.1:$GATEWAY_PORT"
echo "    Wheelhouse:  http://127.0.0.1:$WHEELHOUSE_PORT"
echo "    Admin:       http://127.0.0.1:$ADMIN_PORT"
echo "    Analog:      http://127.0.0.1:$ANALOG_PORT"
echo ""
echo "  Commands:"
echo "    sudo systemctl status 'panorama-*'"
echo "    sudo journalctl -u panorama-cloak-server -f"
echo "    sqlite3 $LOG_DB 'SELECT * FROM _system_logs ORDER BY timestamp DESC LIMIT 10'"
echo ""
echo -e "  ${YELLOW}REMAINING SETUP:${NC}"
echo "    1. Configure Infisical with real credentials in .env"
echo "    2. Set up Tailscale and restrict admin-interface to tailnet"
echo "    3. Configure YubiKey FIDO2 for admin auth (not yet implemented)"
echo "    4. Initialize Episteme (Python service at :8100)"
echo "    5. Set Telnyx webhook URL to http://<your-domain>/sms-inbound"
echo ""
