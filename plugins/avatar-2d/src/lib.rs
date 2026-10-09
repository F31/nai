//! Bounded software RGBA renderer. This is an embeddable core, not a device or
//! host capability registration. Root supplies immutable assets and independent
//! license grants; keeper must still gate frames before any external output.
use nai_contracts::{
    candidate_hash,
    v1::{ContractError, ErrorCode, FrameMetadata, PerformanceCue},
};
use nai_media_core::timeline::audio::{AudioBlock, SampleFormat};
use nai_plugin_sdk::plugin::PluginContext;
use prost::Message;
use serde::Deserialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeSet, VecDeque},
    sync::Arc,
};

type Result<T> = std::result::Result<T, ContractError>;
pub type LicenseGrants = BTreeSet<String>;

fn error(code: ErrorCode, message: &str) -> ContractError {
    ContractError {
        code: code as i32,
        category: "avatar-2d".into(),
        message: message.into(),
        ..Default::default()
    }
}
fn invalid(message: &str) -> ContractError {
    error(ErrorCode::InvalidArgument, message)
}
struct MetadataBudget(usize);
impl std::io::Write for MetadataBudget {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if bytes.len() > 65536 - self.0 {
            return Err(std::io::Error::other("metadata exceeds 64KiB"));
        }
        self.0 += bytes.len();
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
fn schema(value: &Value, source: &str) -> Result<()> {
    if serde_json::to_writer(&mut MetadataBudget(0), value).is_err() {
        return Err(error(
            ErrorCode::ResourceExhausted,
            "avatar metadata exceeds 64KiB",
        ));
    }
    let source: Value =
        serde_json::from_str(source).map_err(|_| invalid("invalid embedded schema"))?;
    let validator =
        jsonschema::validator_for(&source).map_err(|_| invalid("invalid embedded validator"))?;
    if !validator.is_valid(value) {
        return Err(invalid("avatar manifest/config schema rejected"));
    }
    Ok(())
}

/// Bytes already opened/resolved by root. The renderer never opens manifest paths.
pub struct AssetBytes<'a> {
    pub id: &'a str,
    pub bytes: &'a [u8],
}
#[derive(Deserialize)]
struct AssetDeclaration {
    id: String,
    path: String,
    sha256: String,
    license_ref: String,
}
#[derive(Deserialize)]
struct Manifest {
    kind: String,
    license_ref: String,
    assets: Vec<AssetDeclaration>,
    model_binding: String,
    native_width: u32,
    native_height: u32,
    output_width: u32,
    output_height: u32,
    fps_num: u32,
    fps_den: u32,
    supported_actions: Vec<String>,
    supported_emotions: Vec<String>,
}
#[derive(Deserialize, Clone)]
struct Layer {
    asset_id: String,
    role: String,
    width: u32,
    height: u32,
    x: u32,
    y: u32,
}
#[derive(Deserialize)]
struct Config {
    layers: Vec<Layer>,
    sample_rate: u32,
    max_buffer_samples: usize,
    noise_gate: f64,
    full_open_rms: f64,
    attack_ms: f64,
    release_ms: f64,
}
struct Texture {
    layer: Layer,
    pixels: Vec<u8>,
}
pub struct Avatar {
    width: u32,
    height: u32,
    fps_num: u32,
    fps_den: u32,
    config: Config,
    base: Texture,
    closed: Texture,
    open: Texture,
    licenses: LicenseGrants,
}

fn pixels_len(width: u32, height: u32) -> usize {
    width as usize * height as usize * 4
}
fn copy_bytes(bytes: &[u8]) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    out.try_reserve_exact(bytes.len())
        .map_err(|_| error(ErrorCode::ResourceExhausted, "pixel allocation failed"))?;
    out.extend_from_slice(bytes);
    Ok(out)
}
impl Avatar {
    pub fn admit(
        ctx: &PluginContext,
        manifest: &Value,
        config: &Value,
        assets: &[AssetBytes<'_>],
        grants: &LicenseGrants,
    ) -> Result<Self> {
        ctx.check()?;
        schema(
            manifest,
            include_str!("../../../schemas/plugin/avatar.schema.json"),
        )?;
        schema(
            config,
            include_str!("../../../schemas/config/avatar-2d.schema.json"),
        )?;
        let manifest: Manifest = serde_json::from_value(manifest.clone())
            .map_err(|_| invalid("avatar manifest decode failed"))?;
        let config: Config = serde_json::from_value(config.clone())
            .map_err(|_| invalid("2D config decode failed"))?;
        if manifest.kind != "2d" || manifest.model_binding != "software-rgba8-v1" {
            return Err(error(
                ErrorCode::CapabilityNotReady,
                "unsupported renderer binding",
            ));
        }
        if manifest.output_width == 0
            || manifest.output_width > 1920
            || manifest.output_height == 0
            || manifest.output_height > 1080
            || manifest.native_width != manifest.output_width
            || manifest.native_height != manifest.output_height
            || manifest.fps_num == 0
            || manifest.fps_den == 0
            || manifest.fps_num < manifest.fps_den
            || manifest.fps_num as u64 > manifest.fps_den as u64 * 60
            || config.full_open_rms <= config.noise_gate
        {
            return Err(invalid("unsupported dimensions/fps/RMS range"));
        }
        if manifest
            .supported_actions
            .iter()
            .any(|v| !matches!(v.as_str(), "idle" | "speak"))
            || manifest.supported_emotions.iter().any(|v| v != "neutral")
        {
            return Err(error(
                ErrorCode::CapabilityNotReady,
                "unsupported action/emotion declaration",
            ));
        }
        let max_window =
            (config.sample_rate as u64 * manifest.fps_den as u64).div_ceil(manifest.fps_num as u64);
        if config.max_buffer_samples < max_window as usize {
            return Err(invalid("PCM buffer cannot fit one frame"));
        }
        if assets.len() != 3
            || manifest.assets.len() != 3
            || assets.iter().any(|asset| asset.id.len() > 128)
        {
            return Err(invalid("exact three-asset closure required"));
        }
        let mut licenses = LicenseGrants::from([manifest.license_ref.clone()]);
        let mut ids = BTreeSet::new();
        let mut paths = BTreeSet::new();
        for asset in &manifest.assets {
            if !ids.insert(asset.id.clone())
                || !paths.insert(asset.path.clone())
                || asset
                    .path
                    .split('/')
                    .any(|v| v.is_empty() || v == "." || v == "..")
            {
                return Err(invalid("duplicate or aliased asset declaration"));
            }
            licenses.insert(asset.license_ref.clone());
        }
        if !licenses.is_subset(grants) {
            return Err(error(
                ErrorCode::PermissionDenied,
                "avatar license not granted by root",
            ));
        }
        let input_ids: BTreeSet<_> = assets.iter().map(|v| v.id.to_owned()).collect();
        let layer_ids: BTreeSet<_> = config.layers.iter().map(|v| v.asset_id.clone()).collect();
        let roles: BTreeSet<_> = config.layers.iter().map(|v| v.role.as_str()).collect();
        if input_ids.len() != 3 || input_ids != ids || layer_ids != ids || roles.len() != 3 {
            return Err(invalid("duplicate/missing/extra assets or roles"));
        }
        let mut textures = Vec::new();
        for layer in &config.layers {
            ctx.check()?;
            if layer.x as u64 + layer.width as u64 > manifest.output_width as u64
                || layer.y as u64 + layer.height as u64 > manifest.output_height as u64
            {
                return Err(invalid("layer outside canvas"));
            }
            let bytes = assets
                .iter()
                .find(|v| v.id == layer.asset_id)
                .unwrap()
                .bytes;
            if bytes.len() != pixels_len(layer.width, layer.height) {
                return Err(invalid("RGBA layer size mismatch"));
            }
            let declaration = manifest
                .assets
                .iter()
                .find(|v| v.id == layer.asset_id)
                .unwrap();
            let mut digest = Sha256::new();
            for chunk in bytes.chunks(65536) {
                ctx.check()?;
                digest.update(chunk);
            }
            let actual: String = digest
                .finalize()
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect();
            if actual != declaration.sha256 {
                return Err(invalid("asset SHA256 mismatch"));
            }
            textures.push(Texture {
                layer: layer.clone(),
                pixels: copy_bytes(bytes)?,
            });
        }
        let mut take = |role: &str| {
            textures.remove(textures.iter().position(|v| v.layer.role == role).unwrap())
        };
        let base = take("base");
        let closed = take("mouth_closed");
        let open = take("mouth_open");
        if base.layer.x != 0
            || base.layer.y != 0
            || base.layer.width != manifest.output_width
            || base.layer.height != manifest.output_height
            || (
                closed.layer.x,
                closed.layer.y,
                closed.layer.width,
                closed.layer.height,
            ) != (
                open.layer.x,
                open.layer.y,
                open.layer.width,
                open.layer.height,
            )
        {
            return Err(invalid("base or mouth layer geometry mismatch"));
        }
        ctx.check()?;
        Ok(Self {
            width: manifest.output_width,
            height: manifest.output_height,
            fps_num: manifest.fps_num,
            fps_den: manifest.fps_den,
            config,
            base,
            closed,
            open,
            licenses,
        })
    }

    pub fn stream(
        self: &Arc<Self>,
        ctx: &PluginContext,
        cue: &PerformanceCue,
    ) -> Result<RenderStream> {
        ctx.check()?;
        if cue.encoded_len() > 65536 {
            return Err(error(
                ErrorCode::ResourceExhausted,
                "render cue exceeds 64KiB",
            ));
        }
        if cue.session_id != ctx.session_id()
            || cue.session_id.len() > 128
            || cue.generation_epoch != ctx.generation_epoch()
            || cue.utterance_id.is_empty()
            || cue.utterance_id.len() > 128
            || cue.expires_at_utc_ms <= ctx.now_utc_ms()
            || cue.hash_algorithm != "sha256-canonical-v1"
            || cue.candidate_hash != candidate_hash(cue)
        {
            return Err(invalid("render cue scope/expiry/hash rejected"));
        }
        if cue.performance.as_ref().is_some_and(|p| {
            !p.emotion.is_empty() && p.emotion != "neutral"
                || !p.gesture.is_empty() && p.gesture != "idle"
                || !p.gaze.is_empty() && p.gaze != "center"
                || !p.intensity.is_finite()
                || !(0.0..=1.0).contains(&p.intensity)
        }) {
            return Err(error(
                ErrorCode::CapabilityNotReady,
                "unsupported performance cue",
            ));
        }
        Ok(RenderStream {
            avatar: self.clone(),
            session: cue.session_id.clone(),
            utterance: cue.utterance_id.clone(),
            epoch: cue.generation_epoch,
            expiry: cue.expires_at_utc_ms,
            pcm: VecDeque::new(),
            previous: None,
            frame_seq: 0,
            consumed: 0,
            mouth: 0.0,
            revoked: false,
        })
    }
}

pub struct RenderedFrame {
    pub metadata: FrameMetadata,
    pub rgba: Vec<u8>,
    pub mouth_level: f64,
}
pub struct RenderStream {
    avatar: Arc<Avatar>,
    session: String,
    utterance: String,
    epoch: u64,
    expiry: u64,
    pcm: VecDeque<f32>,
    previous: Option<AudioBlock>,
    frame_seq: u64,
    consumed: u64,
    mouth: f64,
    revoked: bool,
}
impl RenderStream {
    fn check(&mut self, ctx: &PluginContext, grants: Option<&LicenseGrants>) -> Result<()> {
        if self.revoked {
            return Err(error(ErrorCode::Cancelled, "render stream revoked"));
        }
        let failure = ctx.check().err().or_else(|| {
            if ctx.session_id() != self.session || ctx.generation_epoch() != self.epoch {
                Some(error(ErrorCode::StaleEpoch, "render epoch/session changed"))
            } else if ctx.now_utc_ms() >= self.expiry {
                Some(error(ErrorCode::DeadlineExceeded, "render cue expired"))
            } else if grants.is_some_and(|g| !self.avatar.licenses.is_subset(g)) {
                Some(error(ErrorCode::PermissionDenied, "avatar license revoked"))
            } else {
                None
            }
        });
        if let Some(failure) = failure {
            self.revoked = true;
            return Err(failure);
        }
        Ok(())
    }
    pub fn buffered_samples(&self) -> usize {
        self.pcm.len()
    }
    pub fn complete(&self) -> bool {
        !self.revoked
            && self.previous.as_ref().is_some_and(|v| v.end_of_stream)
            && self.pcm.is_empty()
    }
    pub fn push(
        &mut self,
        ctx: &PluginContext,
        grants: &LicenseGrants,
        block: AudioBlock,
        pcm: &[f32],
    ) -> Result<()> {
        self.check(ctx, Some(grants))?;
        block
            .validate(self.previous.as_ref())
            .map_err(|_| invalid("PCM timeline rejected"))?;
        if block.session_id != self.session
            || block.utterance_id != self.utterance
            || block.generation_epoch != self.epoch
            || block.sample_rate != self.avatar.config.sample_rate
            || block.channels != 1
            || block.format != SampleFormat::F32
            || block.sample_count as usize != pcm.len()
            || self.previous.is_none() && (block.seq != 0 || block.sample_start != 0)
            || self.previous.as_ref().is_some_and(|v| v.seq == u32::MAX)
            || block
                .sample_start
                .checked_add(block.sample_count as u64)
                .is_none()
        {
            return Err(invalid("PCM scope/format/sequence rejected"));
        }
        if pcm.len()
            > self
                .avatar
                .config
                .max_buffer_samples
                .saturating_sub(self.pcm.len())
        {
            return Err(error(ErrorCode::ResourceExhausted, "PCM queue full"));
        }
        for chunk in pcm.chunks(1024) {
            if chunk.iter().any(|v| !v.is_finite() || v.abs() > 1.0) {
                return Err(invalid("nonfinite or out-of-range PCM"));
            }
            self.check(ctx, Some(grants))?;
        }
        self.pcm
            .try_reserve(pcm.len())
            .map_err(|_| error(ErrorCode::ResourceExhausted, "PCM allocation failed"))?;
        self.check(ctx, Some(grants))?;
        self.pcm.extend(pcm.iter().copied());
        self.previous = Some(block);
        Ok(())
    }
    pub fn next_frame(
        &mut self,
        ctx: &PluginContext,
        grants: &LicenseGrants,
        buffer_ref: &str,
    ) -> Result<Option<RenderedFrame>> {
        self.check(ctx, Some(grants))?;
        if buffer_ref.is_empty() || buffer_ref.len() > 256 {
            return Err(invalid("explicit bounded frame buffer reference required"));
        }
        let next_seq = self
            .frame_seq
            .checked_add(1)
            .ok_or_else(|| error(ErrorCode::ResourceExhausted, "frame sequence exhausted"))?;
        let end: u64 = (next_seq as u128
            * self.avatar.config.sample_rate as u128
            * self.avatar.fps_den as u128
            / self.avatar.fps_num as u128)
            .try_into()
            .map_err(|_| invalid("sample timestamp overflow"))?;
        let count: u64 = end
            .checked_sub(self.consumed)
            .ok_or_else(|| invalid("frame timestamp rewind"))?;
        let eos = self.previous.as_ref().is_some_and(|v| v.end_of_stream);
        if self.pcm.is_empty() || self.pcm.len() < count as usize && !eos {
            return Ok(None);
        }
        let count = (count as usize).min(self.pcm.len());
        let mut sum = 0.0;
        for index in 0..count {
            sum += (self.pcm[index] as f64).powi(2);
            if index % 1024 == 0 {
                self.check(ctx, Some(grants))?;
            }
        }
        let rms = (sum / count as f64).sqrt();
        let config = &self.avatar.config;
        let target = ((rms - config.noise_gate) / (config.full_open_rms - config.noise_gate))
            .clamp(0.0, 1.0);
        let tau = if target > self.mouth {
            config.attack_ms
        } else {
            config.release_ms
        };
        let elapsed_ms = count as f64 * 1000.0 / config.sample_rate as f64;
        let mouth = self.mouth + (target - self.mouth) * (1.0 - (-elapsed_ms / tau).exp());
        let mut rgba = copy_bytes(&self.avatar.base.pixels)?;
        let layer = self.avatar.closed.layer.clone();
        for y in 0..layer.height {
            self.check(ctx, Some(grants))?;
            for x in 0..layer.width {
                let src = ((y * layer.width + x) * 4) as usize;
                let dst = (((y + layer.y) * self.avatar.width + x + layer.x) * 4) as usize;
                let mut mouth_pixel = [0u8; 4];
                for (c, value) in mouth_pixel.iter_mut().enumerate() {
                    *value = (self.avatar.closed.pixels[src + c] as f64 * (1.0 - mouth)
                        + self.avatar.open.pixels[src + c] as f64 * mouth)
                        .round() as u8;
                }
                blend(&mut rgba[dst..dst + 4], mouth_pixel);
            }
        }
        let pts = i64::try_from(self.consumed).map_err(|_| invalid("PTS exceeds signed range"))?;
        let metadata = FrameMetadata {
            session_id: self.session.clone(),
            utterance_id: self.utterance.clone(),
            generation_epoch: self.epoch,
            frame_seq: self.frame_seq,
            pts,
            time_base_num: 1,
            time_base_den: self.avatar.config.sample_rate,
            duration: count as u32,
            width: self.avatar.width,
            height: self.avatar.height,
            pixel_format: "rgba8".into(),
            buffer_ref: buffer_ref.into(),
            ..Default::default()
        };
        self.check(ctx, Some(grants))?;
        self.pcm.drain(..count);
        self.consumed += count as u64;
        self.frame_seq = next_seq;
        self.mouth = mouth;
        Ok(Some(RenderedFrame {
            metadata,
            rgba,
            mouth_level: mouth,
        }))
    }
}

fn blend(dst: &mut [u8], src: [u8; 4]) {
    let sa = src[3] as u64;
    let da = dst[3] as u64;
    let alpha = sa * 255 + da * (255 - sa);
    if alpha == 0 {
        dst.fill(0);
        return;
    }
    for c in 0..3 {
        let value = src[c] as u64 * sa * 255 + dst[c] as u64 * da * (255 - sa);
        dst[c] = ((value + alpha / 2) / alpha) as u8;
    }
    dst[3] = ((alpha + 127) / 255) as u8;
}
