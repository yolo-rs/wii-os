use renderdoc::{RenderDoc, V100};

pub(crate) struct RenderDocCapture {
    api: Option<RenderDoc<V100>>,
    capture_next_emulated_frame: bool,
    active_frame_capture: bool,
}

impl RenderDocCapture {
    pub(crate) fn new() -> Self {
        let api = match RenderDoc::<V100>::new() {
            Ok(api) => {
                let version = api.get_api_version();
                tracing::info!(
                    major = version.0,
                    minor = version.1,
                    patch = version.2,
                    "RenderDoc API initialized"
                );
                Some(api)
            }
            Err(err) => {
                tracing::warn!(
                    ?err,
                    "RenderDoc API unavailable; captures are disabled until RenderDoc is injected or on PATH"
                );
                None
            }
        };

        Self {
            api,
            capture_next_emulated_frame: false,
            active_frame_capture: false,
        }
    }

    pub(crate) fn request_next_emulated_frame(&mut self) {
        if self.api.is_some() {
            self.capture_next_emulated_frame = true;
            tracing::info!("RenderDoc will capture the next emulated frame");
        } else {
            tracing::warn!("RenderDoc capture requested, but the RenderDoc API is unavailable");
        }
    }

    pub(crate) fn begin_emulated_frame(&mut self) {
        if self.capture_next_emulated_frame {
            self.capture_next_emulated_frame = false;
            self.start_frame_capture();
        }
    }

    pub(crate) fn end_emulated_frame(&mut self) {
        if self.active_frame_capture {
            self.end_frame_capture();
        }
    }

    pub(crate) fn start_frame_capture(&mut self) {
        let Some(api) = &mut self.api else {
            tracing::warn!("RenderDoc start_frame_capture requested, but the RenderDoc API is unavailable");
            return;
        };

        if api.is_frame_capturing() {
            tracing::warn!("RenderDoc is already capturing; ignoring start_frame_capture");
            return;
        }

        api.start_frame_capture(std::ptr::null(), std::ptr::null());
        self.active_frame_capture = true;
        tracing::info!("RenderDoc emulated frame capture started");
    }

    pub(crate) fn end_frame_capture(&mut self) {
        let Some(api) = &mut self.api else {
            tracing::warn!("RenderDoc end_frame_capture requested, but the RenderDoc API is unavailable");
            self.active_frame_capture = false;
            return;
        };

        api.end_frame_capture(std::ptr::null(), std::ptr::null());
        self.active_frame_capture = false;
        tracing::info!("RenderDoc emulated frame capture ended");
    }

    pub(crate) fn trigger_capture(&mut self) {
        let Some(api) = &mut self.api else {
            tracing::warn!("RenderDoc trigger_capture requested, but the RenderDoc API is unavailable");
            return;
        };

        api.trigger_capture();
        tracing::info!("RenderDoc host-frame trigger_capture requested");
    }
}
