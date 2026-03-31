#!/usr/bin/env bash
# Run this script once to set up the Cloudflare tunnel.
# Requires: cloudflared installed, logged in to Cloudflare account.
set -euo pipefail

eval "$(/opt/homebrew/bin/brew shellenv)"

echo "==> Step 1: Login to Cloudflare (opens browser)"
cloudflared tunnel login

echo ""
echo "==> Step 2: Create tunnel named 'panorama'"
cloudflared tunnel create panorama

echo ""
echo "==> Step 3: Copy credentials file to expected path"
TUNNEL_ID=$(cloudflared tunnel list --output json | python3 -c "import sys,json; t=[x for x in json.load(sys.stdin) if x['name']=='panorama'][0]; print(t['id'])")
echo "Tunnel ID: $TUNNEL_ID"
cp ~/.cloudflared/${TUNNEL_ID}.json ~/.cloudflared/panorama.json

echo ""
echo "==> Step 4: Route DNS for both hostnames"
cloudflared tunnel route dns panorama sms.idea.flickersong.io
cloudflared tunnel route dns panorama admin.idea.flickersong.io

echo ""
echo "==> Step 5: Install as launchd service (runs on startup)"
sudo cloudflared service install --config ~/.cloudflared/panorama-config.yml

echo ""
echo "Done! Tunnel installed as system service."
echo "Start it: sudo launchctl start com.cloudflare.cloudflared"
echo "Test:     curl https://sms.idea.flickersong.io/health"
echo ""
echo "Telnyx webhook URL: https://sms.idea.flickersong.io/sms-inbound"
