# Homebrew cask for AI Job Hunter.
#
# Releases ship signed macOS .dmg artifacts, so this cask is installable. The
# repo doubles as its own tap (it has this Casks/ directory):
#   brew tap saeedkolivand/ai-job-hunter-app https://github.com/saeedkolivand/ai-job-hunter-app
#   brew install --cask ai-job-hunter
#
# Maintenance:
#   • `version` tracks the latest release that ships macOS .dmg artifacts (the
#     installer build is manual, so not every release has them). The dmg assets
#     are named "macos-AI-Job-Hunter_<version>_<arch>.dmg" (the release
#     pipeline prefixes every artifact with its OS).
#   • When a new build publishes dmgs, bump `version` and refresh both per-arch
#     `sha256` values — `brew bump-cask-pr` does this, or read the assets'
#     sha256 digests from `gh release view v<version> --json assets`.

cask "ai-job-hunter" do
  version "0.147.0"
  sha256 arm:   "220ac6b2c2c839cf212ad09cc753cb764417d221b3904a66dffe93742902e51c",
         intel: "d4cd3bee92c30289d23b6bbfa74792ee7a73073faf86aef3298b3e7fc7a5b1c1"

  arch arm: "aarch64-apple-silicon", intel: "x64-intel"

  url "https://github.com/saeedkolivand/ai-job-hunter-app/releases/download/v#{version}/macos-AI-Job-Hunter_#{version}_#{arch}.dmg",
      verified: "github.com/saeedkolivand/ai-job-hunter-app/"
  name "AI Job Hunter"
  desc "Local-first, AI-native desktop assistant for job searching and applications"
  homepage "https://github.com/saeedkolivand/ai-job-hunter-app"

  app "AI Job Hunter.app"

  # The agent CLI is the SAME binary in argv mode (ADR-037): `ajh-tauri agent
  # <verb>` is detected from argv and short-circuits before the GUI (and before
  # the single-instance plugin) ever starts. Symlinking it onto PATH is what
  # makes the CLI reachable by a human — the app's own ~/.ajh-agent/agent.json
  # pointer solves discovery for *agents*, but nothing put the binary on PATH.
  # Verified against the shipped bundle, not assumed: the executable inside the
  # bundle is named after the Cargo [[bin]] (`ajh-tauri`), NOT productName.
  binary "#{appdir}/AI Job Hunter.app/Contents/MacOS/ajh-tauri"

  # The app is not notarized; clear the quarantine flag after install so
  # Gatekeeper doesn't refuse to open it.
  postflight do
    system_command "/usr/bin/xattr",
                   args: ["-cr", "#{appdir}/AI Job Hunter.app"],
                   sudo: false
  end

  zap trash: [
    "~/Library/Application Support/com.ajh.desktop",
    "~/Library/Caches/com.ajh.desktop",
    "~/Library/Preferences/com.ajh.desktop.plist",
    "~/Library/Saved Application State/com.ajh.desktop.savedState",
  ]
end
