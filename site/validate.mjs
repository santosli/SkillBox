import { readFile, stat } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const requiredFiles = [
  "site/index.html",
  "site/styles.css",
  "site/robots.txt",
  "site/sitemap.xml",
  "site/build.mjs",
  ".github/workflows/pages.yml",
  "docs/promo/skillbox-intro/skillbox-promo.mp4",
  "docs/promo/skillbox-intro/skillbox-promo-poster.jpg",
  "docs/promo/skillbox-intro/assets/skillbox-dashboard.png",
  "docs/promo/skillbox-intro/assets/skillbox-import-review-crop.png",
  "docs/promo/skillbox-intro/assets/skillbox-history.png",
  "docs/promo/skillbox-intro/assets/skillbox-app-icon.png"
];

const checks = [];

function expect(name, condition) {
  checks.push({ name, ok: Boolean(condition) });
}

for (const file of requiredFiles) {
  const stats = await stat(path.join(root, file)).catch(() => null);
  expect(`${file} exists`, stats?.isFile());
}

const html = await readFile(path.join(root, "site", "index.html"), "utf8");
const workflow = await readFile(path.join(root, ".github", "workflows", "pages.yml"), "utf8");
const readme = await readFile(path.join(root, "README.md"), "utf8");
const readmeZh = await readFile(path.join(root, "README.zh-CN.md"), "utf8");

expect("title tag present", /<title>SkillBox - Local skill manager for AI agents<\/title>/.test(html));
expect("description meta present", /<meta name="description" content="[^"]*SkillBox[^"]*">/.test(html));
expect("canonical URL present", /<link rel="canonical" href="https:\/\/santosli\.github\.io\/SkillBox\/">/.test(html));
expect("Open Graph image present", /<meta property="og:image" content="https:\/\/santosli\.github\.io\/SkillBox\/assets\/skillbox-promo-poster\.jpg">/.test(html));
expect("Twitter card present", /<meta name="twitter:card" content="summary_large_image">/.test(html));
expect("JSON-LD SoftwareApplication present", /"@type": "SoftwareApplication"/.test(html));
expect("JSON-LD does not hardcode softwareVersion", !/"softwareVersion"\s*:/.test(html));
expect("video embed uses controls and metadata preload", /<video controls preload="metadata" poster="assets\/skillbox-promo-poster\.jpg"/.test(html));
expect("download CTA uses latest release", /https:\/\/github\.com\/santosli\/SkillBox\/releases\/latest/.test(html));
expect("Homebrew command present", /brew install --cask skillbox/.test(html));

expect("workflow uses configure-pages", /uses: actions\/configure-pages@v5/.test(workflow));
expect("workflow uses upload-pages-artifact", /uses: actions\/upload-pages-artifact@v4/.test(workflow));
expect("workflow uses deploy-pages", /uses: actions\/deploy-pages@v4/.test(workflow));
expect("workflow copies promo video through build script", /node site\/build\.mjs/.test(workflow));
expect("workflow watches site paths", /site\/\*\*/.test(workflow));
expect("workflow watches promo paths", /docs\/promo\/skillbox-intro\/\*\*/.test(workflow));
expect("README has website link", /https:\/\/santosli\.github\.io\/SkillBox\//.test(readme));
expect("Chinese README has website link", /https:\/\/santosli\.github\.io\/SkillBox\//.test(readmeZh));

const failed = checks.filter((check) => !check.ok);

for (const check of checks) {
  console.log(`${check.ok ? "ok" : "not ok"} - ${check.name}`);
}

if (failed.length > 0) {
  process.exitCode = 1;
}
