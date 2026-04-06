use std::collections::HashMap;

use eframe::egui;
use egui_extras::DatePickerButton;
use jiff::civil::Date;
use jiff::Zoned;

use crate::app::asset_service::AssetService;
use crate::domain::allocation_record::{AllocationPosition, AllocationRecord};
use crate::domain::asset::ReferenceType;

pub struct PositionListItem {
    pub id: i64,
    pub label: String,
    pub amount_input: String,
}

pub struct DesktopApp {
    asset_service: AssetService,

    show_add_asset_dialog: bool,
    asset_name_input: String,
    reference_value_input: String,
    selected_reference_type: ReferenceType,

    show_add_allocation_record_dialog: bool,
    allocation_record_date: Date,
    allocation_record_assets: Vec<PositionListItem>,

    show_allocation_dialog: bool,
    latest_allocation_record: Option<AllocationRecord>,
    asset_name_by_id: HashMap<i64, String>,

    status_message: Option<String>,
}

impl DesktopApp {
    pub fn new(asset_service: AssetService) -> Self {
        let allocation_record_assets = asset_service
            .list_assets()
            .unwrap_or_default()
            .into_iter()
            .map(|asset| PositionListItem {
                id: asset.id,
                label: format!("{} ({})", asset.name, asset.reference.value),
                amount_input: String::new(),
            })
            .collect();

        Self {
            asset_service,
            show_add_asset_dialog: false,
            asset_name_input: String::new(),
            reference_value_input: String::new(),
            selected_reference_type: ReferenceType::Isin,

            show_add_allocation_record_dialog: false,
            allocation_record_date: Zoned::now().date(),
            allocation_record_assets,

            show_allocation_dialog: false,
            latest_allocation_record: None,
            asset_name_by_id: HashMap::new(),

            status_message: None,
        }
    }

    fn reset_add_asset_dialog(&mut self) {
        self.asset_name_input.clear();
        self.reference_value_input.clear();
        self.selected_reference_type = ReferenceType::Isin;
    }

    fn reload_asset_list_for_allocation_record(&mut self) {
        match self.asset_service.list_assets() {
            Ok(assets) => {
                self.asset_name_by_id.clear();
                self.allocation_record_assets.clear();

                for asset in assets {
                    self.asset_name_by_id.insert(asset.id, asset.name.clone());

                    self.allocation_record_assets.push(PositionListItem {
                        id: asset.id,
                        label: format!("{} ({})", asset.name, asset.reference.value),
                        amount_input: String::new(),
                    });
                }
            }
            Err(err) => {
                self.status_message = Some(err.to_string());
            }
        }
    }

    fn reset_add_allocation_record_dialog(&mut self) {
        self.allocation_record_date = Zoned::now().date();
        for asset in &mut self.allocation_record_assets {
            asset.amount_input.clear();
        }
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

        egui::CentralPanel::default().show_inside(ui, |ui| {
            ui.heading("Asset Allocation Tracker");
            ui.add_space(12.0);

            ui.horizontal(|ui| {
                if ui.button("Add Asset").clicked() {
                    self.reset_add_asset_dialog();
                    self.status_message = None;
                    self.show_add_asset_dialog = true;
                }

                if ui.button("Add Allocation Record").clicked() {
                    self.reload_asset_list_for_allocation_record();
                    self.reset_add_allocation_record_dialog();
                    self.status_message = None;
                    self.show_add_allocation_record_dialog = true;
                }

                if ui.button("Show Allocation").clicked() {
                    self.reload_asset_list_for_allocation_record();

                    match self.asset_service.get_latest_allocation_record() {
                        Ok(record) => {
                            self.latest_allocation_record = record;
                            self.status_message = None;
                            self.show_allocation_dialog = true;
                        }
                        Err(err) => {
                            self.status_message = Some(err.to_string());
                        }
                    }
                }
            });

            if let Some(message) = &self.status_message {
                ui.add_space(12.0);
                ui.label(message);
            }
        });

        if self.show_allocation_dialog {
            let mut dialog_open = self.show_allocation_dialog;

            egui::Window::new("Current Allocation")
                .collapsible(false)
                .resizable(true)
                .open(&mut dialog_open)
                .show(&ctx, |ui| {
                    if let Some(record) = &self.latest_allocation_record {
                        let total: i64 = record.positions.iter().map(|p| p.amount).sum();

                        if total <= 0 {
                            ui.label("The latest allocation record contains no positive positions.");
                            return;
                        }

                        ui.label(format!(
                            "Based on the latest allocation record from {}:",
                            record.date
                        ));
                        ui.add_space(10.0);

                        for position in &record.positions {
                            let asset_name = self.asset_name_by_id
                                .get(&position.asset_id)
                                .map(|s| s.as_str())
                                .unwrap_or("Unknown asset");

                            let percentage = (position.amount as f64 / total as f64) * 100.0;
                            let fraction = position.amount as f32 / total as f32;

                            ui.label(format!(
                                "{} - {} ({:.1}%)",
                                asset_name, position.amount, percentage
                            ));

                            ui.add(
                                egui::ProgressBar::new(fraction)
                                    .desired_width(320.0)
                                    .text(format!("{:.1}%", percentage)),
                            );

                            ui.add_space(6.0);
                        }
                    } else {
                        ui.label("No allocation record found.");
                    }
                });

            self.show_allocation_dialog = dialog_open;
        }

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

        if self.show_add_allocation_record_dialog {
            let mut dialog_open = self.show_add_allocation_record_dialog;
            let mut should_close_after_show = false;

            egui::Window::new("Add Allocation Record")
                .collapsible(false)
                .resizable(true)
                .open(&mut dialog_open)
                .show(&ctx, |ui| {
                    ui.label("Date:");
                    ui.add(DatePickerButton::new(&mut self.allocation_record_date));

                    ui.add_space(12.0);
                    ui.label("Positions:");

                    egui::ScrollArea::vertical()
                        .max_height(260.0)
                        .show(ui, |ui| {
                            for asset in &mut self.allocation_record_assets {
                                ui.horizontal(|ui| {
                                    ui.add(
                                        egui::TextEdit::singleline(&mut asset.amount_input)
                                            .desired_width(80.0),
                                    );
                                    ui.label(&asset.label);
                                });
                            }
                        });

                    ui.add_space(12.0);

                    ui.horizontal(|ui| {
                        if ui.button("OK").clicked() {
                            let mut positions = Vec::new();
                            let mut validation_error = None;

                            for asset in &self.allocation_record_assets {
                                let trimmed = asset.amount_input.trim();

                                if trimmed.is_empty() {
                                    continue;
                                }

                                let amount = match trimmed.parse::<i64>() {
                                    Ok(value) => value,
                                    Err(_) => {
                                        validation_error = Some(format!(
                                            "Invalid amount for asset '{}'",
                                            asset.label
                                        ));
                                        break;
                                    }
                                };

                                if amount <= 0 {
                                    validation_error = Some(format!(
                                        "Amount must be greater than 0 for asset '{}'",
                                        asset.label
                                    ));
                                    break;
                                }

                                positions.push(AllocationPosition {
                                    asset_id: asset.id,
                                    amount,
                                });
                            }

                            if let Some(message) = validation_error {
                                self.status_message = Some(message);
                            } else {
                                match self.asset_service.add_allocation_record(
                                    self.allocation_record_date,
                                    positions,
                                ) {
                                    Ok(()) => {
                                        self.status_message =
                                            Some("Allocation record was saved.".into());
                                        self.reset_add_allocation_record_dialog();
                                        should_close_after_show = true;
                                    }
                                    Err(err) => {
                                        self.status_message = Some(err.to_string());
                                    }
                                }
                            }
                        }

                        if ui.button("Cancel").clicked() {
                            self.reset_add_allocation_record_dialog();
                            should_close_after_show = true;
                        }
                    });
                });

            self.show_add_allocation_record_dialog = dialog_open && !should_close_after_show;
        }
    }
}