Client OpenXR implementation guidance (Meta LUT overlay)

Primary reference
-meta_lut_overlay_execplan.md

Expected files to touch
- alvr/session/src/settings.rs
- alvr/graphics/src/stream.rs
- alvr/client_openxr/src/stream.rs
- alvr/client_openxr/src/lib.rs
- alvr/client_openxr/src/passthrough.rs
- alvr/client_openxr/src/extra_extensions/passthrough_fb.rs
- alvr/client_openxr/src/extra_extensions/multimodal_input.rs (or another constants file)

OpenXR requirements
- Enable "XR_META_passthrough_color_lut" at xrCreateInstance time via the exts.other allowlist.
- Runtime toggling: only create/apply LUT when PassthroughMode::MetaLutOverlay is active.
- Fail closed: if function pointers are unavailable, log and continue without LUT.

Composition rules
- Existing ALVR modes: keep [passthrough, projection].
- Meta LUT Overlay mode:
  - Use [projection, passthrough].
  - Projection must remain fully opaque.
  - Do not run video-side chroma key shader.

Failure handling
- Fix within current checkpoint; do not advance until builds are green.
