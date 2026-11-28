use std::{env, fs, path::PathBuf};
fn main() {
    println!("cargo:rerun-if-changed=v561-voice/tauri.conf.json");
    println!("cargo:rerun-if-changed=../../target-windows/v561-voice-ui");
    let root = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("manifest directory"));
    // Original source-generated icon, not a downloaded or unlicensed asset.
    let icons = root.join("icons");
    fs::create_dir_all(&icons).expect("create icon directory");
    let mut rgba = Vec::with_capacity(32 * 32 * 4);
    for y in 0..32 {
        for x in 0..32 {
            let letter = (8..24).contains(&y)
                && ((8..12).contains(&x)
                    || (20..24).contains(&x)
                    || ((10..20).contains(&x) && (8..12).contains(&y)));
            rgba.extend_from_slice(if letter {
                &[63, 220, 175, 255]
            } else {
                &[24, 32, 48, 255]
            });
        }
    }
    let output = fs::File::create(icons.join("icon.png")).expect("create icon");
    let mut encoder = png::Encoder::new(output, 32, 32);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
        .write_header()
        .expect("encode icon header")
        .write_image_data(&rgba)
        .expect("encode icon pixels");
    // ICO supports an embedded PNG payload. Reuse the same original pixels,
    // with a single 32x32/32-bit entry; no external icon tool or artwork.
    let png = fs::read(icons.join("icon.png")).expect("read encoded icon");
    let mut ico = vec![0, 0, 1, 0, 1, 0, 32, 32, 0, 0, 1, 0, 32, 0];
    ico.extend_from_slice(&(png.len() as u32).to_le_bytes());
    ico.extend_from_slice(&22u32.to_le_bytes());
    ico.extend_from_slice(&png);
    fs::write(icons.join("icon.ico"), ico).expect("write Windows icon");
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(
        tauri_build::AppManifest::new().commands(&[
            "workbench_inspect",
            "frontend_ready",
            "v561_live_status",
            "v561_live_prepare",
            "v561_live_start",
            "v561_live_prepare_content",
            "v561_live_play_prepared",
            "v561_live_ask",
            "v561_live_stop",
            "v561_live_config",
            "workbench_all_mute",
            "workbench_mic_stop",
            "workbench_mic_devices",
            "workbench_mic_poll",
            "workbench_human_start",
            "workbench_human_resume",
            "workbench_open",
            "foundation_command",
            "workbench_close",
            "workbench_stop_audition",
            "workbench_audition_nonce",
            "workbench_avatar_choose_and_admit",
            "workbench_avatar_stage_preview",
            "workbench_avatar_choose_photo",
            "workbench_avatar_photo_preview",
            "workbench_avatar_choose_video",
            "workbench_avatar_video_preview",
            "workbench_character_generate",
            "workbench_character_action",
            "workbench_character_select",
            "workbench_character_revoke",
        ]),
    ))
    .expect("build bootstrap ACL and assets");
}

