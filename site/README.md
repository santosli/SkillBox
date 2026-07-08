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
