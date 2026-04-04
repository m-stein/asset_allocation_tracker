use eframe::egui;

use crate::app::asset_service::AssetService;

pub struct DesktopApp {
    asset_service: AssetService,
    show_add_asset_dialog: bool,
    asset_name_input: String,
    status_message: Option<String>,
}

impl DesktopApp {
    pub fn new(asset_service: AssetService) -> Self {
        Self {
            asset_service,
            show_add_asset_dialog: false,
            asset_name_input: String::new(),
            status_message: None,
        }
    }
}

impl eframe::App for DesktopApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        ui.heading("Asset Allocation Tracker");
        ui.add_space(12.0);

        if ui.button("Add Asset").clicked() {
            self.asset_name_input.clear();
            self.status_message = None;
            self.show_add_asset_dialog = true;
        }

        if let Some(message) = &self.status_message {
            ui.add_space(12.0);
            ui.label(message);
        }

        if self.show_add_asset_dialog {
            let ctx = ui.ctx().clone();
            let mut dialog_open = self.show_add_asset_dialog;
            let mut should_close_after_show = false;

            egui::Window::new("Add Asset")
                .collapsible(false)
                .resizable(false)
                .open(&mut dialog_open)
                .show(&ctx, |ui| {
                    ui.label("Asset name:");
                    ui.text_edit_singleline(&mut self.asset_name_input);

                    ui.add_space(10.0);

                    if ui.button("OK").clicked() {
                        match self.asset_service.add_asset(self.asset_name_input.clone()) {
                            Ok(()) => {
                                self.status_message = Some(format!(
                                    "Asset '{}' was saved.",
                                    self.asset_name_input.trim()
                                ));
                                self.asset_name_input.clear();
                                should_close_after_show = true;
                            }
                            Err(err) => {
                                self.status_message = Some(err.to_string());
                            }
                        }
                    }
                });

            self.show_add_asset_dialog = dialog_open && !should_close_after_show;
        }
    }
}