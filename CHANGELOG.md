# mania-converter changelog

## v0.6.1

### Fixed
- **malody:** preserve Malody scroll effects across BPM changes when converting to osu (4e6ee17)
- **osu:** Add malody scroll=1.0 effect for osu uninhereted timing point with no following sv (#17) (17bfb9a)
- **malody:** export osu files in mcz (#18) (5372526)


### Other changes
- **lib:** 0.6.0 -> 0.6.1 (#19) (60487d1)



## v0.6.0

### Added
- **osu_func:** Implemented hitsound parsing for osu notes (dbf8297)
- **malody_func:** mc and osu convert utilizing old segments, mc serialize (467ed10)
- **malody, utils:** add zip_utils module and utilize it in mcz2osz (c61322c)
- **osu:** implement osz2mcz, and optimize osz_func (f8abb85)


### Fixed
- ci duplicate triggers, hook exec bit and skip hooks in CI (4b013bc)
- finish all hook checks before rejecting commits (#6) (9460894)
- **osu:** avoid BPM-less panic and revive the short timing point fallback (#11) (f2ba3b0)


### Changed
- dedupe conversions, upgrade deps and use variable font (#4) (e4d4774)


### Documentation
- add contirbuting guide (91a62b0)


### Tests
- **osu:** add osz to mcz smoke test (b28bb91)


### Maintenance
- add pre-commit hooks & cargo clippy (ec40748)
- add agent guide and stop pre-commit auto-staging (#5) (acffc2d)
- **release:** restrict write permission, tighten trigger and guard existing tag (#10) (ae6ed9f)
- **release:** stop the version step failing when no release exists yet (f0bef6b)


### Other changes
- Reorganize workspace (ee4b434)
- Reorganized structure (f15a459)
- Added functions to convert from .osu to .mc (a03b337)
- feat(osu_func, malody_func): Implemented time-based hitsound bi-directional conversion. (dbb5fea)
- feat(osu): Reorganized module osu and made annotations. (d12b02c)
- feat(osu): clippy (cc8517a)


## v0.6.0-b

# Updates
+ Changed Structure of  `OsuData` and `OsuHitObject` to support float timings of hit objects.
+ Fixed bugs including negative offset and preview time in `McData`
+ Info Generation now uses `lazy_static` to cache svg and font data, making it slightly faster.
# To-do List (for v0.6.0 stable)
+ Add osz to mcz function;
+ Use `vello` as an alternative gpu-accelerated svg renderer.

## v0.4.1

Updates the Structure `BeatMapInfo`:
+ Now the member variables inside are all public
+ Added new member variables `min_bpm`, `max_bpm` and `length`, and `to_beatmap_info()` in `OsuData` will automatically calculate these values, the `fmt:Display` for `BeatMapInfo` also includes string outputs for them.

## v0.4.0

A total REWORK.
+ Reorganized file structure, making it easier to read and modify.
+ Moved webapp as a standalone project, making the converter lighter.
+ Added osu!mania reborn SR calculation (based on sunnyxxy's 2025/04/15 version)
+ Now mcz searching in working directory /mc searching in temp directory run in parallel.
+ Added parameters to output information of converted maps.
+ Added a function to parse osz information for future use.

## v0.3.0

A major update for experience
+ Now the osu file generation part finally gets rid of old python style and fits in Rust style.
+ A level-up for parsing algorithms, now most of the maps converted will not confront unsnapped events in AiMod. There still remains problems at 124BPM, since the old osu editor is a Blackbox, no more effort will be put into it until completely understanding how it works. Since the maps converted using previous versions don't have these errors in modern Mapset Verifier, I think it's worthless continue working on this BS.
+ Uses `rayon` for parallel note translation.
+ Now only files related will be encapsuled, including mania mode .mc files and the audio and BG they use. And the files will be added directly to osz files, not maintaining the possible folders in mcz.

## v0.2.1

Changed the webapp.rs, bind it to IP 0.0.0.0 so that it can both be used in private and public.<br>
Since it is only an update for webapp, no standalone release is built.

## v0.2.0

Added webapp support and deployment.

## v0.1.1

Now fixed bugs about files under folders and non-ascii filename problems.
