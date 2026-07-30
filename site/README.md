# SkillBox Homepage

Static GitHub Pages source for the public SkillBox homepage:

https://santosli.github.io/SkillBox/

The source is intentionally plain HTML/CSS plus small Node scripts. It does not add a package workspace or runtime dependencies.

## Local Preview

Build the Pages artifact:

```sh
node site/build.mjs
```

Serve the generated directory:

```sh
python3 -m http.server 4173 --directory site-dist
```

Then open:

```text
http://127.0.0.1:4173/
```

## Validation

```sh
node site/validate.mjs
git diff --check
```

`site/build.mjs` copies the committed promo assets from `docs/promo/skillbox-intro/` into `site-dist/assets/`. The generated `site-dist/` directory is ignored and should not be committed.

## Website Telemetry

After a visitor opts in, the homepage loads VibeLoft's browser telemetry to count page views and verify the public product listing. The first-party consent controller lives in `site/telemetry.js`; the disclosure in `site/privacy.html` is part of the deployable site and must stay aligned with the telemetry client behavior.

The current client records page paths without query strings or fragments, persists a random first-party device ID, submits a browser-generated device feature digest, and respects Global Privacy Control and Do Not Track. Declining prevents the third-party script from loading; withdrawing consent stops the active client and removes its device ID. It does not connect to the SkillBox app, CLI, managed store, or local usage database.

When changing the integration, inspect the published `https://vibeloft.ai/telemetry/v1.js` client and update both `site/privacy.html` and `site/validate.mjs` before deployment.

## Search Indexing

After Pages deploys, submit the sitemap URL to search consoles if you want faster discovery:

```text
https://santosli.github.io/SkillBox/sitemap.xml
```

Maintainer follow-up after a homepage or public metadata change:

- Submit the sitemap to Google Search Console.
- Submit the same sitemap to Bing Webmaster Tools.
- Check the GitHub repository About website URL, repository topics, and social
  preview image.
- After deployment, verify the public sitemap URL and use a query such as
  `site:santosli.github.io/SkillBox` later to check discovery.

These are manual external actions, not CI steps. Do not commit search-console
credentials, verification secrets, analytics scripts, or tracking pixels.
