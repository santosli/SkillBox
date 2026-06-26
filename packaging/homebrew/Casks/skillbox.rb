cask "skillbox" do
  version "0.3.5"
  sha256 "e295e3dbca1310b16fdc4d669e675adfd2df509d636db956768e30adb1a673e0"

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
