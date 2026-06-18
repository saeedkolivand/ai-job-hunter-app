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
  version "0.109.3"
  sha256 arm:   "10d2c6684b51dd4176a3ec4e08a214032c4e5b1b768c834b803205486744a8fa",
         intel: "7d77b56c68389bac3d340f9df50bae52841594d80962e735e12a75db16671ecd"

  arch arm: "aarch64-apple-silicon", intel: "x64-intel"

  url "https://github.com/saeedkolivand/ai-job-hunter-app/releases/download/v#{version}/macos-AI-Job-Hunter_#{version}_#{arch}.dmg",
      verified: "github.com/saeedkolivand/ai-job-hunter-app/"
  name "AI Job Hunter"
  desc "Local-first, AI-native desktop assistant for job searching and applications"
  homepage "https://github.com/saeedkolivand/ai-job-hunter-app"

  app "AI Job Hunter.app"

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
