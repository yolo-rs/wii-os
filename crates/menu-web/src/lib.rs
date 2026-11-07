use wasm_bindgen::prelude::*;
use wii_menu_core::{MenuAction, MenuCatalog, MenuEffect, MenuModel, Screen};

/// Install the channel preview interactions once for the lifetime of the page.
/// The shared model remains the owner of selection and launch decisions.
#[wasm_bindgen]
pub fn install_menu_interactions() -> Result<(), JsValue> {
    use wasm_bindgen::{JsCast, closure::Closure};
    use web_sys::{Element, Event, HtmlElement};

    let window = web_sys::window().ok_or_else(|| JsValue::from_str("window unavailable"))?;
    let document = window
        .document()
        .ok_or_else(|| JsValue::from_str("document unavailable"))?;
    let body = document.body().ok_or_else(|| JsValue::from_str("body unavailable"))?;
    let handler = Closure::<dyn FnMut(Event)>::new(move |event: Event| {
        let Some(target) = event.target().and_then(|target| target.dyn_into::<Element>().ok()) else {
            return;
        };
        let Some(window) = web_sys::window() else { return };
        let Some(document) = window.document() else { return };
        let Some(body) = document.body() else { return };
        if let Ok(Some(channel)) = target.closest(".occupied .hover") {
            let bounds = channel.get_bounding_client_rect();
            let x = bounds.left() + window.scroll_x().unwrap_or(0.0) + bounds.width() / 2.0;
            let y = bounds.top() + window.scroll_y().unwrap_or(0.0) + bounds.height() / 2.0;
            let origin = format!("{x}px {y}px 0px");
            if let Ok(Some(menu)) = document.query_selector(".main-menu") {
                let _ = menu.class_list().add_1("channel-splash");
                if let Some(html) = menu.dyn_ref::<HtmlElement>() {
                    let _ = html.style().set_property("transform-origin", &origin);
                }
            }
            if let Ok(Some(splash)) = document.query_selector(".splash-screen") {
                if let Some(html) = splash.dyn_ref::<HtmlElement>() {
                    let _ = html.style().set_property("transform-origin", &origin);
                    let image = channel.get_attribute("data-img").unwrap_or_default();
                    let _ = html
                        .style()
                        .set_property("background-image", &format!("url({:?})", image));
                }
            }
            let _ = body.class_list().add_1("channel-splash");
        } else if target.closest(".menu-btn").ok().flatten().is_some() {
            if let Ok(Some(menu)) = document.query_selector(".main-menu") {
                let _ = menu.class_list().remove_1("channel-splash");
            }
            let _ = body.class_list().remove_1("channel-splash");
            let _ = body.class_list().add_1("splash-switch");
        } else {
            return;
        }
        let cleanup = Closure::once_into_js(move || {
            let _ = body.class_list().remove_1("splash-switch");
        });
        let _ = window.set_timeout_with_callback_and_timeout_and_arguments_0(cleanup.unchecked_ref(), 900);
    });
    body.add_event_listener_with_callback("click", handler.as_ref().unchecked_ref())?;
    handler.forget();
    Ok(())
}

#[wasm_bindgen]
pub struct MenuSession {
    model: MenuModel,
    catalog: MenuCatalog,
    last_effect: Option<MenuEffect>,
}

#[wasm_bindgen]
impl MenuSession {
    #[wasm_bindgen(constructor)]
    pub fn new(catalog_json: &str) -> Result<MenuSession, JsValue> {
        let catalog: MenuCatalog = serde_json::from_str(catalog_json)
            .map_err(|error| JsValue::from_str(&format!("invalid Wii OS menu catalog: {error}")))?;
        catalog.validate().map_err(|error| JsValue::from_str(&error))?;
        let model = MenuModel::new(catalog.channels.clone());
        Ok(Self {
            model,
            catalog,
            last_effect: None,
        })
    }

    pub fn dispatch(&mut self, action_json: &str) -> Result<String, JsValue> {
        let action: MenuAction = serde_json::from_str(action_json)
            .map_err(|error| JsValue::from_str(&format!("invalid Wii OS menu action: {error}")))?;
        self.last_effect = self.model.apply(action);
        self.snapshot()
    }

    pub fn focus_and_select(&mut self, slot: usize) -> Result<String, JsValue> {
        self.model.apply(MenuAction::FocusSlot { slot });
        self.last_effect = self.model.apply(MenuAction::Select);
        self.snapshot()
    }

    pub fn move_channel(
        &mut self,
        from_page: usize,
        from_slot: usize,
        to_page: usize,
        to_slot: usize,
    ) -> Result<String, JsValue> {
        self.last_effect = self.model.apply(MenuAction::MoveChannel {
            from_page,
            from_slot,
            to_page,
            to_slot,
        });
        self.snapshot()
    }

    pub fn snapshot(&self) -> Result<String, JsValue> {
        let selected_channel = self.model.selected_channel();
        let screen = match &self.model.screen {
            Screen::HealthAndSafety => "health-and-safety",
            Screen::Home => "home",
            Screen::Preview { .. } => "preview",
            Screen::SdCardMenu => "sd-card-menu",
            Screen::SettingSelect => "setting-select",
            Screen::DataManagement { .. } => "data-management",
            Screen::Settings => "settings",
            Screen::About => "about",
            Screen::Mail => "mail",
            Screen::LetterWriter => "letter-writer",
            Screen::AddressBook => "address-book",
            Screen::HomeButtonMenu => "home-button-menu",
            Screen::NetPlay => "netplay",
            Screen::GamePackager { .. } => "game-packager",
        };
        let pages: Vec<Vec<Option<&wii_menu_core::Channel>>> = self
            .model
            .all_pages()
            .iter()
            .map(|page| page.iter().map(|slot| slot.as_ref()).collect())
            .collect();
        serde_json::to_string(&serde_json::json!({
            "schemaVersion": 1,
            "screen": screen,
            "screenDetail": self.model.screen,
            "page": self.model.current_page(),
            "pageCount": self.model.page_count(),
            "selectedSlot": self.model.selected_slot(),
            "selectedChannel": selected_channel.map(|channel| channel.id.as_str()),
            "channelCount": self.catalog.channels.len(),
            "safeMode": self.model.safe_mode,
            "healthAcknowledged": self.model.health_acknowledged,
            "discInserted": self.model.disc_inserted,
            "sdInserted": self.model.sd_inserted,
            "settingsPage": self.model.settings_page,
            "sysconf": self.model.sysconf,
            "pages": pages,
            "effect": self.last_effect,
        }))
        .map_err(|error| JsValue::from_str(&error.to_string()))
    }
}
