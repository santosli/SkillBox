cask "skillbox" do
  version "0.1.0-alpha.1"
  sha256 "REPLACE_WITH_SHA256_FROM_SHA256SUMS"

  url "https://github.com/skillbox-dev/skill-box/releases/download/v#{version}/SkillBox_#{version}_universal.dmg",
      verified: "github.com/skillbox-dev/skill-box/"
  name "SkillBox"
  desc "Local skill manager for agent runtimes"
  homepage "https://github.com/skillbox-dev/skill-box"

  depends_on macos: ">= :sonoma"

  app "SkillBox.app"

  zap trash: [
    "~/Library/Application Support/io.github.skillbox-dev.skillbox",
    "~/Library/Caches/io.github.skillbox-dev.skillbox",
    "~/Library/Logs/SkillBox",
    "~/Library/Preferences/io.github.skillbox-dev.skillbox.plist",
    "~/Library/Saved Application State/io.github.skillbox-dev.skillbox.savedState",
  ]
end
