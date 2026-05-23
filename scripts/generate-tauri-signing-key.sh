#!/usr/bin/env bash
# generate-tauri-signing-key.sh
#
# Generates a minisign key pair for signing Tauri update artifacts.
# Run this once to set up the CI pipeline.
#
# Prerequisites: cargo + tauri-cli installed
#   cargo install tauri-cli
#
# Usage:
#   bash scripts/generate-tauri-signing-key.sh
#
# After running this script:
#   1. Add TAURI_SIGNING_PRIVATE_KEY to GitHub Secrets:
#         Content: the entire contents of ~/.tauri/ajh.key
#   2. Add TAURI_SIGNING_PRIVATE_KEY_PASSWORD to GitHub Secrets:
#         Content: the password you entered (or empty string if none)
#   3. Add TAURI_SIGNING_PUBLIC_KEY to GitHub Secrets:
#         Content: the public key printed below (also in ~/.tauri/ajh.key.pub)
#   4. In apps/tauri/src-tauri/tauri.conf.json, replace the placeholder
#      pubkey with the real public key from TAURI_SIGNING_PUBLIC_KEY.
#      The CI pipeline injects this automatically via the sync-tauri-version
#      script, but the value in the repo must be valid base64.

set -euo pipefail

KEY_PATH="${HOME}/.tauri/ajh"

# Check if --force flag is passed
FORCE_FLAG=""
if [[ "${1:-}" == "--force" ]]; then
  FORCE_FLAG="--force"
  echo "Force flag enabled - will overwrite existing key pair"
fi

echo "Generating Tauri signing key at ${KEY_PATH}.key ..."
cargo tauri signer generate -w "${KEY_PATH}.key" ${FORCE_FLAG}

echo ""
echo "✅ Key pair generated."
echo ""
echo "Public key (also in ${KEY_PATH}.key.pub):"
cat "${KEY_PATH}.key.pub"
echo ""
echo "Next steps:"
echo "  1. Add the private key file contents as GitHub secret TAURI_SIGNING_PRIVATE_KEY"
echo "  2. Add your key password as TAURI_SIGNING_PRIVATE_KEY_PASSWORD (empty string if none)"
echo "  3. Add the public key above as TAURI_SIGNING_PUBLIC_KEY"
echo "  4. The CI pipeline reads TAURI_SIGNING_PUBLIC_KEY and injects it into tauri.conf.json"
