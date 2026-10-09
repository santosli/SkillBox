import { cp, mkdir, rm } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const dist = path.join(root, "site-dist");
const site = path.join(root, "site");
const promo = path.join(root, "docs", "promo", "skillbox-intro");
// 首页播放当前宣传片；产品截图与字体仍沿用 v0.9.0 promo 目录里的素材。
const productPromo = path.join(root, "docs", "promo", "skillbox-product-v0.9.5");
const screenshots = path.join(root, "docs", "screenshots");
const assets = path.join(dist, "assets");

await rm(dist, { recursive: true, force: true });
await mkdir(assets, { recursive: true });

for (const file of ["index.html", "privacy.html", "telemetry.js", "styles.css", "robots.txt", "sitemap.xml", "404.html", "googleffb526fcf02488a3.html"]) {
  await cp(path.join(site, file), path.join(dist, file));
}

for (const [from, to] of [
  ["skillbox-product-promo.mp4", "skillbox-promo.mp4"],
  ["skillbox-product-promo-poster.jpg", "skillbox-promo-poster.jpg"]
]) {
  await cp(path.join(productPromo, from), path.join(assets, to));
}

for (const [from, to] of [
  ["assets/skillbox-dashboard.png", "skillbox-dashboard.png"],
  ["assets/skillbox-workspaces.png", "skillbox-workspaces.png"],
  ["assets/skillbox-rankings.png", "skillbox-rankings.png"],
  ["assets/skillbox-rankings-coverage.png", "skillbox-rankings-coverage.png"],
  ["assets/skillbox-history.png", "skillbox-history.png"],
  ["assets/skillbox-skill-detail.png", "skillbox-skill-detail.png"],
  ["assets/skillbox-app-icon.png", "skillbox-app-icon.png"]
]) {
  await cp(path.join(promo, from), path.join(assets, to));
}

await cp(
  path.join(screenshots, "skillbox-collection-import-review.png"),
  path.join(assets, "skillbox-collection-import-review.png")
);

// 正文用系统字体栈（与桌面端一致），只有英文标题需要 Space Grotesk。
// v0.9.0 promo 目录里的 IBM Plex 子集留给那支片子自己的 index.html，不再进站点产物。
await mkdir(path.join(assets, "fonts"), { recursive: true });
for (const font of ["space-grotesk-400.woff2", "space-grotesk-700.woff2"]) {
  await cp(path.join(promo, "assets", "fonts", font), path.join(assets, "fonts", font));
}

console.log(`Built ${path.relative(root, dist)}`);
