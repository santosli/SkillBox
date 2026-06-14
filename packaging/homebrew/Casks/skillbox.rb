cask "skillbox" do
  version "0.3.1"
  sha256 "eb6fda5618f823db4713acc42547ac7f15c23fd9b4108f1891b4815d79befb6d"

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
