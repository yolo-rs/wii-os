use egui::{Context, RichText};

const REPO_URL: &str = "https://github.com/ioncodes/gecko";

pub fn show_about(ctx: &Context, open: &mut bool) {
    egui::Window::new("About")
        .open(open)
        .resizable(false)
        .collapsible(false)
        .default_width(280.0)
        .show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(4.0);
                ui.label(RichText::new("Gecko").size(20.0).strong());
                ui.label("GameCube / Wii emulator");
                ui.add_space(6.0);
                ui.hyperlink_to(REPO_URL, REPO_URL);
                ui.add_space(8.0);
            });

            ui.separator();
            ui.add_space(4.0);

            ui.label(RichText::new("Author").strong());
            ui.label("Layle");

            ui.add_space(8.0);

            ui.label(RichText::new("Acknowledgements").strong());
            ui.label("zayd");
            ui.label("vxpm");
            ui.label("hazelwiss");
            ui.label("Dolphin team");
        });
}
