#!/usr/bin/env bash
set -euo pipefail

# ──────────────────────────────────────────────────────────────────────────────
# Panorama — Interactive Install Script for macOS
#
# Installs all system dependencies, builds Rust services, sets up databases,
# creates launchd agents, and generates the .env configuration.
#
# Usage:  ./scripts/install-macos.sh
# ──────────────────────────────────────────────────────────────────────────────

PANORAMA_DIR="${PANORAMA_DIR:-$HOME/panorama}"
DATA_DIR="$PANORAMA_DIR/data"
LOG_DIR="$HOME/Library/Logs/panorama"
LOG_DB="$DATA_DIR/panorama_logs.db"
LAUNCH_AGENTS_DIR="$HOME/Library/LaunchAgents"

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
NC='\033[0m'

step=0
total_steps=13

banner() {
    echo ""
    echo -e "${CYAN}╔══════════════════════════════════════════════════════════════╗${NC}"
    echo -e "${CYAN}║              PANORAMA — macOS Install                       ║${NC}"
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

prompt_secret() {
    local msg="$1" default="$2"
    read -srp "$(echo -e "${YELLOW}$msg${NC} [${default:+****}]: ")" value
    echo ""
    echo "${value:-$default}"
}

check_macos() {
    if [[ "$(uname -s)" != "Darwin" ]]; then
        echo -e "${RED}Error: This script is for macOS only. Use install.sh for Linux.${NC}"
        exit 1
    fi
}

# ──────────────────────────────────────────────────────────────────────────────
# Main
# ──────────────────────────────────────────────────────────────────────────────

banner
check_macos

echo "This script will install Panorama and all its services on this Mac."
echo "Target directory: $PANORAMA_DIR"
echo ""
if ! prompt_yn "Continue with installation?"; then
    echo "Aborted."
    exit 0
fi

# ── Step 1: System packages ──────────────────────────────────────────────────
progress "Installing system dependencies (Homebrew)"

if ! command -v brew &>/dev/null; then
    echo -e "${YELLOW}Homebrew not found. Installing...${NC}"
    /bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"
    # Add brew to PATH for Apple Silicon Macs
    if [[ -f /opt/homebrew/bin/brew ]]; then
        eval "$(/opt/homebrew/bin/brew shellenv)"
    fi
fi

echo "Installing packages via Homebrew..."
brew install pkg-config openssl node python3 sqlite3 2>/dev/null || true

# Docker — check if Docker CLI is available (Docker Desktop or Colima)
if ! command -v docker &>/dev/null; then
    echo -e "${YELLOW}Docker not found.${NC}"
    echo "Install Docker Desktop from https://www.docker.com/products/docker-desktop/"
    echo "  or use Colima: brew install colima docker docker-compose && colima start"
    if ! prompt_yn "Continue without Docker? (Meilisearch/ChromaDB won't start)" "n"; then
        exit 1
    fi
else
    echo "Docker found: $(docker --version)"
fi

# Xcode Command Line Tools
if ! xcode-select -p &>/dev/null; then
    echo "Installing Xcode Command Line Tools..."
    xcode-select --install
    echo -e "${YELLOW}Please complete the Xcode CLT installation dialog, then re-run this script.${NC}"
    exit 1
fi

# ── Step 2: Rust toolchain ───────────────────────────────────────────────────
progress "Installing Rust toolchain"

if command -v rustup &>/dev/null; then
    echo "Rust already installed: $(rustc --version)"
    rustup update stable
else
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable
    source "$HOME/.cargo/env"
fi

# ── Step 3: Create directories ───────────────────────────────────────────────
progress "Creating directories"

mkdir -p "$PANORAMA_DIR" "$DATA_DIR" "$DATA_DIR/blobs" "$LOG_DIR" "$LAUNCH_AGENTS_DIR"

echo "  Install dir: $PANORAMA_DIR"
echo "  Data dir:    $DATA_DIR"
echo "  Log dir:     $LOG_DIR"

# ── Step 4: Clone or copy source ─────────────────────────────────────────────
progress "Setting up source code"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

if [[ -f "$SCRIPT_DIR/Cargo.toml" ]]; then
    echo "Copying from local source: $SCRIPT_DIR"
    if [[ "$SCRIPT_DIR" != "$PANORAMA_DIR" ]]; then
        rsync -a --exclude target --exclude .git "$SCRIPT_DIR/" "$PANORAMA_DIR/"
    else
        echo "Source and target are the same directory — skipping copy."
    fi
else
    REPO_URL=$(prompt_value "Git repository URL" "https://github.com/aVOIDSTARch/panorama.git")
    git clone --recursive "$REPO_URL" "$PANORAMA_DIR"
fi

# ── Step 5: Build Rust services ──────────────────────────────────────────────
progress "Building Rust services (release mode)"

# Set OpenSSL paths for Homebrew (macOS ships LibreSSL which some crates reject)
OPENSSL_PREFIX="$(brew --prefix openssl 2>/dev/null || echo "")"
if [[ -n "$OPENSSL_PREFIX" ]]; then
    export OPENSSL_DIR="$OPENSSL_PREFIX"
    export PKG_CONFIG_PATH="${OPENSSL_PREFIX}/lib/pkgconfig${PKG_CONFIG_PATH:+:$PKG_CONFIG_PATH}"
    echo "Using OpenSSL from: $OPENSSL_PREFIX"
fi

cd "$PANORAMA_DIR"
cargo build --release 2>&1 | tail -5

echo "Built binaries:"
ls -la target/release/{cloak-server,cortex-api,cortex-mcp,gateway,datastore,wheelhouse,admin-interface,analog-communications} 2>/dev/null || true

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

if [[ -f "$PANORAMA_DIR/services/cerebro/docker-compose.yml" ]] && command -v docker &>/dev/null; then
    if prompt_yn "Start Meilisearch + ChromaDB containers?"; then
        cd "$PANORAMA_DIR/services/cerebro"
        # Use 'docker compose' (v2 plugin) with fallback to 'docker-compose'
        if docker compose version &>/dev/null; then
            docker compose up -d
        elif command -v docker-compose &>/dev/null; then
            docker-compose up -d
        else
            echo -e "${YELLOW}Neither 'docker compose' nor 'docker-compose' found — skipping.${NC}"
        fi
    fi
else
    echo -e "${YELLOW}Docker not available or docker-compose.yml not found — skipping.${NC}"
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
WHEELHOUSE_PORT=$(prompt_value "Wheelhouse port" "8200")
ADMIN_PORT=$(prompt_value "Admin interface port" "8400")
ANALOG_PORT=$(prompt_value "Analog communications port" "8600")

echo ""
echo "External service configuration:"
CLOAK_INFISICAL_URL=$(prompt_value "Infisical URL" "https://infisical.example.com")
CLOAK_INFISICAL_TOKEN=$(prompt_secret "Infisical service token" "changeme")
CLOAK_INFISICAL_PROJECT=$(prompt_value "Infisical project ID" "placeholder")
CLOAK_INFISICAL_ENV=$(prompt_value "Infisical environment" "production")

echo ""
ADMIN_PASSWORD=$(prompt_secret "Admin interface password" "$(openssl rand -hex 16)")
MEILI_KEY=$(prompt_secret "Meilisearch master key" "$(openssl rand -hex 16)")

echo ""
echo "Analog Communications (SMS):"
TELNYX_PUBLIC_KEY=$(prompt_value "Telnyx Ed25519 public key (base64, or empty)" "")
ANALOG_OWNER_NUMBER=$(prompt_value "Owner phone number (E.164)" "")
ANALOG_ALLOWED_SENDERS=$(prompt_value "Allowed sender numbers (comma-separated)" "$ANALOG_OWNER_NUMBER")
OWNER_TOTP_SECRET=$(prompt_secret "Owner TOTP shared secret (base32, or empty)" "")

echo ""
echo "Security:"
TAILSCALE_INTERFACE=$(prompt_value "Tailscale interface name (or empty for 0.0.0.0)" "")
WEBAUTHN_RP_ID=$(prompt_value "WebAuthn RP ID (e.g. admin.yourdomain.ts.net, or empty)" "")
if [[ -n "$WEBAUTHN_RP_ID" ]]; then
    WEBAUTHN_RP_ORIGIN=$(prompt_value "WebAuthn RP origin URL" "https://$WEBAUTHN_RP_ID")
else
    WEBAUTHN_RP_ORIGIN=""
fi

# Write .env
cat > "$PANORAMA_DIR/.env" <<ENVEOF
# ── Panorama Environment Configuration ──
# Generated by install-macos.sh on $(date -Iseconds)

# Cloak
CLOAK_PORT=$CLOAK_PORT
CLOAK_INFISICAL_URL=$CLOAK_INFISICAL_URL
CLOAK_INFISICAL_TOKEN=$CLOAK_INFISICAL_TOKEN
CLOAK_INFISICAL_PROJECT=$CLOAK_INFISICAL_PROJECT
CLOAK_INFISICAL_ENV=$CLOAK_INFISICAL_ENV
SECRET_CACHE_TTL_SECS=3600
LOG_LEVEL=info

# Cortex
CORTEX_PORT=$CORTEX_PORT
CORTEX_MANIFEST=$PANORAMA_DIR/cortex-manifest.toml
CLOAK_URL=http://127.0.0.1:$CLOAK_PORT

# Datastore
DATASTORE_PORT=$DATASTORE_PORT
DATASTORE_DB_PATH=$DATA_DIR/datastore.db
DATASTORE_BLOB_ROOT=$DATA_DIR/blobs
DATASTORE_URL=http://127.0.0.1:$DATASTORE_PORT

# Gateway
GATEWAY_PORT=$GATEWAY_PORT
GATEWAY_ADMIN_PORT=$GATEWAY_ADMIN_PORT

# Wheelhouse
WHEELHOUSE_PORT=$WHEELHOUSE_PORT
WHEELHOUSE_URL=http://127.0.0.1:$WHEELHOUSE_PORT

# Admin Interface
ADMIN_PORT=$ADMIN_PORT
ADMIN_PASSWORD=$ADMIN_PASSWORD
CORTEX_URL=http://127.0.0.1:$CORTEX_PORT
EPISTEME_URL=http://127.0.0.1:8100
CEREBRO_URL=http://127.0.0.1:8101
GATEWAY_URL=http://127.0.0.1:$GATEWAY_PORT
LOG_DB_PATH=$LOG_DB

# Admin security
TAILSCALE_INTERFACE=$TAILSCALE_INTERFACE
WEBAUTHN_RP_ID=$WEBAUTHN_RP_ID
WEBAUTHN_RP_ORIGIN=$WEBAUTHN_RP_ORIGIN
WEBAUTHN_CREDENTIALS_PATH=$DATA_DIR/webauthn_credentials.json

# Analog Communications
ANALOG_PORT=$ANALOG_PORT
TELNYX_PUBLIC_KEY=$TELNYX_PUBLIC_KEY
ANALOG_OWNER_NUMBER=$ANALOG_OWNER_NUMBER
ANALOG_ALLOWED_SENDERS=$ANALOG_ALLOWED_SENDERS
OWNER_TOTP_SECRET=$OWNER_TOTP_SECRET

# Cerebro dependencies
MEILI_KEY=$MEILI_KEY
ENVEOF

chmod 600 "$PANORAMA_DIR/.env"
echo -e "${GREEN}.env written to $PANORAMA_DIR/.env${NC}"

# ── Step 9: Initialize databases ─────────────────────────────────────────────
progress "Initializing databases"

touch "$DATA_DIR/datastore.db"
echo "SQLite databases will be initialized on first service start (WAL mode)."

# ── Step 10: Create launchd agents ───────────────────────────────────────────
progress "Creating launchd agent plists"

# Helper: read .env file and emit plist EnvironmentVariables dict entries
env_to_plist_dict() {
    local env_file="$1"
    while IFS='=' read -r key value; do
        # Skip comments and blank lines
        [[ -z "$key" || "$key" =~ ^# ]] && continue
        # Trim leading/trailing whitespace
        key="$(echo "$key" | xargs)"
        value="$(echo "$value" | xargs)"
        [[ -z "$key" ]] && continue
        echo "            <key>$key</key>"
        echo "            <string>$value</string>"
    done < "$env_file"
}

generate_env_dict() {
    echo "        <key>EnvironmentVariables</key>"
    echo "        <dict>"
    env_to_plist_dict "$PANORAMA_DIR/.env"
    echo "        </dict>"
}

# Cache the env dict so we don't re-parse for each service
ENV_DICT="$(generate_env_dict)"

create_plist() {
    local label="$1" work_dir="$2" log_name="$3"
    shift 3
    local args=("$@")
    local plist_path="$LAUNCH_AGENTS_DIR/${label}.plist"

    # Build ProgramArguments XML
    local args_xml=""
    for arg in "${args[@]}"; do
        args_xml="$args_xml        <string>$arg</string>
"
    done

    cat > "$plist_path" <<PLISTEOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>$label</string>
    <key>ProgramArguments</key>
    <array>
$args_xml    </array>
    <key>WorkingDirectory</key>
    <string>$work_dir</string>
$ENV_DICT
    <key>KeepAlive</key>
    <dict>
        <key>SuccessfulExit</key>
        <false/>
    </dict>
    <key>RunAtLoad</key>
    <false/>
    <key>StandardOutPath</key>
    <string>$LOG_DIR/${log_name}.log</string>
    <key>StandardErrorPath</key>
    <string>$LOG_DIR/${log_name}.err.log</string>
</dict>
</plist>
PLISTEOF

    echo "  Created $plist_path"
}

# Standard Rust services (no extra args)
for bin in cloak-server cortex-api datastore wheelhouse admin-interface analog-communications; do
    create_plist "com.panorama.${bin}" "$PANORAMA_DIR" "$bin" \
        "$PANORAMA_DIR/target/release/$bin"
done

# Gateway needs "serve" subcommand with config path
create_plist "com.panorama.gateway" "$PANORAMA_DIR" "gateway" \
    "$PANORAMA_DIR/target/release/gateway" "serve" "-c" "$PANORAMA_DIR/crates/gateway/gateway.toml"

# Cerebro service (Node.js) — entrypoint is dist/src/api/server.js
NODE_BIN="$(command -v node)"
create_plist "com.panorama.cerebro" "$PANORAMA_DIR/services/cerebro" "cerebro" \
    "$NODE_BIN" "$PANORAMA_DIR/services/cerebro/dist/src/api/server.js"

# ── Step 11: Set permissions ─────────────────────────────────────────────────
progress "Setting file permissions"

chmod -R u+rw "$PANORAMA_DIR"
chmod 600 "$PANORAMA_DIR/.env"
echo "Permissions set for current user: $(whoami)"

# ── Step 12: Install MCP configuration ───────────────────────────────────────
progress "Setting up MCP tool server"

echo "cortex-mcp can be used as an MCP tool server for AI coding assistants."
echo "It reads cortex-manifest.toml and exposes Panorama services as callable tools."
echo ""
echo "To use with Claude Code, add to your MCP config:"
echo ""
echo "  {"
echo "    \"mcpServers\": {"
echo "      \"panorama\": {"
echo "        \"command\": \"$PANORAMA_DIR/target/release/cortex-mcp\","
echo "        \"env\": {"
echo "          \"CORTEX_URL\": \"http://127.0.0.1:$CORTEX_PORT\","
echo "          \"CORTEX_MANIFEST\": \"$PANORAMA_DIR/cortex-manifest.toml\""
echo "        }"
echo "      }"
echo "    }"
echo "  }"
echo ""

# ── Step 13: Start services ──────────────────────────────────────────────────
progress "Starting services"

BOOT_ORDER=(
    com.panorama.cloak-server
    com.panorama.datastore
    com.panorama.cortex-api
    com.panorama.cerebro
    com.panorama.gateway
    com.panorama.wheelhouse
    com.panorama.analog-communications
    com.panorama.admin-interface
)

if prompt_yn "Load and start all Panorama services now?"; then
    for svc in "${BOOT_ORDER[@]}"; do
        echo -n "  Starting $svc... "
        launchctl load "$LAUNCH_AGENTS_DIR/${svc}.plist" 2>/dev/null && \
            echo -e "${GREEN}OK${NC}" || echo -e "${YELLOW}FAILED (check $LOG_DIR/)${NC}"
        sleep 1
    done
else
    echo "Plists created but not loaded. To start manually:"
    echo "  launchctl load ~/Library/LaunchAgents/com.panorama.cloak-server.plist"
    echo "  launchctl load ~/Library/LaunchAgents/com.panorama.cortex-api.plist"
    echo "  ... etc"
    echo ""
    echo "To stop a service:"
    echo "  launchctl unload ~/Library/LaunchAgents/com.panorama.cloak-server.plist"
fi

# ── Done ─────────────────────────────────────────────────────────────────────
echo ""
echo -e "${CYAN}╔══════════════════════════════════════════════════════════════╗${NC}"
echo -e "${CYAN}║              Installation Complete (macOS)                  ║${NC}"
echo -e "${CYAN}╚══════════════════════════════════════════════════════════════╝${NC}"
echo ""
echo "  Config:        $PANORAMA_DIR/.env"
echo "  Data:          $DATA_DIR/"
echo "  Logs:          $LOG_DIR/"
echo "  Logs DB:       $LOG_DB"
echo "  Binaries:      $PANORAMA_DIR/target/release/"
echo "  Credentials:   $DATA_DIR/webauthn_credentials.json"
echo "  Plists:        $LAUNCH_AGENTS_DIR/com.panorama.*.plist"
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
echo "  MCP tools:     $PANORAMA_DIR/target/release/cortex-mcp (12 tools)"
echo ""
echo "  Commands:"
echo "    launchctl list | grep panorama"
echo "    tail -f $LOG_DIR/cloak-server.log"
echo "    sqlite3 $LOG_DB 'SELECT * FROM _system_logs ORDER BY timestamp DESC LIMIT 10'"
echo ""
echo -e "  ${YELLOW}POST-INSTALL CHECKLIST:${NC}"
echo "    1. Configure Infisical with real credentials in .env"
if [[ -z "$TAILSCALE_INTERFACE" ]]; then
echo "    2. Set up Tailscale: install, join tailnet, set TAILSCALE_INTERFACE in .env"
else
echo "    2. Tailscale interface configured: $TAILSCALE_INTERFACE ✓"
fi
if [[ -z "$WEBAUTHN_RP_ID" ]]; then
echo "    3. Set WEBAUTHN_RP_ID + WEBAUTHN_RP_ORIGIN in .env for YubiKey auth"
else
echo "    3. WebAuthn configured for $WEBAUTHN_RP_ID — register key at /auth/register ✓"
fi
if [[ -z "$TELNYX_PUBLIC_KEY" ]]; then
echo "    4. Set TELNYX_PUBLIC_KEY in .env for webhook signature verification"
else
echo "    4. Telnyx Ed25519 verification configured ✓"
fi
if [[ -z "$OWNER_TOTP_SECRET" ]]; then
echo "    5. Set OWNER_TOTP_SECRET in .env for owner TOTP verification"
else
echo "    5. Owner TOTP verification configured ✓"
fi
echo "    6. Set Telnyx webhook URL to https://<your-domain>/sms-inbound"
echo "    7. Initialize Episteme (Python service at :8100)"
echo ""
echo -e "  ${YELLOW}macOS-SPECIFIC NOTES:${NC}"
echo "    • After editing .env, reload services: launchctl unload then load each plist"
echo "    • Logs are in $LOG_DIR/ (not journalctl)"
echo "    • To uninstall services: launchctl unload ~/Library/LaunchAgents/com.panorama.*.plist"
echo ""
