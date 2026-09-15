cask "skillbox" do
  version "0.9.4"
  sha256 "3706c6ba3275c4017c48d60005065d071ba041f9b5330dea59e0a85052fb249a"

  url "https://github.com/santosli/SkillBox/releases/download/v#{version}/SkillBox_#{version}_universal.dmg"
  name "SkillBox"
  desc "Local skill manager for agent runtimes"
  homepage "https://github.com/santosli/SkillBox"

  depends_on macos: :sonoma

  app "SkillBox.app"

  zap trash: [
    "~/Library/Application Support/io.github.santosli.skillbox",
    "~/Library/Caches/io.github.santosli.skillbox",
    "~/Library/Logs/SkillBox",
    "~/Library/Preferences/io.github.santosli.skillbox.plist",
    "~/Library/Saved Application State/io.github.santosli.skillbox.savedState",
  ]
end
