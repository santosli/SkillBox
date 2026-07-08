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
const jsonLdMatch = html.match(/<script type="application\/ld\+json">\s*([\s\S]*?)\s*<\/script>/);
let jsonLd = null;
try {
  jsonLd = jsonLdMatch ? JSON.parse(jsonLdMatch[1]) : null;
} catch {
  jsonLd = null;
}
const graph = Array.isArray(jsonLd?.["@graph"]) ? jsonLd["@graph"] : [];
const softwareApplication = graph.find((entry) => entry?.["@type"] === "SoftwareApplication");
const website = graph.find((entry) => entry?.["@type"] === "WebSite");

expect("SEO title uses searchable macOS agent intent", /<title>SkillBox - Local AI Agent Skill Manager for macOS<\/title>/.test(html));
expect("robots meta allows indexing", /<meta name="robots" content="index, follow">/.test(html));
expect(
  "description mentions SkillBox, local-first agents, Codex, Claude, SKILL.md, and GitHub updates",
  /<meta name="description" content="[^"]*SkillBox[^"]*local-first[^"]*Codex[^"]*Claude[^"]*SKILL\.md[^"]*GitHub[^"]*">/.test(html)
);
expect("canonical URL present", /<link rel="canonical" href="https:\/\/santosli\.github\.io\/SkillBox\/">/.test(html));
expect("Open Graph image present", /<meta property="og:image" content="https:\/\/santosli\.github\.io\/SkillBox\/assets\/skillbox-promo-poster\.jpg">/.test(html));
expect("Open Graph image dimensions present", /<meta property="og:image:width" content="1920">/.test(html) && /<meta property="og:image:height" content="1080">/.test(html));
expect("Twitter card present", /<meta name="twitter:card" content="summary_large_image">/.test(html));
expect("Twitter image alt present", /<meta name="twitter:image:alt" content="SkillBox macOS app dashboard and promo poster">/.test(html));
expect("FAQ section includes at least four question items", /<section class="section faq-section" id="faq"/.test(html) && (html.match(/class="faq-item"/g) ?? []).length >= 4);
expect("FAQ mentions agent search terms", /Codex skills/.test(html) && /Claude skills/.test(html) && /SKILL\.md/.test(html) && /local-first/.test(html) && /GitHub-backed remote skills/.test(html));
expect("JSON-LD parses", Boolean(jsonLd));
expect("JSON-LD SoftwareApplication present", Boolean(softwareApplication));
expect("JSON-LD WebSite present", Boolean(website));
expect(
  "JSON-LD includes stable software SEO fields",
  softwareApplication?.applicationSubCategory === "AI agent skill manager"
    && softwareApplication?.softwareRequirements === "macOS"
    && softwareApplication?.offers?.["@type"] === "Offer"
    && softwareApplication?.offers?.price === 0
);
expect("JSON-LD does not hardcode softwareVersion", !/"softwareVersion"\s*:/.test(html));
expect("video embed uses controls and metadata preload", /<video controls preload="metadata" poster="assets\/skillbox-promo-poster\.jpg"/.test(html));
expect("download CTA uses latest release", /https:\/\/github\.com\/santosli\/SkillBox\/releases\/latest/.test(html));
expect("Homebrew command present", /brew install --cask skillbox/.test(html));
expect("README badges do not advertise legacy Node CLI", !/Node\.js-legacy%20CLI|legacy CLI/.test(readme) && !/Node\.js-legacy%20CLI|legacy CLI/.test(readmeZh));
expect("README badges describe frontend tooling", /Frontend-React%20%2B%20Vite/.test(readme) && /Frontend-React%20%2B%20Vite/.test(readmeZh));

expect("workflow uses configure-pages", /uses: actions\/configure-pages@v6/.test(workflow));
expect("workflow uses upload-pages-artifact", /uses: actions\/upload-pages-artifact@v5/.test(workflow));
expect("workflow uses deploy-pages", /uses: actions\/deploy-pages@v5/.test(workflow));
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
