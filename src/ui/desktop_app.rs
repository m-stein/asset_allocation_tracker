use eframe::egui;

use crate::app::asset_service::AssetService;
use crate::domain::asset::ReferenceType;

pub struct DesktopApp {
    asset_service: AssetService,
    show_add_asset_dialog: bool,
    asset_name_input: String,
    reference_value_input: String,
    selected_reference_type: ReferenceType,
    status_message: Option<String>,
}

impl DesktopApp {
    pub fn new(asset_service: AssetService) -> Self {
        Self {
            asset_service,
            show_add_asset_dialog: false,
            asset_name_input: String::new(),
            reference_value_input: String::new(),
            selected_reference_type: ReferenceType::Isin,
            status_message: None,
        }
    }

    fn reset_add_asset_dialog(&mut self) {
        self.asset_name_input.clear();
        self.reference_value_input.clear();
        self.selected_reference_type = ReferenceType::Isin;
    }

    fn reference_type_label(reference_type: ReferenceType) -> &'static str {
        match reference_type {
            ReferenceType::Iban => "IBAN",
            ReferenceType::Isin => "ISIN",
            ReferenceType::Ticker => "Ticker",
        }
    }
}

impl eframe::App for DesktopApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();

        // Main panel
        egui::CentralPanel::default().show_inside(ui, |ui| {
            ui.heading("Asset Allocation Tracker");
            ui.add_space(12.0);

            if ui.button("Add Asset").clicked() {
                self.reset_add_asset_dialog();
                self.status_message = None;
                self.show_add_asset_dialog = true;
            }

            if let Some(message) = &self.status_message {
                ui.add_space(12.0);
                ui.label(message);
            }
        });

        // Dialog
        if self.show_add_asset_dialog {
            let mut dialog_open = self.show_add_asset_dialog;
            let mut should_close_after_show = false;

            egui::Window::new("Add Asset")
                .collapsible(false)
                .resizable(false)
                .open(&mut dialog_open)
                .show(&ctx, |ui| {
                    ui.label("Asset name:");
                    ui.text_edit_singleline(&mut self.asset_name_input);

                    ui.add_space(8.0);

                    ui.label("Reference type:");
                    egui::ComboBox::from_id_salt("reference_type")
                        .selected_text(Self::reference_type_label(self.selected_reference_type))
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut self.selected_reference_type,
                                ReferenceType::Iban,
                                Self::reference_type_label(ReferenceType::Iban),
                            );
                            ui.selectable_value(
                                &mut self.selected_reference_type,
                                ReferenceType::Isin,
                                Self::reference_type_label(ReferenceType::Isin),
                            );
                            ui.selectable_value(
                                &mut self.selected_reference_type,
                                ReferenceType::Ticker,
                                Self::reference_type_label(ReferenceType::Ticker),
                            );
                        });

                    ui.add_space(8.0);

                    ui.label("Reference value:");
                    ui.text_edit_singleline(&mut self.reference_value_input);

                    ui.add_space(12.0);

                    ui.horizontal(|ui| {
                        if ui.button("OK").clicked() {
                            match self.asset_service.add_asset(
                                self.asset_name_input.clone(),
                                self.selected_reference_type,
                                self.reference_value_input.clone(),
                            ) {
                                Ok(()) => {
                                    self.status_message = Some(format!(
                                        "Asset '{}' was saved.",
                                        self.asset_name_input.trim()
                                    ));
                                    self.reset_add_asset_dialog();
                                    should_close_after_show = true;
                                }
                                Err(err) => {
                                    self.status_message = Some(err.to_string());
                                }
                            }
                        }

                        if ui.button("Cancel").clicked() {
                            self.reset_add_asset_dialog();
                            should_close_after_show = true;
                        }
                    });
                });

            self.show_add_asset_dialog = dialog_open && !should_close_after_show;
        }
    }
}