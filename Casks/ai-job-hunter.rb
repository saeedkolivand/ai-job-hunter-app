# Homebrew cask for AI Job Hunter Assistant.
#
# Releases ship signed macOS .dmg artifacts, so this cask is installable. The
# repo doubles as its own tap (it has this Casks/ directory):
#   brew tap saeedkolivand/ai-job-hunter-assistant-app https://github.com/saeedkolivand/ai-job-hunter-assistant-app
#   brew install --cask ai-job-hunter
#
# Maintenance:
#   • Keep `version` in sync with the latest GitHub release tag — CI can run
#     `brew bump-cask-pr`. The dmg assets are named
#     "AI-Job-Hunter-Assistant_<version>_<arch>.dmg".
#   • `sha256 :no_check` skips hash verification; pin per-arch shas for a
#     verified install once they are published with the release.

cask "ai-job-hunter" do
  version "0.52.0"
  sha256 :no_check # TODO: pin per-arch .dmg sha256 for verified installs

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
