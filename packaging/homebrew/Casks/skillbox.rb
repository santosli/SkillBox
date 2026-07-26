cask "skillbox" do
  version "0.6.1"
  sha256 "f0c2673044bfef9b4050fc524cac2757cb2f52b31a274ddf905f9188bddfc765"

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
