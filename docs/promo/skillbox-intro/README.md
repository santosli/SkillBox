# SkillBox Intro Promo

Source for the approved 30-second horizontal SkillBox promotional video.

The composition is a HyperFrames HTML artifact. It introduces the SkillBox problem/solution arc: scattered agent skills, product reveal, managed local store, review/update safety, usage/history, and GitHub CTA.

## Preview

```sh
npx hyperframes preview
```

Open the Studio URL reported by the command for the `skillbox-intro` project.

## Render

```sh
npx hyperframes render --output /tmp/skillbox-promo-v3.mp4 --quality high
```

The approved rendered MP4 is stored outside git at `/tmp/skillbox-promo-v3.mp4`. Rendered videos and temporary inspection captures are intentionally not committed by default.

## Verification

Current verification commands:

```sh
npx hyperframes lint
npx hyperframes validate
npx hyperframes inspect --json
ffprobe -v error -select_streams v:0 -show_entries stream=codec_name,width,height,r_frame_rate,duration -of default=noprint_wrappers=1 /tmp/skillbox-promo-v3.mp4
ffprobe -v error -select_streams a:0 -show_entries stream=codec_name,channels,sample_rate,duration -of default=noprint_wrappers=1 /tmp/skillbox-promo-v3.mp4
ffmpeg -hide_banner -nostats -i /tmp/skillbox-promo-v3.mp4 -af volumedetect -f null -
ffmpeg -hide_banner -nostats -i /tmp/skillbox-promo-v3.mp4 -af silencedetect=noise=-45dB:d=0.5 -f null -
```

Known accepted lint warnings:

- `gsap_studio_edit_blocked`: the composition intentionally uses a registered GSAP timeline for deterministic rendered animation. Removing timeline ownership would break the HyperFrames animation contract and the approved motion.
- `composition_file_too_large`: this is a single 30-second promo composition. Splitting into sub-compositions would be a larger refactor after visual approval and is not needed for this packaged candidate.

Animation map status:

- The HyperFrames animation-map script was attempted with temporary `@hyperframes/producer` installs, including Node 22, without adding project dependencies.
- The remaining blocker is `ERR_AMBIGUOUS_MODULE_SYNTAX` inside `@hyperframes/producer`'s bundled `wawoff2` dependency.
