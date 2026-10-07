# Media privacy

- Do not open or play the user's downloaded videos, or inspect their thumbnails or video frames.
- Screenshots are allowed only when downloaded-media thumbnails, titles/filenames, and video frames are hidden before capture. Use synthetic inspection pages where possible.
- For real app screenshots, enable **Allow screenshots** and **Inspection privacy**, save, and wait for the protected UI to render. Never disable Inspection privacy for an agent screenshot.
- Do not use DOM dumps, accessibility dumps, logs, or media metadata to retrieve the hidden titles/filenames. Inspect code and synthetic fixtures instead.
- Do not capture fullscreen or picture-in-picture media. If a screen cannot be confidently redacted, do not capture or view it.

# Changelog and pushes

- Add concrete release notes to `CHANGELOG` in `src/local/pages/changelog.rs` for every change before pushing. Include all changes in the pushed range.
- Run `make hooks` in a new checkout to enable the tracked pre-push guard.
- Never bypass the changelog guard with `--no-verify`, an alternate hooks path, or by removing the hook. Fix the missing notes instead.
