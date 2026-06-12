# Homebrew cask for AI Job Hunter Assistant.
#
# Releases ship signed macOS .dmg artifacts, so this cask is installable. The
# repo doubles as its own tap (it has this Casks/ directory):
#   brew tap saeedkolivand/ai-job-hunter-assistant-app https://github.com/saeedkolivand/ai-job-hunter-assistant-app
#   brew install --cask ai-job-hunter
#
# Maintenance:
#   • `version` tracks the latest release that ships macOS .dmg artifacts (the
#     installer build is manual, so not every release has them). The dmg assets
#     are named "AI-Job-Hunter-Assistant_<version>_<arch>.dmg".
#   • When a new build publishes dmgs, bump `version` and refresh both per-arch
#     `sha256` values — `brew bump-cask-pr` does this, or read the assets'
#     sha256 digests from `gh release view v<version> --json assets`.

cask "ai-job-hunter" do
  version "0.93.0"
  sha256 arm:   "b8efd9a18a362865d167d10f739cdbbb42946cd32e3df645fe32b3a7534003ae",
         intel: "cf72b15e01dfdec3668822f8968534100b284ad28f8b5339406ac3bb16bf95fb"

  arch arm: "aarch64-apple-silicon", intel: "x64-intel"

  url "https://github.com/saeedkolivand/ai-job-hunter-assistant-app/releases/download/v#{version}/AI-Job-Hunter-Assistant_#{version}_#{arch}.dmg",
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
