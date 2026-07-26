# SkillBox Intro Promo

Source for the 30-second horizontal SkillBox promotional video refreshed for the v0.6.1 product surface.

The composition is a HyperFrames HTML artifact. It introduces one local home for agent skills, runtime-profile-aware Workspaces, reviewed GitHub installs and deployment compatibility, evidence-aware Calls with separate History references, and the GitHub CTA.

All product screenshots come from the deterministic browser preview fixture. They contain generic paths, skill names, and counts rather than local user data.

## Preview

```sh
npx hyperframes preview
```

Open the Studio URL reported by the command for the `skillbox-intro` project.

## Render

```sh
npx hyperframes render --output /tmp/skillbox-promo-v061-render.mp4 --quality high
ffmpeg -i /tmp/skillbox-promo-v061-render.mp4 \
  -c:v copy -af loudnorm=I=-16:TP=-1:LRA=11 \
  -c:a aac -b:a 192k -ar 48000 -t 30 \
  /tmp/skillbox-promo-v061.mp4
```

The first command renders the deterministic composition. The second normalizes the public audio master and writes `/tmp/skillbox-promo-v061.mp4`.

The public README uses these committed media files from this folder:

- `skillbox-promo.mp4` - stable repository-relative video link.
- `skillbox-promo-poster.jpg` - clickable poster image.

Other rendered videos and temporary inspection captures are intentionally not committed by default.

## Verification

Current verification commands:

```sh
npx hyperframes lint
npx hyperframes validate
npx hyperframes inspect --json
ffprobe -v error -select_streams v:0 -show_entries stream=codec_name,width,height,r_frame_rate,duration -of default=noprint_wrappers=1 /tmp/skillbox-promo-v061.mp4
ffprobe -v error -select_streams a:0 -show_entries stream=codec_name,channels,sample_rate,duration -of default=noprint_wrappers=1 /tmp/skillbox-promo-v061.mp4
ffmpeg -hide_banner -nostats -i /tmp/skillbox-promo-v061.mp4 -af loudnorm=I=-16:TP=-1:LRA=11:print_format=summary -f null -
ffmpeg -hide_banner -nostats -i /tmp/skillbox-promo-v061.mp4 -af silencedetect=noise=-45dB:d=0.5 -f null -
```

Known accepted lint warning:

- `composition_file_too_large`: this is a single 30-second promo composition with one registered deterministic GSAP timeline. Splitting it into sub-compositions would add cross-file timing ownership without improving the public artifact, so the source remains intentionally self-contained.

Animation map status:

- The HyperFrames animation-map script was attempted with temporary `@hyperframes/producer` installs, including Node 22, without adding project dependencies.
- The remaining blocker is `ERR_AMBIGUOUS_MODULE_SYNTAX` inside `@hyperframes/producer`'s bundled `wawoff2` dependency.
