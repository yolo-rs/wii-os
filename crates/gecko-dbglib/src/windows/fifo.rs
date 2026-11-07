use egui::Context;
use egui_phosphor::regular as icons;
use gecko::flipper::gx::recorder::{FifoRecorder, RecorderState};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FifoRecorderAction {
    Start,
    Stop,
    Cancel,
}

fn human_bytes(bytes: u64) -> String {
    if bytes >= 1024 * 1024 {
        format!("{:.1} MiB", bytes as f64 / (1024.0 * 1024.0))
    } else if bytes >= 1024 {
        format!("{:.1} KiB", bytes as f64 / 1024.0)
    } else {
        format!("{bytes} B")
    }
}

pub fn show_fifo_recorder(
    ctx: &Context,
    open: &mut bool,
    recorder: Option<&FifoRecorder>,
    path_input: &mut String,
    last_result: &str,
    action: &mut Option<FifoRecorderAction>,
) {
    egui::Window::new("FIFO Recorder")
        .open(open)
        .resizable(false)
        .default_size(egui::vec2(240.0, 0.0))
        .show(ctx, |ui| {
            ui.set_min_width(220.0);
            let btn_size = egui::vec2(ui.available_width(), 0.0);

            match recorder {
                None => {
                    ui.horizontal(|ui| {
                        ui.label("Output:");
                        egui::TextEdit::singleline(path_input)
                            .desired_width(160.0)
                            .font(egui::TextStyle::Monospace)
                            .show(ui);
                    });

                    if ui
                        .add_enabled(
                            !path_input.trim().is_empty(),
                            egui::Button::new(format!("{} Record", icons::RECORD)).min_size(btn_size),
                        )
                        .clicked()
                    {
                        *action = Some(FifoRecorderAction::Start);
                    }
                    ui.label(egui::RichText::new("Starts at the next presented frame").small().weak());
                }
                Some(rec) => {
                    let state_label = match rec.state() {
                        RecorderState::Waiting => "waiting for a frame",
                        RecorderState::Recording => "recording",
                        RecorderState::Done => "finishing",
                    };
                    ui.label(format!("{} {state_label}", icons::RECORD));
                    ui.label(format!("frames: {}", rec.frames_recorded()));
                    ui.label(format!("stream: {}", human_bytes(rec.fifo_bytes())));
                    ui.label(format!("memory: {}", human_bytes(rec.update_bytes())));

                    ui.separator();
                    if ui
                        .add(egui::Button::new(format!("{} Stop & Save", icons::STOP)).min_size(btn_size))
                        .clicked()
                    {
                        *action = Some(FifoRecorderAction::Stop);
                    }
                    if ui
                        .add(egui::Button::new(format!("{} Cancel", icons::TRASH)).min_size(btn_size))
                        .clicked()
                    {
                        *action = Some(FifoRecorderAction::Cancel);
                    }
                }
            }

            if !last_result.is_empty() {
                ui.separator();
                ui.label(egui::RichText::new(last_result).small());
            }
        });
}
