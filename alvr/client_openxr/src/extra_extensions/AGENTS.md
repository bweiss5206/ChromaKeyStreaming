extra_extensions FFI rules (META LUT)

- Follow existing patterns in this folder: repr(C) structs + typed function pointers + xrGetInstanceProcAddr loading.
- Use exact OpenXR entrypoint names:
  - xrCreatePassthroughColorLutMETA
  - xrDestroyPassthroughColorLutMETA
  - xrPassthroughLayerSetStyleFB
- Use META structs chained into XrPassthroughStyleFB:
  - XrPassthroughColorLutCreateInfoMETA
  - XrPassthroughColorLutDataMETA
  - XrPassthroughColorMapLutMETA
- First implementation: create LUT once (on enable), apply style on config changes, destroy LUT on Drop.
- Prefer types from openxrs fork if present; else define local raw structs matching spec.
