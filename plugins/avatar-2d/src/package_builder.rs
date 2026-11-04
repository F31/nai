//! Assemble pre-existing RGBA layers. This does not generate images or grant rights.
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

pub struct Layers<'a> {
    pub width: u32,
    pub height: u32,
    pub mouth_width: u32,
    pub mouth_height: u32,
    pub mouth_x: u32,
    pub mouth_y: u32,
    pub base: &'a [u8],
    pub mouth_closed: &'a [u8],
    pub mouth_open: &'a [u8],
}

pub struct PackageMetadata {
    pub manifest: Value,
    pub config: Value,
}

/// No filesystem access, pixel generation, decoding, or authorization.
pub fn assemble(
    layers: &Layers<'_>,
    avatar_id: &str,
    license_ref: &str,
) -> Result<PackageMetadata, &'static str> {
    fn valid_label(s: &str) -> bool {
        !s.is_empty() && s.len() <= 128 && !s.chars().any(char::is_control)
    }
    fn size(w: u32, h: u32) -> Option<usize> {
        usize::try_from(w.checked_mul(h)?.checked_mul(4)?).ok()
    }
    if !valid_label(avatar_id)
        || !valid_label(license_ref)
        || layers.width == 0
        || layers.height == 0
        || layers.width > 1920
        || layers.height > 1080
        || layers.mouth_width == 0
        || layers.mouth_height == 0
        || layers
            .mouth_x
            .checked_add(layers.mouth_width)
            .is_none_or(|x| x > layers.width)
        || layers
            .mouth_y
            .checked_add(layers.mouth_height)
            .is_none_or(|y| y > layers.height)
    {
        return Err("AVATAR_GEOMETRY_REJECTED");
    }
    let base_size = size(layers.width, layers.height).ok_or("AVATAR_SIZE_REJECTED")?;
    let mouth_size = size(layers.mouth_width, layers.mouth_height).ok_or("AVATAR_SIZE_REJECTED")?;
    if base_size != layers.base.len()
        || mouth_size != layers.mouth_closed.len()
        || mouth_size != layers.mouth_open.len()
        || base_size > 8 * 1024 * 1024
        || mouth_size > 8 * 1024 * 1024
        || base_size
            .checked_add(mouth_size.saturating_mul(2))
            .is_none_or(|n| n > 16 * 1024 * 1024)
    {
        return Err("AVATAR_SIZE_REJECTED");
    }
    let hash = |bytes: &[u8]| {
        Sha256::digest(bytes)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    };
    let manifest = json!({"schema_version":1,"avatar_id":avatar_id,"version":"1","kind":"2d","license_ref":license_ref,
        "assets":[{"id":"base","path":"base.rgba","sha256":hash(layers.base),"license_ref":license_ref},
                  {"id":"closed","path":"closed.rgba","sha256":hash(layers.mouth_closed),"license_ref":license_ref},
                  {"id":"open","path":"open.rgba","sha256":hash(layers.mouth_open),"license_ref":license_ref}],
        "model_binding":"software-rgba8-v1","resource_profile_ref":"software-2d","supported_actions":["idle","speak"],
        "supported_emotions":["neutral"],"voice_binding":"default","native_width":layers.width,"native_height":layers.height,
        "output_width":layers.width,"output_height":layers.height,"fps_num":25,"fps_den":1,"base_duration_samples":"0","switch_points":[],"personalization_refs":{}});
    let config = json!({"schema_version":1,"pixel_format":"rgba8","sample_rate":24000,"max_buffer_samples":24000,
        "noise_gate":0.02,"full_open_rms":0.5,"attack_ms":10,"release_ms":100,"layers":[
        {"asset_id":"base","role":"base","width":layers.width,"height":layers.height,"x":0,"y":0},
        {"asset_id":"closed","role":"mouth_closed","width":layers.mouth_width,"height":layers.mouth_height,"x":layers.mouth_x,"y":layers.mouth_y},
        {"asset_id":"open","role":"mouth_open","width":layers.mouth_width,"height":layers.mouth_height,"x":layers.mouth_x,"y":layers.mouth_y}]});
    Ok(PackageMetadata { manifest, config })
}
