use std::{
    cell::RefCell,
    env,
    error::Error,
    fs,
    path::{Path, PathBuf},
    rc::Rc,
};

use gtk::{
    Application, ApplicationWindow, Button, CssProvider, Grid, Label, Orientation, Picture, gdk,
    glib, prelude::*,
};
use wii_menu_core::{MenuAction, MenuCatalog, MenuEffect, MenuModel, Screen};
use wii_menu_runtime::launch_channel;

const STYLE: &str = r#"
window { background: #eaf0f2; color: #25343b; }
.menu-header { font-size: 22px; font-weight: 700; color: #334b55; }
.menu-page { color: #6f828a; font-size: 14px; }
.menu-tile { background: #ffffff; border: 1px solid #b7c5ca; border-radius: 12px; padding: 8px; min-width: 190px; min-height: 130px; }
.menu-tile:focus, .menu-tile:hover { border: 2px solid #69bfce; background: #f8feff; }
.menu-selected { outline: 3px solid #43b5c7; outline-offset: 1px; }
.menu-tile-label { color: #314a54; font-size: 14px; }
.menu-empty { background: #e1e8eb; border: 1px dashed #aebdc2; border-radius: 12px; min-width: 190px; min-height: 130px; }
.menu-footer { background: #d7e0e3; border-radius: 16px; padding: 12px; }
.menu-status { color: #58717a; }
"#;

fn workspace_root() -> PathBuf {
    if let Some(path) = env::var_os("WII_OS_WORKSPACE") {
        return PathBuf::from(path);
    }
    if let Ok(current) = env::current_dir() {
        if let Some(root) = current
            .ancestors()
            .find(|candidate| candidate.join("wii-os/pipeline.ps1").is_file())
        {
            return root.to_path_buf();
        }
        if current.join("wii-os/menu/catalog.example.json").is_file() {
            return current;
        }
    }
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .and_then(Path::parent)
        .unwrap_or(Path::new("."))
        .to_path_buf()
}

fn resolve_asset(workspace: &Path, icon: &str) -> Option<PathBuf> {
    let path = if let Some(relative) = icon.strip_prefix("assets/user/") {
        workspace
            .join("wii-os/userland/data/wii/assets")
            .join(relative)
    } else if let Some(relative) = icon.strip_prefix("assets/images/") {
        workspace
            .join("wii-os/frontend/assets/images")
            .join(relative)
    } else {
        return None;
    };
    path.is_file().then_some(path)
}

fn load_model(workspace: &Path) -> Result<MenuModel, Box<dyn Error>> {
    let default_local = workspace.join("wii-os/userland/data/wii/menu.catalog.json");
    let example = workspace.join("wii-os/menu/catalog.example.json");
    let path = env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .or_else(|| env::var_os("WII_OS_MENU_CATALOG").map(PathBuf::from))
        .unwrap_or_else(|| {
            if default_local.is_file() {
                default_local
            } else {
                example
            }
        });
    let catalog: MenuCatalog = serde_json::from_slice(&fs::read(path)?)?;
    Ok(catalog.into_model()?)
}

fn selected_description(model: &MenuModel) -> String {
    match &model.screen {
        Screen::Preview { channel_id } => model
            .selected_channel()
            .map(|channel| format!("{} — prêt à démarrer", channel.title))
            .unwrap_or_else(|| format!("Canal sélectionné : {channel_id}")),
        Screen::Settings => "Réglages Wii OS · profil Dolphin et répertoires utilisateur".into(),
        Screen::About => "Wii OS · cœur de menu partagé Rust/C/WASM".into(),
        Screen::Mail => "Aucune nouvelle notification".into(),
        Screen::Home => model
            .selected_channel()
            .map(|channel| channel.title.clone())
            .unwrap_or_else(|| "Sélectionne une chaîne".into()),
    }
}

fn refresh_grid(
    grid: &Grid,
    model: &Rc<RefCell<MenuModel>>,
    workspace: &Path,
    description: &Label,
    page_label: &Label,
    status: &Label,
    start: &Button,
) {
    while let Some(child) = grid.first_child() {
        grid.remove(&child);
    }
    let current = model.borrow().current_page();
    let selected_slot = model.borrow().selected_slot();
    let page = model.borrow().page(current).unwrap_or(&[]).to_vec();
    for (slot, channel) in page.into_iter().enumerate() {
        let (button, title) = if let Some(channel) = channel {
            let button = Button::new();
            button.add_css_class("menu-tile");
            let content = gtk::Box::new(Orientation::Vertical, 6);
            if let Some(path) = channel
                .icon
                .as_deref()
                .and_then(|key| resolve_asset(workspace, key))
            {
                let picture = Picture::for_filename(path);
                picture.set_size_request(170, 88);
                picture.set_can_shrink(true);
                content.append(&picture);
            }
            let title = Label::new(Some(&channel.title));
            title.add_css_class("menu-tile-label");
            title.set_wrap(true);
            content.append(&title);
            button.set_child(Some(&content));
            button.set_tooltip_text(Some(&channel.title));
            button.update_property(&[gtk::accessible::Property::Label(&channel.title)]);
            (button, Some(channel.title))
        } else {
            let button = Button::with_label("Chaîne vide");
            button.add_css_class("menu-empty");
            (button, None)
        };
        button.set_hexpand(true);
        button.set_vexpand(true);
        if title.is_some() && slot == selected_slot {
            button.add_css_class("menu-selected");
        }
        if title.is_some() {
            let model = Rc::clone(model);
            let grid_ref = grid.downgrade();
            let desc_ref = description.downgrade();
            let page_ref = page_label.downgrade();
            let status_ref = status.downgrade();
            let start_ref = start.downgrade();
            let workspace = workspace.to_path_buf();
            button.connect_clicked(move |_| {
                model.borrow_mut().apply(MenuAction::FocusSlot { slot });
                model.borrow_mut().apply(MenuAction::Select);
                if let (Some(grid), Some(desc), Some(page), Some(status), Some(start)) = (
                    grid_ref.upgrade(),
                    desc_ref.upgrade(),
                    page_ref.upgrade(),
                    status_ref.upgrade(),
                    start_ref.upgrade(),
                ) {
                    refresh_grid(&grid, &model, &workspace, &desc, &page, &status, &start);
                }
            });
        }
        grid.attach(&button, (slot % 4) as i32, (slot / 4) as i32, 1, 1);
    }
    let model_ref = model.borrow();
    description.set_label(&selected_description(&model_ref));
    page_label.set_label(&format!(
        "Page {} / {}",
        model_ref.current_page() + 1,
        model_ref.page_count()
    ));
    start.set_sensitive(matches!(model_ref.screen, Screen::Preview { .. }));
    if matches!(model_ref.screen, Screen::Home) {
        status.set_label("Clavier : flèches, Entrée, Échap et pages · activation à la souris");
    }
}

fn install_style() {
    if let Some(display) = gdk::Display::default() {
        let provider = CssProvider::new();
        provider.load_from_data(STYLE);
        gtk::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
}

fn main() -> glib::ExitCode {
    let app = Application::builder()
        .application_id("org.wii-os.Menu")
        .build();
    app.connect_activate(|app| {
        install_style();
        let workspace = workspace_root();
        let model = match load_model(&workspace) {
            Ok(model) => Rc::new(RefCell::new(model)),
            Err(error) => {
                eprintln!("Wii OS GTK: catalogue illisible: {error}");
                return;
            }
        };
        let window = ApplicationWindow::builder().application(app).title("Wii OS").default_width(1280).default_height(820).build();
        let root = gtk::Box::new(Orientation::Vertical, 14);
        root.set_margin_top(18); root.set_margin_bottom(18); root.set_margin_start(22); root.set_margin_end(22);
        let header = gtk::Box::new(Orientation::Horizontal, 12);
        let heading = Label::new(Some("Menu Wii OS")); heading.add_css_class("menu-header"); heading.set_xalign(0.0); heading.set_hexpand(true);
        let page_label = Label::new(None); page_label.add_css_class("menu-page");
        header.append(&heading); header.append(&page_label); root.append(&header);

        let grid = Grid::new(); grid.set_row_spacing(10); grid.set_column_spacing(10); grid.set_hexpand(true); grid.set_vexpand(true);
        let aspect = gtk::AspectFrame::builder().ratio(4.0 / 3.0).obey_child(false).hexpand(true).vexpand(true).build();
        aspect.set_child(Some(&grid)); root.append(&aspect);

        let footer = gtk::Box::new(Orientation::Horizontal, 12); footer.add_css_class("menu-footer");
        let description = Label::new(None); description.set_xalign(0.0); description.set_hexpand(true);
        let previous = Button::with_label("‹ Page"); let next = Button::with_label("Page ›");
        let settings = Button::with_label("Réglages"); let back = Button::with_label("Retour"); let start = Button::with_label("Démarrer");
        footer.append(&description); footer.append(&previous); footer.append(&next); footer.append(&settings); footer.append(&back); footer.append(&start); root.append(&footer);
        let status = Label::new(Some("Chargement du catalogue Wii OS")); status.add_css_class("menu-status"); status.set_xalign(0.0); root.append(&status);
        window.set_child(Some(&root));
        refresh_grid(&grid, &model, &workspace, &description, &page_label, &status, &start);

        let keys = gtk::EventControllerKey::new();
        keys.set_propagation_phase(gtk::PropagationPhase::Capture);
        {
            let model = Rc::clone(&model);
            let grid_ref = grid.downgrade();
            let desc_ref = description.downgrade();
            let page_ref = page_label.downgrade();
            let status_ref = status.downgrade();
            let start_ref = start.downgrade();
            let workspace = workspace.clone();
            keys.connect_key_pressed(move |_, key, _, _| {
                let action = match key {
                    gdk::Key::Left => Some(MenuAction::MoveSelection { dx: -1, dy: 0 }),
                    gdk::Key::Right => Some(MenuAction::MoveSelection { dx: 1, dy: 0 }),
                    gdk::Key::Up => Some(MenuAction::MoveSelection { dx: 0, dy: -1 }),
                    gdk::Key::Down => Some(MenuAction::MoveSelection { dx: 0, dy: 1 }),
                    gdk::Key::Page_Up => Some(MenuAction::ChangePage { delta: -1 }),
                    gdk::Key::Page_Down => Some(MenuAction::ChangePage { delta: 1 }),
                    gdk::Key::Escape => Some(MenuAction::Back),
                    gdk::Key::Return | gdk::Key::KP_Enter | gdk::Key::space => {
                        Some(match &model.borrow().screen {
                            Screen::Home => MenuAction::Select,
                            Screen::Preview { .. } => MenuAction::Start,
                            _ => MenuAction::Back,
                        })
                    }
                    _ => None,
                };
                let Some(action) = action else { return glib::Propagation::Proceed; };
                let effect = model.borrow_mut().apply(action);
                let mut launch_message = None;
                if let Some(MenuEffect::LaunchChannel { channel_id }) = effect {
                    let result = { launch_channel(&model.borrow(), &workspace, &channel_id) };
                    match result {
                        Ok(pid) => launch_message = Some(format!("Dolphin démarré (PID {pid})")),
                        Err(error) => {
                            model.borrow_mut().apply(MenuAction::LaunchFinished { succeeded: false });
                            launch_message = Some(format!("Échec du lancement : {error}"));
                        }
                    }
                }
                if let (Some(grid), Some(desc), Some(page), Some(status), Some(start)) = (
                    grid_ref.upgrade(), desc_ref.upgrade(), page_ref.upgrade(), status_ref.upgrade(), start_ref.upgrade(),
                ) {
                    refresh_grid(&grid, &model, &workspace, &desc, &page, &status, &start);
                    if let Some(message) = launch_message { status.set_label(&message); }
                }
                glib::Propagation::Stop
            });
        }
        window.add_controller(keys);

        for (button, delta) in [(previous, -1_i8), (next, 1_i8)] {
            let model = Rc::clone(&model); let grid_ref = grid.downgrade(); let desc_ref = description.downgrade();
            let page_ref = page_label.downgrade(); let status_ref = status.downgrade(); let start_ref = start.downgrade();
            let workspace = workspace.clone();
            button.connect_clicked(move |_| {
                model.borrow_mut().apply(MenuAction::ChangePage { delta });
                if let (Some(grid), Some(desc), Some(page), Some(status), Some(start)) =
                    (grid_ref.upgrade(), desc_ref.upgrade(), page_ref.upgrade(), status_ref.upgrade(), start_ref.upgrade()) {
                    refresh_grid(&grid, &model, &workspace, &desc, &page, &status, &start);
                }
            });
        }
        {
            let model = Rc::clone(&model); let grid_ref = grid.downgrade(); let desc_ref = description.downgrade();
            let page_ref = page_label.downgrade(); let status_ref = status.downgrade(); let start_ref = start.downgrade();
            let workspace = workspace.clone();
            settings.connect_clicked(move |_| {
                model.borrow_mut().apply(MenuAction::OpenSettings);
                if let Some(status) = status_ref.upgrade() { status.set_label("Réglages : les fichiers système et les sauvegardes restent dans le profil Dolphin séparé."); }
                if let (Some(grid), Some(desc), Some(page), Some(status), Some(start)) =
                    (grid_ref.upgrade(), desc_ref.upgrade(), page_ref.upgrade(), status_ref.upgrade(), start_ref.upgrade()) {
                    refresh_grid(&grid, &model, &workspace, &desc, &page, &status, &start);
                }
            });
        }
        {
            let model = Rc::clone(&model); let grid_ref = grid.downgrade(); let desc_ref = description.downgrade();
            let page_ref = page_label.downgrade(); let status_ref = status.downgrade(); let start_ref = start.downgrade();
            let workspace = workspace.clone();
            back.connect_clicked(move |_| {
                model.borrow_mut().apply(MenuAction::Back);
                if let (Some(grid), Some(desc), Some(page), Some(status), Some(start)) =
                    (grid_ref.upgrade(), desc_ref.upgrade(), page_ref.upgrade(), status_ref.upgrade(), start_ref.upgrade()) {
                    refresh_grid(&grid, &model, &workspace, &desc, &page, &status, &start);
                }
            });
        }
        {
            let model = Rc::clone(&model); let status = status.downgrade(); let workspace = workspace.clone();
            start.connect_clicked(move |_| {
                let effect = model.borrow_mut().apply(MenuAction::Start);
                if let (Some(MenuEffect::LaunchChannel { channel_id }), Some(status)) = (effect, status.upgrade()) {
                    match launch_channel(&model.borrow(), &workspace, &channel_id) {
                        Ok(pid) => status.set_label(&format!("Dolphin démarré (PID {pid})")),
                        Err(error) => { model.borrow_mut().apply(MenuAction::LaunchFinished { succeeded: false }); status.set_label(&format!("Échec du lancement : {error}")); }
                    }
                }
            });
        }
        window.present();
    });
    app.run()
}
