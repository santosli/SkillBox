cask "skillbox" do
  version "0.3.7"
  sha256 "aa9a7d2a90f65c1fb9053dbb66fff51349ed1f65ea5c2fa50dfe9ef253205b8e"

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
