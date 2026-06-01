# Homebrew cask for AI Job Hunter Assistant.
#
# STATUS: ready to use *once the release pipeline publishes signed installers*.
# Today the GitHub releases carry version tags but no build artifacts, so there
# is no `.dmg` for Homebrew to download yet. To make this live:
#   1. Have the release workflow build + attach the macOS `.dmg`s to each GitHub
#      release (Tauri names them "AI Job Hunter Assistant_<version>_<arch>.dmg").
#   2. Either pin `sha256` per release (recommended — e.g. via `brew bump-cask-pr`
#      in CI) or keep `sha256 :no_check` for unverified installs.
#   3. Publish this file in a tap so users can `brew install --cask ai-job-hunter`
#      (a dedicated `saeedkolivand/homebrew-tap` repo, or tap this repo directly).
#
# Until artifacts exist, `brew` will fail to fetch — that's expected.

cask "ai-job-hunter" do
  version "0.47.0"
  sha256 :no_check # TODO: pin the per-release .dmg sha256 once builds are published

  arch arm: "aarch64", intel: "x64"

  url "https://github.com/saeedkolivand/ai-job-hunter-assistant-app/releases/download/v#{version}/AI%20Job%20Hunter%20Assistant_#{version}_#{arch}.dmg",
      verified: "github.com/saeedkolivand/ai-job-hunter-assistant-app/"
  name "AI Job Hunter Assistant"
  desc "Local-first, AI-native desktop assistant for job searching and applications"
  homepage "https://github.com/saeedkolivand/ai-job-hunter-assistant-app"

  app "AI Job Hunter Assistant.app"

  # The app is not notarized; clear the quarantine flag after install so
  # Gatekeeper doesn't refuse to open it.
  postflight do
    system_command "/usr/bin/xattr",
                   args: ["-cr", "#{appdir}/AI Job Hunter Assistant.app"],
                   sudo: false
  end

  zap trash: [
    "~/Library/Application Support/com.ajh.desktop",
    "~/Library/Caches/com.ajh.desktop",
    "~/Library/Preferences/com.ajh.desktop.plist",
    "~/Library/Saved Application State/com.ajh.desktop.savedState",
  ]
end
