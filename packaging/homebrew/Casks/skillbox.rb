cask "skillbox" do
  version "0.1.1"
  sha256 "d3e74dd1a3cf04c97227d6ea5fbdc71ec4ae889b61caef3f59c31219c23fe530"

  url "https://github.com/santosli/skill-box/releases/download/v#{version}/SkillBox_#{version}_universal.dmg"
  name "SkillBox"
  desc "Local skill manager for agent runtimes"
  homepage "https://github.com/santosli/skill-box"

  depends_on macos: ">= :sonoma"

  app "SkillBox.app"

  zap trash: [
    "~/Library/Application Support/io.github.santosli.skillbox",
    "~/Library/Caches/io.github.santosli.skillbox",
    "~/Library/Logs/SkillBox",
    "~/Library/Preferences/io.github.santosli.skillbox.plist",
    "~/Library/Saved Application State/io.github.santosli.skillbox.savedState",
  ]
end
