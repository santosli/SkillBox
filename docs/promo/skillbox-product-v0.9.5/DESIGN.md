# SkillBox Product Promo Design · v0.9.5

## Style Prompt

A 75-second horizontal product overview for SkillBox v0.9.5. Calm, precise, local-first. The visual
language is the product's own light interface, not a separate marketing skin: feature shots mount the
real React components from `apps/desktop/src` and load the product's own `colors.css` and `styles.css`.
Dark title cards carry the narrative beats; the product UI carries the proof. No screenshots, no mockups,
no invented metrics.

## Canvas

- Size: 1920×1080, 16:9
- Duration: 75.000s (40 bars at 128 BPM, 1.875s per bar)
- Frame rate: 30 fps
- Language: Simplified Chinese copy, English headline spans

## Narrative

Spine: **散在各处的技能，收进一个库**. The first two feature shots are the centre — unified management
itself. The last three are what makes it trustworthy — where skills land, how they get in, how usage is
counted.

- 0–7.5s — Title: one library for every skill
- 7.5–18.75s — Dashboard: everything in one screen
- 18.75–30s — Skill detail: one edit, every runtime
- 30–41.25s — Workspaces: no single source of truth
- 41.25–52.5s — Import Review: review before write
- 52.5–65.625s — Usage: counted, not guessed
- 65.625–75s — Close: local-first, review-first

## Colors

All tokens come from `apps/desktop/src/colors.css`. The film derives only two variables and invents no
brand colour of its own.

- Ink (title cards): `#0f172a` (= `--skillbox-text-strong`)
- On-ink text: `#f8fafc` (= `--skillbox-surface-muted`)
- Accent: `#2563eb` (= `--skillbox-blue`); on ink, `#bfdbfe` (= `--skillbox-blue-border`)
- Product surfaces keep their own values, untouched.

## Typography

- English headline spans: `Space Grotesk` 700 — the same brand face as the v0.9.0 promo.
- Chinese headline and body: `PingFang SC` → `Noto Sans CJK SC` → system sans.
- Product UI keeps its own font stack. Nothing inside a component is restyled.

## Motion

- Hard-edged left-to-right wipe between shots, 0.45s, always on a bar line. No cross-dissolve: fading a
  dark title card over a light product screen produces a grey mid-state.
- Copy enters with a short rise + fade; panels settle on the downbeat.
- Two shots use a gentle 1.07× camera push toward the region being discussed.
- The timeline is a pure function of time, so seeking backwards reproduces the frame exactly.

## Sound

- Score composed in code for this film: 128 BPM, 40 bars, A major, fixed random seed. Python stdlib +
  FFmpeg only — no samples, no third-party recordings, no music generation model.
- 12 action cues, every audible landmark within 0 frames of its action and on the quarter-note grid.
- Music ducks per cue; every cue measures 3–6 dB above the ducked music.

## What Not To Do

- No screenshots standing in for live components.
- No invented metrics, and no fixture presented as somebody's real usage.
- No claim about collection-level update/rollback — still Phase D.
- No restyling of product components; framing adjustments only, and documented.
