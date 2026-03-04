ALVR fork task: Meta passthrough LUT overlay (greenscreen)

Goal
- Add a new passthrough mode that keys out a physical greenscreen by applying a Meta passthrough Color LUT with alpha to the passthrough layer.
- Composite passthrough as an OVERLAY above the streamed projection layer for this mode only.

Hard requirements
- Minimal, surgical edits. No refactors.
- Work in compile-safe checkpoints. After each checkpoint, run the build commands below.
- Commit after each checkpoint (one checkpoint = one commit).

Failure handling (automation-friendly)
- If a checkpoint build fails, DO NOT proceed to the next checkpoint.
- Debug and fix within the current checkpoint, then rerun the build commands.
- Try up to 3 fix attempts within the checkpoint.
- If still failing after 3 attempts or a human decision is required, stop and report:
  - the failing command
  - the full error output
  - current git status
  - last commit hash

Build commands (run after every checkpoint)
- cargo build -p alvr_session
- cargo build -p alvr_graphics
- cargo build -p alvr_client_openxr

Non-goals
- Do not change decode, timing, reprojection, foveation, or transport code.
- Do not alter existing passthrough modes (Blend/RGB/HSV) except where required to compile after adding a new enum variant.

Final acceptance
- New passthrough mode exists: Meta LUT Overlay (greenscreen).
- For Meta LUT Overlay only:
  - Projection layer is fully opaque (no alpha flags on projection).
  - Passthrough layer has LUT style applied (XR_META_passthrough_color_lut) and is submitted as an overlay (after projection in xrEndFrame).
- Existing modes remain unchanged and still work.
