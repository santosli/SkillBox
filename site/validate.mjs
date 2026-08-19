import { readFile, stat } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const googleVerificationFile = "googleffb526fcf02488a3.html";
const googleVerificationToken = `google-site-verification: ${googleVerificationFile}`;
const requiredFiles = [
  "site/index.html",
  "site/privacy.html",
  "site/telemetry.js",
  "site/styles.css",
  "site/robots.txt",
  "site/sitemap.xml",
  `site/${googleVerificationFile}`,
  "site-dist/privacy.html",
  "site-dist/telemetry.js",
  "site-dist/assets/skillbox-collection-import-review.png",
  `site-dist/${googleVerificationFile}`,
  "site/build.mjs",
  ".github/workflows/pages.yml",
  "docs/promo/skillbox-intro/skillbox-promo.mp4",
  "docs/promo/skillbox-intro/skillbox-promo-poster.jpg",
  "docs/promo/skillbox-intro/assets/skillbox-dashboard.png",
  "docs/promo/skillbox-intro/assets/skillbox-workspaces.png",
  "docs/promo/skillbox-intro/assets/skillbox-rankings.png",
  "docs/promo/skillbox-intro/assets/skillbox-rankings-coverage.png",
  "docs/promo/skillbox-intro/assets/skillbox-history.png",
  "docs/promo/skillbox-intro/assets/skillbox-skill-detail.png",
  "docs/promo/skillbox-intro/assets/skillbox-github-install-review.png",
  "docs/promo/skillbox-intro/assets/skillbox-app-icon.png",
  "docs/screenshots/skillbox-collection-import-review.png"
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
const privacy = await readFile(path.join(root, "site", "privacy.html"), "utf8");
const telemetry = await readFile(path.join(root, "site", "telemetry.js"), "utf8");
const robots = await readFile(path.join(root, "site", "robots.txt"), "utf8");
const sitemap = await readFile(path.join(root, "site", "sitemap.xml"), "utf8");
const workflow = await readFile(path.join(root, ".github", "workflows", "pages.yml"), "utf8");
const readme = await readFile(path.join(root, "README.md"), "utf8");
const readmeZh = await readFile(path.join(root, "README.zh-CN.md"), "utf8");
const googleVerificationSource = await readFile(path.join(root, "site", googleVerificationFile), "utf8").catch(() => "");
const googleVerificationBuilt = await readFile(path.join(root, "site-dist", googleVerificationFile), "utf8").catch(() => "");
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
  "description mentions SkillBox, local-first agents, runtime profiles, SKILL.md, and reviewed GitHub installs",
  /<meta name="description" content="[^"]*SkillBox[^"]*local-first[^"]*Codex[^"]*Claude Code[^"]*Cursor[^"]*SKILL\.md[^"]*GitHub[^"]*">/.test(html)
);
expect("canonical URL present", /<link rel="canonical" href="https:\/\/santosli\.github\.io\/SkillBox\/">/.test(html));
expect(
  "robots.txt points to the canonical sitemap",
  /Sitemap:\s*https:\/\/santosli\.github\.io\/SkillBox\/sitemap\.xml/.test(robots)
);
expect(
  "sitemap.xml contains the canonical homepage",
  /<loc>https:\/\/santosli\.github\.io\/SkillBox\/<\/loc>/.test(sitemap)
);
expect("Open Graph image present", /<meta property="og:image" content="https:\/\/santosli\.github\.io\/SkillBox\/assets\/skillbox-promo-poster\.jpg">/.test(html));
expect("Open Graph image dimensions present", /<meta property="og:image:width" content="1920">/.test(html) && /<meta property="og:image:height" content="1080">/.test(html));
expect("Twitter card present", /<meta name="twitter:card" content="summary_large_image">/.test(html));
expect("Twitter image alt present", /<meta name="twitter:image:alt" content="SkillBox macOS app dashboard and promo poster">/.test(html));
expect("homepage loads the first-party telemetry consent controller", /<script defer src="telemetry\.js"><\/script>/.test(html));
expect("homepage does not load VibeLoft before consent", !/<script[^>]+src="https:\/\/vibeloft\.ai\/telemetry\/v1\.js"/.test(html));
expect(
  "consent controller uses the assigned VibeLoft identity",
  /const SCRIPT_URL = "https:\/\/vibeloft\.ai\/telemetry\/v1\.js"/.test(telemetry)
    && /const PRODUCT_ID = "2b756c64-1f61-4945-a187-d86bf25e56aa"/.test(telemetry)
    && /const AUTH_KEY = "vl_web\.[A-Za-z0-9_-]{43}"/.test(telemetry)
);
expect(
  "consent controller gates loading and supports withdrawal",
  /skillbox\.analytics\.consent\.v1/.test(telemetry)
    && /navigator\.globalPrivacyControl/.test(telemetry)
    && /navigator\.doNotTrack/.test(telemetry)
    && /client\.stop\(\{ flush: false \}\)/.test(telemetry)
    && /localStorage\.removeItem\(DEVICE_KEY\)/.test(telemetry)
    && /script\.addEventListener\("load"/.test(telemetry)
    && /readConsent\(\) === ALLOWED/.test(telemetry)
);
expect("homepage exposes allow and decline choices", /data-analytics-allow/.test(html) && /data-analytics-decline/.test(html));
expect("consent notice keeps a stable privacy link", /<span data-analytics-status>[\s\S]*?<\/span>\s*Read the <a href="privacy\.html">website privacy notice<\/a>/.test(html));
expect("consent notice is a non-modal live region", /data-analytics-consent role="region" aria-live="polite"/.test(html));
expect("homepage links to website privacy disclosure", /<a href="privacy\.html">Website privacy<\/a>/.test(html));
expect(
  "privacy disclosure separates website telemetry from local SkillBox data",
  /No VibeLoft request is made before you allow analytics/.test(privacy)
    && /separate from the SkillBox macOS app and CLI/.test(privacy)
    && /cannot access managed skills/.test(privacy)
    && /vibeloft\.telemetry\.device\.v1/.test(privacy)
);
expect(
  "privacy disclosure documents VibeLoft payload and controls",
  /query parameters and fragments are removed/.test(privacy)
    && /SHA-256 feature digest/.test(privacy)
    && /Global Privacy Control/.test(privacy)
    && /Do Not Track/.test(privacy)
    && /https:\/\/api\.vibeloft\.ai\/api\/v1\/telemetry\/events/.test(privacy)
    && /trusted-telemetry-for-vibecoding-products/.test(privacy)
    && !/https:\/\/vibeloft\.ai\/privacy/.test(privacy)
);
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
expect(
  "homepage uses current canonical product visuals",
  /assets\/skillbox-dashboard\.png/.test(html)
    && /assets\/skillbox-workspaces\.png/.test(html)
    && /assets\/skillbox-rankings-coverage\.png/.test(html)
    && /assets\/skillbox-history\.png/.test(html)
    && /assets\/skillbox-collection-import-review\.png/.test(html)
);
expect(
  "homepage explains GitHub collection review boundary",
  /one bounded repository ref/.test(html)
    && /resolved SHA/.test(html)
    && /one explicit User\/Remote choice/.test(html)
    && /selects or clears the eligible set/.test(html)
    && /Mixed or\s+unresolved type state stays blocked/.test(html)
    && /nothing deploys\s+automatically/.test(html)
    && /collection-level\s+update and rollback remain Phase D work/.test(html)
);
expect(
  "public README and homepage retire the stale collection review visual",
  !/skillbox-github-install-review\.png/.test(html)
    && !/skillbox-github-install-review\.png/.test(readme)
    && !/skillbox-github-install-review\.png/.test(readmeZh)
);
expect(
  "homepage explains evidence-aware local metrics",
  /locally confirmed and defensible inferred invocations/.test(html)
    && /History references stay separate/.test(html)
    && /without claiming account analytics/.test(html)
);
expect("homepage has no stale versioned visual references", !/v0?41|v041|skillbox-import-review-crop/.test(html));
expect("video embed uses controls and metadata preload", /<video controls preload="metadata" poster="assets\/skillbox-promo-poster\.jpg"/.test(html));
expect("promo is labeled as a v0.9.0 release artifact", /v0\.9\.0 · 30-second overview/.test(html) && /This v0\.9\.0 promo/.test(html));
expect("download CTA uses latest release", /https:\/\/github\.com\/santosli\/SkillBox\/releases\/latest/.test(html));
expect("Homebrew command present", /brew install --cask skillbox/.test(html));
expect("Google verification source contains expected token", googleVerificationSource.trim() === googleVerificationToken);
expect("Google verification build output contains expected token", googleVerificationBuilt.trim() === googleVerificationToken);
expect("README badges do not advertise legacy Node CLI", !/Node\.js-legacy%20CLI|legacy CLI/.test(readme) && !/Node\.js-legacy%20CLI|legacy CLI/.test(readmeZh));
expect("README badges describe frontend tooling", /Frontend-React%20%2B%20Vite/.test(readme) && /Frontend-React%20%2B%20Vite/.test(readmeZh));

expect("workflow uses configure-pages", /uses: actions\/configure-pages@v6/.test(workflow));
expect("workflow uses upload-pages-artifact", /uses: actions\/upload-pages-artifact@v5/.test(workflow));
expect("workflow uses deploy-pages", /uses: actions\/deploy-pages@v5/.test(workflow));
expect("workflow copies promo video through build script", /node site\/build\.mjs/.test(workflow));
const buildScript = await readFile(path.join(root, "site", "build.mjs"), "utf8");
expect("build copies the website privacy page", /"privacy\.html"/.test(buildScript));
expect("build copies the telemetry consent controller", /"telemetry\.js"/.test(buildScript));
expect(
  "build copies the canonical collection review screenshot from docs",
  /docs[^\n]*screenshots/.test(buildScript)
    && /skillbox-collection-import-review\.png/.test(buildScript)
    && !/assets\/skillbox-github-install-review\.png/.test(buildScript)
);
expect(
  "homepage keeps installed-source provenance display-only",
  /Validated copied skills/.test(html)
    && /normalized GitHub source/.test(html)
    && /display-only provenance/.test(html)
    && /without inventing branch, HEAD, or update authority/.test(html)
);
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
