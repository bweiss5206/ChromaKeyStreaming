# Meta LUT Overlay Debug Log

## Scope
- Project: ALVR Android OpenXR client (Quest 3)
- Branch: `meta-lut-overlay`
- Working commit from this session: `c8f9fda9`
- Goal: Make `MetaLutOverlay` passthrough mode key greenscreen correctly over streamed PCVR content.

## Main issues found
1. **Protocol mismatch prevented connection earlier**
- Server/client protocol mismatch (`server=20`, `client=21-dev12`) caused connect failures until matching builds were used.

2. **Meta LUT extension not actually enabled at instance creation**
- Runtime reported:
  - `meta_passthrough_color_lut: true` in available extensions
  - but function proc load failed with:
    - `xrCreatePassthroughColorLutMETA not supported, extension is not enabled`
- Fix: enable `exts.meta_passthrough_color_lut = available_extensions.meta_passthrough_color_lut`.

3. **META LUT style path had raw FFI/spec mismatches**
- Corrected raw structs and usage around:
  - `XrPassthroughColorLutCreateInfoMETA`
  - `XrPassthroughColorLutDataMETA`
  - `XrPassthroughColorMapLutMETA`
- Corrected color-map structure type:
  - `XR_TYPE_PASSTHROUGH_COLOR_MAP_LUT_META = 1000266100`

4. **LUT update cache did not rebuild on real-time parameter changes**
- LUT was effectively cached by resolution only.
- Fix: rebuild when full `MetaLutOverlayConfig` changes.

5. **Style application timing**
- Style application could miss initial stream transition.
- Fix: apply passthrough style at `StreamingStarted`, not only on later realtime config updates.

## Files touched (client feature path)
- `alvr/client_openxr/src/lib.rs`
- `alvr/client_openxr/src/passthrough.rs`
- `alvr/client_openxr/src/stream.rs`
- `alvr/client_openxr/src/extra_extensions/passthrough_fb.rs`

## Validation performed
- Build commands run successfully:
  - `cargo build -p alvr_session`
  - `cargo build -p alvr_graphics`
  - `cargo build -p alvr_client_openxr`
- Device deploy/run:
  - `cd alvr/client_openxr`
  - `cargo apk run --no-logcat`
- Logcat verification focused on:
  - extension availability/enablement
  - LUT proc load and style apply errors
  - passthrough compositor style status

## Practical tuning workflow (MetaLutOverlay)
Use this as a repeatable process:

1. Set baseline:
- `weight = 1.0`
- `lut_resolution = 32`
- `hue_center_deg = 120`
- `hue_width_deg = 40`
- `sat_min = 0.30`
- `val_min = 0.20`
- `feather = 0.20`

2. Tune in order:
- Lock `weight=1.0`, `feather=0.0` first.
- Move `hue_center_deg` to align with your actual greenscreen hue.
- Increase `hue_width_deg` until background is fully keyed.
- Raise `sat_min` to protect skin/neutral objects from accidental keying.
- Raise `val_min` only if dark green shadow zones are not keying.
- Add `feather` (`0.10`–`0.30`) to reduce edge harshness.
- Increase `lut_resolution` to `48`/`64` only if needed.

## Quick sanity commands
- Local branch + latest commit:
  - `git branch --show-current`
  - `git log -1 --oneline`
- Remote branch exists:
  - `git ls-remote --heads origin meta-lut-overlay`
- Local vs remote same commit:
  - `git fetch origin`
  - `git rev-parse HEAD`
  - `git rev-parse origin/meta-lut-overlay`

## Notes
- This file is a concise engineering record of the debug/fix path.
- If behavior regresses, start with logcat checks for extension enablement and style application errors before changing LUT math.
