//! Display cache only: the caller must independently validate clock and task.
pub(crate) fn require_render_area(
    size: (usize, usize),
    operation: &nai_runtime_core::foundation::Operation,
) -> Result<(), &'static str> {
    if size.0 == 0 || size.1 == 0 {
        operation.cancel();
        Err("OUTPUT_WINDOW_UNAVAILABLE")
    } else {
        Ok(())
    }
}
pub struct Presentation {
    displayed: Option<Option<u64>>,
}
impl Default for Presentation {
    fn default() -> Self {
        Self {
            displayed: Some(None),
        }
    }
}
impl Presentation {
    pub fn changed(&self, next: Option<u64>) -> bool {
        self.displayed != Some(next)
    }
    pub fn submitted(&mut self, next: Option<u64>) {
        self.displayed = Some(next);
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn zero_client_area_cancels_descendants_without_using_focus_as_authority() {
        use nai_runtime_core::foundation::Operation;
        for size in [(0, 0), (768, 0), (0, 768)] {
            let op = Operation::model_task(1000).unwrap();
            let child = op.model_child(1000).unwrap();
            assert_eq!(
                require_render_area(size, &op),
                Err("OUTPUT_WINDOW_UNAVAILABLE")
            );
            assert!(child.check().is_err());
        }
        let op = Operation::model_task(1000).unwrap();
        assert!(require_render_area((768, 768), &op).is_ok());
        assert!(op.check().is_ok());
    }
    #[test]
    fn clearing_then_same_frame_requires_redraw() {
        let mut state = Presentation::default();
        assert!(!state.changed(None));
        assert!(state.changed(Some(7)));
        state.submitted(Some(7));
        assert!(!state.changed(Some(7)));
        assert!(state.changed(None));
        state.submitted(None);
        assert!(state.changed(Some(7)));
    }
    #[cfg(windows)]
    #[test]
    #[ignore = "explicit zero-model native UI: minimize only the labeled test window"]
    fn native_minimized_area_cancels_without_models_or_audio() {
        use nai_runtime_core::foundation::Operation;
        use std::{
            path::PathBuf,
            time::{Duration, Instant},
        };
        let dir = PathBuf::from(
            std::env::var_os("NAI_CAPTURE_PROBE_ARTIFACTS").expect("explicit artifacts"),
        );
        std::fs::create_dir_all(&dir).unwrap();
        let mut window = minifb::Window::new(
            "nai · 显示区域故障测试（零模型）",
            512,
            512,
            minifb::WindowOptions::default(),
        )
        .unwrap();
        window.set_target_fps(30);
        let pixels = vec![0x00337799; 512 * 512];
        window.update_with_buffer(&pixels, 512, 512).unwrap();
        // Isolated native UI maintenance, not a model/network deadline.
        let op = Operation::maintenance(180000).unwrap();
        let child = op.maintenance_child(180000).unwrap();
        let mut witness = crate::v561_window_witness::Witness::new(dir.join("window-pixels.json"));
        std::fs::write(
            dir.join("ready.json"),
            b"{\"scope\":\"native_zero_area_guard_not_media_acceptance\"}",
        )
        .unwrap();
        let began = Instant::now();
        loop {
            assert!(
                window.is_open(),
                "Test window was closed; minimization not verified"
            );
            assert!(
                began.elapsed() < Duration::from_secs(175),
                "No zero client area observed; no fault acceptance"
            );
            window.update_with_buffer(&pixels, 512, 512).unwrap();
            let size = window.get_size();
            witness.observe(
                &pixels,
                (512, 512),
                size,
                window.is_active(),
                crate::v561_window_witness::Phase::Frame,
            );
            if require_render_area(size, &op).is_err() {
                assert!(matches!(
                    child.check(),
                    Err(nai_contracts::workbench::WorkbenchError::Cancelled)
                ));
                std::fs::write(dir.join("result.json"),serde_json::to_vec(&serde_json::json!({"scope":"native_zero_area_guard_not_media_acceptance","native_size":size,"error":"OUTPUT_WINDOW_UNAVAILABLE","descendant_cancelled":true,"elapsed_ms":began.elapsed().as_millis(),"model_calls":0,"audio_calls":0})).unwrap()).unwrap();
                break;
            }
        }
    }
}
