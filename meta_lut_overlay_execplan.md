Meta LUT Overlay (greenscreen) ExecPlan for Codex

Outcome summary
- Add PassthroughMode::MetaLutOverlay that:
  1) Leaves the streamed projection fully opaque.
  2) Applies an RGBA color LUT to the passthrough layer (alpha=0 for green screen range).
  3) Submits passthrough as an overlay above projection by reversing layer order in xrEndFrame for this mode only.

Global constraints
- Do not change existing passthrough modes’ behavior.
- Every checkpoint must compile and be committed.
- After each checkpoint run:
  cargo build -p alvr_session
  cargo build -p alvr_graphics
  cargo build -p alvr_client_openxr

Checkpoints

Checkpoint 0: Preconditions (no code changes)
- Ensure branch: feat/meta-lut-overlay
- Ensure working tree clean
- Run baseline builds (commands above) and confirm success

Checkpoint 1: Add new mode + ensure it compiles (no OpenXR changes yet)
Files:
- alvr/session/src/settings.rs
- alvr/graphics/src/stream.rs
- alvr/client_openxr/src/stream.rs

Steps:
1) In alvr/session/src/settings.rs:
   - Add a new config struct:

     #[derive(SettingsSchema, Serialize, Deserialize, Clone, PartialEq, Debug)]
     pub struct MetaLutOverlayConfig {
         #[schema(flag="real-time")]
         #[schema(gui(slider(min=0.0, max=1.0, step=0.01)))]
         pub weight: f32,

         #[schema(flag="real-time")]
         pub lut_resolution: u32,

         #[schema(flag="real-time")]
         pub hue_center_deg: f32,
         #[schema(flag="real-time")]
         pub hue_width_deg: f32,
         #[schema(flag="real-time")]
         pub sat_min: f32,
         #[schema(flag="real-time")]
         pub val_min: f32,
         #[schema(flag="real-time")]
         pub feather: f32,
     }

   - Add enum variant in PassthroughMode:

     #[schema(strings(display_name = "Meta LUT Overlay (greenscreen)"))]
     MetaLutOverlay(#[schema(flag="real-time")] MetaLutOverlayConfig),

2) In alvr/graphics/src/stream.rs:
   - Update set_passthrough_push_constants() match to include:
     Some(PassthroughMode::MetaLutOverlay(_)) => treat like None:
       passthrough_mode = 0
       alpha = 1.0

   Rationale: MetaLutOverlay does NOT use the video chroma shader.

3) In alvr/client_openxr/src/stream.rs:
   - Where ProjectionLayerAlphaConfig is derived from self.config.passthrough:
     - For MetaLutOverlay, return None (projection fully opaque).
     - For existing modes, keep existing behavior.

Commit:
- Message: "CP1: Add MetaLutOverlay passthrough mode (no video alpha)"

Checkpoint 2: Enable XR_META_passthrough_color_lut at instance creation
Files:
- alvr/client_openxr/src/extra_extensions/multimodal_input.rs (or another constants file)
- alvr/client_openxr/src/lib.rs

Steps:
1) Add constant:
   pub const META_PASSTHROUGH_COLOR_LUT_EXTENSION_NAME: &str = "XR_META_passthrough_color_lut";

2) In alvr/client_openxr/src/lib.rs:
   - Add META_PASSTHROUGH_COLOR_LUT_EXTENSION_NAME to the exts.other allowlist array in the filter() block.

Notes:
- This only enables the extension at xrCreateInstance time. It does not “turn on” LUT behavior. That happens later.

Commit:
- Message: "CP2: Enable XR_META_passthrough_color_lut in extension allowlist"

Checkpoint 3: Add passthrough style update hook (stubbed, compiles)
Files:
- alvr/client_openxr/src/passthrough.rs
- alvr/client_openxr/src/lib.rs
- alvr/client_openxr/src/extra_extensions/passthrough_fb.rs

Steps:
1) In alvr/client_openxr/src/passthrough.rs:
   - Add method on PassthroughLayer:

     pub fn update_style(
         &mut self,
         session: &xr::Session<xr::OpenGlEs>,
         mode: Option<&PassthroughMode>,
     ) {
         if let Some(handle) = &mut self.handle_fb {
             let _ = handle.update_style(session, mode);
         }
     }

   - Ensure PassthroughMode is imported from the same crate used by RealTimeConfig.

2) In alvr/client_openxr/src/lib.rs:
   - In ClientCoreEvent::RealTimeConfig handler, after toggling passthrough_layer on/off:
     - If passthrough_layer exists, call passthrough_layer.update_style(&xr_session, config.passthrough.as_ref()).

3) In alvr/client_openxr/src/extra_extensions/passthrough_fb.rs:
   - Add a placeholder method to PassthroughFB:

     pub fn update_style(
         &mut self,
         _session: &xr::Session<xr::OpenGlEs>,
         _mode: Option<&PassthroughMode>,
     ) -> xr::Result<()> {
         Ok(())
     }

Commit:
- Message: "CP3: Add passthrough style update hook (stub)"

Checkpoint 4: Implement META LUT FFI + apply style when mode is MetaLutOverlay
Files:
- alvr/client_openxr/src/extra_extensions/passthrough_fb.rs

Steps:
1) Extend PassthroughFB struct to store LUT state:
   - Add: meta_lut: Option<MetaLutState>

2) Add META LUT raw structs and PFNs.
   - Prefer using openxrs fork types if available. If not, define local repr(C) structs matching the spec:
     - XrPassthroughColorLutDataMETA
     - XrPassthroughColorLutCreateInfoMETA
     - XrPassthroughColorMapLutMETA
     - Handle type XrPassthroughColorLutMETA (opaque)
   - Add PFNs loaded via xrGetInstanceProcAddr:
     - xrCreatePassthroughColorLutMETA
     - xrDestroyPassthroughColorLutMETA
     - xrPassthroughLayerSetStyleFB

3) Implement LUT generation (first working version)
   - Build an RGBA LUT buffer sized resolution^3 * 4 bytes.
   - Preserve RGB passthrough (RGBout = RGBin).
   - Compute alpha=0 for green-screen region using HSV gating (hue band + sat/value minimums); alpha=255 otherwise.
   - Use cfg.feather optionally for soft edge (okay to start with hard edge).

4) Implement PassthroughFB::update_style(session, mode)
   - If mode is MetaLutOverlay(cfg):
     - Ensure META function pointers loaded.
     - Ensure LUT exists (create once).
     - Call xrPassthroughLayerSetStyleFB(layer_handle, style) with:
       - style.next = &XrPassthroughColorMapLutMETA { colorLut, weight }
       - style.textureOpacityFactor = 1.0
   - If mode is not MetaLutOverlay:
     - Remove LUT mapping by setting a style with next = null (leave opacity factor = 1.0).
   - Log success/failure codes but do not crash.

5) Ensure Drop cleans up LUT:
   - If LUT handle exists, call xrDestroyPassthroughColorLutMETA before destroying passthrough layer/handle.

Commit:
- Message: "CP4: Implement META LUT overlay style on FB passthrough layer"

Checkpoint 5: Reorder xrEndFrame layers for MetaLutOverlay (overlay)
Files:
- alvr/client_openxr/src/lib.rs

Steps:
1) At the point where layers slice is built (immediately before xr_frame_stream.end):
   - Determine whether the active passthrough mode is MetaLutOverlay.
     - Use stream_context (if present) to check stream.config.passthrough matches MetaLutOverlay.
2) If passthrough_layer exists:
   - For MetaLutOverlay: layers = [&projection_layer, passthrough_layer]
   - Otherwise: layers = [passthrough_layer, &projection_layer]

Commit:
- Message: "CP5: Submit passthrough as overlay for MetaLutOverlay mode"

Checkpoint 6: Sanity checks (no functional expansion)
Files:
- as needed

Steps:
1) Search for any other exhaustive matches on PassthroughMode and ensure MetaLutOverlay is handled (compile will usually force this).
2) Verify any uses_passthrough() helpers treat MetaLutOverlay as passthrough-enabled.
3) Ensure Android passthrough feature declaration exists:
   - Verify the produced Android manifest includes com.oculus.feature.PASSTHROUGH (required=false preferred).
   - If missing, add it via the project’s Android manifest generation mechanism.

Commit:
- Message: "CP6: Sanity checks for MetaLutOverlay mode"

Manual test checklist (post-CP5)
- Launch ALVR client on Quest.
- Connect and start stream.
- Switch passthrough mode to "Meta LUT Overlay (greenscreen)".
- Confirm:
  - Physical greenscreen becomes transparent (PCVR visible behind).
  - Your body/props remain visible via passthrough overlay.
  - Switching back to existing modes still behaves exactly as before.
