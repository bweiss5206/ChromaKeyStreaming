use alvr_common::{error, info};
use alvr_session::{MetaLutOverlayConfig, PassthroughMode};
use alvr_system_info::Platform;
use openxr::{
    self as xr, raw,
    sys::{self, Handle},
};
use std::{ffi::c_void, ptr, sync::LazyLock};

type XrPassthroughColorLutMETA = u64;

static TYPE_PASSTHROUGH_COLOR_LUT_DATA_META: LazyLock<xr::StructureType> =
    LazyLock::new(|| xr::StructureType::from_raw(1000266000));
static TYPE_PASSTHROUGH_COLOR_LUT_CREATE_INFO_META: LazyLock<xr::StructureType> =
    LazyLock::new(|| xr::StructureType::from_raw(1000266001));
static TYPE_PASSTHROUGH_COLOR_MAP_LUT_META: LazyLock<xr::StructureType> =
    LazyLock::new(|| xr::StructureType::from_raw(1000266002));

#[repr(C)]
struct XrPassthroughColorLutDataMETA {
    ty: xr::StructureType,
    next: *const c_void,
    buffer: *const u8,
    buffer_size: u32,
}

#[repr(C)]
struct XrPassthroughColorLutCreateInfoMETA {
    ty: xr::StructureType,
    next: *const c_void,
    lut_type: u32,
    data: XrPassthroughColorLutDataMETA,
}

#[repr(C)]
struct XrPassthroughColorMapLutMETA {
    ty: xr::StructureType,
    next: *const c_void,
    color_lut: XrPassthroughColorLutMETA,
    weight: f32,
}

type CreatePassthroughColorLutMETA = unsafe extern "system" fn(
    sys::PassthroughFB,
    *const XrPassthroughColorLutCreateInfoMETA,
    *mut XrPassthroughColorLutMETA,
) -> sys::Result;
type DestroyPassthroughColorLutMETA = unsafe extern "system" fn(XrPassthroughColorLutMETA) -> sys::Result;
type PassthroughLayerSetStyleFB =
    unsafe extern "system" fn(sys::PassthroughLayerFB, *const sys::PassthroughStyleFB) -> sys::Result;

struct MetaLutState {
    resolution: u32,
    handle: XrPassthroughColorLutMETA,
    destroy_lut: DestroyPassthroughColorLutMETA,
}

pub struct PassthroughFB {
    handle: sys::PassthroughFB,
    layer_handle: sys::PassthroughLayerFB,
    layer: sys::CompositionLayerPassthroughFB,
    ext_fns: raw::PassthroughFB,
    meta_lut: Option<MetaLutState>,
}

impl PassthroughFB {
    pub fn new(session: &xr::Session<xr::OpenGlEs>, platform: Platform) -> xr::Result<Self> {
        let ext_fns = session
            .instance()
            .exts()
            .fb_passthrough
            .ok_or(sys::Result::ERROR_EXTENSION_NOT_PRESENT)?;

        let mut handle = sys::PassthroughFB::NULL;
        let info = sys::PassthroughCreateInfoFB {
            ty: sys::PassthroughCreateInfoFB::TYPE,
            next: ptr::null(),
            flags: sys::PassthroughFlagsFB::IS_RUNNING_AT_CREATION,
        };
        unsafe {
            super::xr_res((ext_fns.create_passthrough)(
                session.as_raw(),
                &info,
                &mut handle,
            ))?
        };

        let mut layer_handle = sys::PassthroughLayerFB::NULL;
        let info = sys::PassthroughLayerCreateInfoFB {
            ty: sys::PassthroughLayerCreateInfoFB::TYPE,
            next: ptr::null(),
            passthrough: handle,
            flags: sys::PassthroughFlagsFB::IS_RUNNING_AT_CREATION,
            purpose: sys::PassthroughLayerPurposeFB::RECONSTRUCTION,
        };
        unsafe {
            super::xr_res((ext_fns.create_passthrough_layer)(
                session.as_raw(),
                &info,
                &mut layer_handle,
            ))?
        };

        let layer = sys::CompositionLayerPassthroughFB {
            ty: sys::CompositionLayerPassthroughFB::TYPE,
            next: ptr::null(),
            flags: xr::CompositionLayerFlags::BLEND_TEXTURE_SOURCE_ALPHA,
            space: sys::Space::NULL,
            layer_handle,
        };

        // HACK: YVR runtime seems to ignore IS_RUNNING_AT_CREATION on versions <= 3.0.1
        if platform.is_yvr() {
            unsafe { super::xr_res((ext_fns.passthrough_start)(handle))? };
        }

        Ok(Self {
            handle,
            layer_handle,
            layer,
            ext_fns,
            meta_lut: None,
        })
    }

    // return reference to make sure the passthrough handle is not dropped while the layer is in use
    pub fn layer(&self) -> &sys::CompositionLayerPassthroughFB {
        &self.layer
    }

    pub fn update_style(
        &mut self,
        session: &xr::Session<xr::OpenGlEs>,
        mode: Option<&PassthroughMode>,
    ) -> xr::Result<()> {
        let set_style: PassthroughLayerSetStyleFB =
            super::get_instance_proc(session, "xrPassthroughLayerSetStyleFB")?;

        match mode {
            Some(PassthroughMode::MetaLutOverlay(config)) => {
                let meta_lut = self.ensure_meta_lut(session, config)?;

                let lut_map = XrPassthroughColorMapLutMETA {
                    ty: *TYPE_PASSTHROUGH_COLOR_MAP_LUT_META,
                    next: ptr::null(),
                    color_lut: meta_lut,
                    weight: config.weight,
                };

                let style = sys::PassthroughStyleFB {
                    ty: sys::PassthroughStyleFB::TYPE,
                    next: ptr::from_ref(&lut_map).cast(),
                    texture_opacity_factor: 1.0,
                    edge_color: xr::Color4f {
                        r: 0.0,
                        g: 0.0,
                        b: 0.0,
                        a: 0.0,
                    },
                };

                unsafe { super::xr_res(set_style(self.layer_handle, &style))? };
                info!("Applied Meta passthrough LUT style");
            }
            _ => {
                let style = sys::PassthroughStyleFB {
                    ty: sys::PassthroughStyleFB::TYPE,
                    next: ptr::null(),
                    texture_opacity_factor: 1.0,
                    edge_color: xr::Color4f {
                        r: 0.0,
                        g: 0.0,
                        b: 0.0,
                        a: 0.0,
                    },
                };

                unsafe { super::xr_res(set_style(self.layer_handle, &style))? };
            }
        }

        Ok(())
    }

    fn ensure_meta_lut(
        &mut self,
        session: &xr::Session<xr::OpenGlEs>,
        cfg: &MetaLutOverlayConfig,
    ) -> xr::Result<XrPassthroughColorLutMETA> {
        if let Some(state) = &self.meta_lut {
            if state.resolution == cfg.lut_resolution {
                return Ok(state.handle);
            }
        }

        self.destroy_meta_lut();

        let create_lut: CreatePassthroughColorLutMETA =
            super::get_instance_proc(session, "xrCreatePassthroughColorLutMETA")?;
        let destroy_lut: DestroyPassthroughColorLutMETA =
            super::get_instance_proc(session, "xrDestroyPassthroughColorLutMETA")?;

        let lut = generate_lut(cfg);
        let lut_data = XrPassthroughColorLutDataMETA {
            ty: *TYPE_PASSTHROUGH_COLOR_LUT_DATA_META,
            next: ptr::null(),
            buffer: lut.as_ptr(),
            buffer_size: lut.len() as u32,
        };
        let create_info = XrPassthroughColorLutCreateInfoMETA {
            ty: *TYPE_PASSTHROUGH_COLOR_LUT_CREATE_INFO_META,
            next: ptr::null(),
            lut_type: 0,
            data: lut_data,
        };

        let mut handle = 0;
        unsafe { super::xr_res(create_lut(self.handle, &create_info, &mut handle))? };

        self.meta_lut = Some(MetaLutState {
            resolution: cfg.lut_resolution,
            handle,
            destroy_lut,
        });

        Ok(handle)
    }

    fn destroy_meta_lut(&mut self) {
        if let Some(state) = self.meta_lut.take() {
            unsafe {
                if let Err(err) = super::xr_res((state.destroy_lut)(state.handle)) {
                    error!("Failed destroying Meta passthrough LUT: {err}");
                }
            }
        }
    }
}

impl Drop for PassthroughFB {
    fn drop(&mut self) {
        self.destroy_meta_lut();

        unsafe {
            (self.ext_fns.destroy_passthrough_layer)(self.layer_handle);
            (self.ext_fns.destroy_passthrough)(self.handle);
        }
    }
}

fn generate_lut(cfg: &MetaLutOverlayConfig) -> Vec<u8> {
    let resolution = cfg.lut_resolution.max(2);
    let mut out = vec![0_u8; (resolution * resolution * resolution * 4) as usize];

    for r in 0..resolution {
        for g in 0..resolution {
            for b in 0..resolution {
                let rf = r as f32 / (resolution - 1) as f32;
                let gf = g as f32 / (resolution - 1) as f32;
                let bf = b as f32 / (resolution - 1) as f32;

                let (hue_deg, sat, val) = rgb_to_hsv_deg(rf, gf, bf);
                let alpha = lut_alpha(hue_deg, sat, val, cfg);

                let idx = ((r * resolution * resolution + g * resolution + b) * 4) as usize;
                out[idx] = (rf * 255.0) as u8;
                out[idx + 1] = (gf * 255.0) as u8;
                out[idx + 2] = (bf * 255.0) as u8;
                out[idx + 3] = alpha;
            }
        }
    }

    out
}

fn lut_alpha(hue_deg: f32, sat: f32, val: f32, cfg: &MetaLutOverlayConfig) -> u8 {
    if sat < cfg.sat_min || val < cfg.val_min {
        return 255;
    }

    let half_width = (cfg.hue_width_deg.abs() * 0.5).max(0.0001);
    let distance = hue_distance_deg(hue_deg, cfg.hue_center_deg);

    if distance >= half_width {
        return 255;
    }

    let feather = cfg.feather.max(0.0);
    if feather <= 0.0 {
        return 0;
    }

    let feather_width = (half_width * feather).max(0.0001);
    let edge_start = (half_width - feather_width).max(0.0);

    if distance <= edge_start {
        0
    } else {
        let t = ((distance - edge_start) / feather_width).clamp(0.0, 1.0);
        (t * 255.0) as u8
    }
}

fn hue_distance_deg(a: f32, b: f32) -> f32 {
    let diff = (a - b).rem_euclid(360.0);
    diff.min(360.0 - diff)
}

fn rgb_to_hsv_deg(r: f32, g: f32, b: f32) -> (f32, f32, f32) {
    let max = r.max(g.max(b));
    let min = r.min(g.min(b));
    let delta = max - min;

    let hue = if delta <= f32::EPSILON {
        0.0
    } else if max == r {
        60.0 * ((g - b) / delta).rem_euclid(6.0)
    } else if max == g {
        60.0 * (((b - r) / delta) + 2.0)
    } else {
        60.0 * (((r - g) / delta) + 4.0)
    };

    let sat = if max <= f32::EPSILON { 0.0 } else { delta / max };
    (hue, sat, max)
}
