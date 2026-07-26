import { cp, mkdir, rm } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const dist = path.join(root, "site-dist");
const site = path.join(root, "site");
const promo = path.join(root, "docs", "promo", "skillbox-intro");
const assets = path.join(dist, "assets");

await rm(dist, { recursive: true, force: true });
await mkdir(assets, { recursive: true });

for (const file of ["index.html", "privacy.html", "telemetry.js", "styles.css", "robots.txt", "sitemap.xml", "404.html", "googleffb526fcf02488a3.html"]) {
  await cp(path.join(site, file), path.join(dist, file));
}

for (const [from, to] of [
  ["skillbox-promo.mp4", "skillbox-promo.mp4"],
  ["skillbox-promo-poster.jpg", "skillbox-promo-poster.jpg"],
  ["assets/skillbox-dashboard.png", "skillbox-dashboard.png"],
  ["assets/skillbox-workspaces.png", "skillbox-workspaces.png"],
  ["assets/skillbox-rankings.png", "skillbox-rankings.png"],
  ["assets/skillbox-rankings-coverage.png", "skillbox-rankings-coverage.png"],
  ["assets/skillbox-history.png", "skillbox-history.png"],
  ["assets/skillbox-skill-detail.png", "skillbox-skill-detail.png"],
  ["assets/skillbox-github-install-review.png", "skillbox-github-install-review.png"],
  ["assets/skillbox-app-icon.png", "skillbox-app-icon.png"]
]) {
  await cp(path.join(promo, from), path.join(assets, to));
}

await cp(path.join(promo, "assets", "fonts"), path.join(assets, "fonts"), { recursive: true });

console.log(`Built ${path.relative(root, dist)}`);
