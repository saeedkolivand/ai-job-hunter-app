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
#   3. Commit the PUBLIC key into apps/desktop/src-tauri/tauri.conf.json at
#      plugins.updater.pubkey — paste the contents of ~/.tauri/ajh.key.pub
#      verbatim. The public key is NOT a secret; it is the single source of
#      truth the app verifies updates against, so it lives in the repo (CI does
#      not inject it). It must match the private key from step 1, or the
#      auto-updater breaks. See docs/DEPLOYMENT.md (Updater signing keys).

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
echo "  3. Paste the public key above into plugins.updater.pubkey in"
echo "     apps/desktop/src-tauri/tauri.conf.json and commit it (it is not a secret)"
echo "  4. CI verifies the committed pubkey matches the signing key on every release"
