use std::collections::HashMap;

use eframe::egui;
use egui_extras::DatePickerButton;
use jiff::civil::Date;
use jiff::Zoned;

use crate::app::asset_service::AssetService;
use crate::domain::allocation_record::{AllocationPosition, AllocationRecord};
use crate::domain::category::Category;
use crate::domain::named_distribution::NamedDistribution;
use crate::domain::asset_reference_type::AssetReferenceType;
use crate::ui::desktop_app::pie_chart::draw_pie_chart;

pub struct PositionItem {
    pub id: i64,
    pub label: String,
    pub amount_input: String,
}

#[derive(Debug, Clone)]
pub struct CategoryItem {
    pub id: i64,
    pub name: String,
}

#[derive(PartialEq)]
enum Page {
    AllocationDiagram,
    AddAsset,
    AddCategory,
    AddCategoryValue,
    AddAllocationRecord,
}

pub struct DesktopApp {
    asset_service: AssetService,

    asset_name_input: String,
    reference_value_input: String,
    selected_reference_type: AssetReferenceType,

    allocation_record_date: Date,
    allocation_record_assets: Vec<PositionItem>,

    latest_allocation_record: Option<AllocationRecord>,
    asset_name_by_id: HashMap<i64, String>,

    category_name_input: String,

    category_value_name_input: String,
    selected_category_id_for_value: Option<i64>,
    asset_categories: Vec<CategoryItem>,

    alloc_diagram_category_id: Option<i64>,
    alloc_diagram_data: Option<Vec<NamedDistribution>>,

    category_id_to_selected_value_id: HashMap<i64, Option<i64>>,

    status_message: Option<String>,

    page: Page,
}

impl DesktopApp {
    const MAIN_PAGE: Page = Page::AllocationDiagram;
    const H1_SIZE: f32 = 32.0;
    const H2_SIZE: f32 = 24.0;

    pub fn new(asset_service: AssetService) -> Self {
        let mut s = Self {
            asset_service,
            asset_name_input: String::new(),
            reference_value_input: String::new(),
            selected_reference_type: AssetReferenceType::Isin,

            allocation_record_date: Zoned::now().date(),
            allocation_record_assets: Vec::new(),

            latest_allocation_record: None,
            asset_name_by_id: HashMap::new(),

            category_name_input: String::new(),

            category_value_name_input: String::new(),
            selected_category_id_for_value: None,
            asset_categories: Vec::new(),

            alloc_diagram_category_id: None,
            alloc_diagram_data: None,

            category_id_to_selected_value_id: HashMap::new(),

            status_message: None,

            page: Page::AllocationDiagram,
        };
        s.init_alocation_diagram_page();
        s.reload_latest_allocation_record();
        s.reload_asset_list_for_allocation_record();
        s
    }

    fn reload_latest_allocation_record(&mut self) {
        match self.asset_service.get_latest_allocation_record() {
            Ok(record) => {
                self.latest_allocation_record = record;
                self.status_message = None;
            }
            Err(err) => {
                self.latest_allocation_record = None;
                self.status_message = Some(err.to_string());
            }
        }
    }

    fn reload_asset_categories(&mut self) {
        match self.asset_service.list_asset_categories() {
            Ok(categories) => {
                self.asset_categories = categories
                    .into_iter()
                    .map(|category| CategoryItem {
                        id: category.id,
                        name: category.name,
                    })
                    .collect();

                self.selected_category_id_for_value = self.asset_categories
                    .first()
                    .map(|category| category.id);
            }
            Err(err) => {
                self.status_message = Some(err.to_string());
            }
        }
    }

    fn selected_category_name_for_value(&self) -> &str {
        match self.selected_category_id_for_value {
            Some(selected_id) => self.asset_categories
                .iter()
                .find(|category| category.id == selected_id)
                .map(|category| category.name.as_str())
                .unwrap_or("Select..."),
            None => "Select...",
        }
    }

    fn allocation_diagram_category_selected_text(&self) -> &str {
        match self.alloc_diagram_category_id {
            Some(category_id) => self.asset_categories
                .iter()
                .find(|category| category.id == category_id)
                .map(|category| category.name.as_str())
                .unwrap_or("Position"),
            None => "Position",
        }
    }

    fn reset_add_category_value_page(&mut self) {
        self.category_value_name_input.clear();
        self.selected_category_id_for_value = self.asset_categories
            .first()
            .map(|category| category.id);
    }

    fn reset_add_asset_page(&mut self) {
        self.asset_name_input.clear();
        self.reference_value_input.clear();
        self.selected_reference_type = AssetReferenceType::Isin;
    }
    
    fn reset_add_category_page(&mut self) {
        self.category_name_input.clear();
    }

    fn reload_asset_list_for_allocation_record(&mut self) {
        match self.asset_service.list_assets() {
            Ok(assets) => {
                self.asset_name_by_id.clear();
                self.allocation_record_assets.clear();

                for asset in assets {
                    self.asset_name_by_id.insert(asset.id, asset.name.clone());

                    self.allocation_record_assets.push(PositionItem {
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

    fn reset_add_allocation_record_page(&mut self) {
        self.allocation_record_date = Zoned::now().date();
        for asset in &mut self.allocation_record_assets {
            asset.amount_input.clear();
        }
    }

    fn reference_type_label(reference_type: AssetReferenceType) -> &'static str {
        match reference_type {
            AssetReferenceType::Iban => "IBAN",
            AssetReferenceType::Isin => "ISIN",
            AssetReferenceType::Ticker => "Ticker",
        }
    }

    fn init_add_allocation_record_page(&mut self) {
        self.reload_asset_list_for_allocation_record();
        self.reset_add_allocation_record_page();
        self.status_message = None;
    }

    fn show_add_allocation_record_page(&mut self, ui: &mut egui::Ui) {

        ui.label(egui::RichText::new("Add Allocation Record").heading().size(Self::H2_SIZE));
        ui.add_space(12.0);

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
        if ui.button("Save").clicked() {
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
                        self.status_message = Some(format!(
                            "Allocation record '{}' was saved.",
                            self.allocation_record_date.to_string()
                        ));
                        self.reset_add_allocation_record_page();
                    }
                    Err(err) => {
                        self.status_message = Some(err.to_string());
                    }
                }
            }
        }
    }

    fn init_alocation_diagram_page(&mut self) {
        self.reload_asset_categories();
    }

    fn show_allocation_diagram_page(&mut self, ui: &mut egui::Ui) {

        ui.label(egui::RichText::new("Allocation Diagram").heading().size(Self::H2_SIZE));
        ui.add_space(12.0);

        ui.label("Category:");

        let prev_category_id = self.alloc_diagram_category_id;
        egui::ComboBox::from_id_salt("allocation_diagram_category")
            .selected_text(self.allocation_diagram_category_selected_text())
            .show_ui(ui, |ui| {
                for category in &self.asset_categories {
                    ui.selectable_value(
                        &mut self.alloc_diagram_category_id,
                        Some(category.id),
                        &category.name,
                    );
                }
                ui.selectable_value(
                    &mut self.alloc_diagram_category_id,
                    None,
                    "Position",
                );
            });
        ui.add_space(12.0);

        if prev_category_id != self.alloc_diagram_category_id {
            if let Some(category_id) = self.alloc_diagram_category_id {
                match self.asset_service.get_distribution_for_category(category_id) {
                    Ok(data) => {
                        self.alloc_diagram_data = Some(data)
                    }
                    Err(err) => {
                        self.alloc_diagram_data = None;
                        self.status_message = Some(err.to_string());
                    }
                }
            } else {
                self.alloc_diagram_data = None;
                self.reload_latest_allocation_record();
            }
            ui.add_space(12.0);
        }
        if let Some(data) = self.alloc_diagram_data.as_ref() {
            draw_pie_chart(ui, &data);
        } else if let Some(record) = &self.latest_allocation_record {
            let total: i64 = record.positions.iter().map(|p| p.amount).sum();

            if total <= 0 {
                ui.label("The latest allocation record contains no positive positions.");
                return;
            }

            ui.label(format!(
                "Record from {}:",
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
        }
    }

    fn show_page_button(
        &mut self,
        ui: &mut egui::Ui,
        page: Page,
        label: &str,
        init_page_fn: fn(&mut Self),
    ) {
        let response = ui.add_sized(
            [180.0, 20.0],
            egui::Button::selectable(self.page == page, label),
        );
        if response.clicked() {
            init_page_fn(self);
            self.page = page;
        }
    }

    fn init_add_category_page(&mut self) {
        self.reset_add_category_page();
        self.status_message = None;
    }

    fn init_add_category_value_page(&mut self) {
        self.reload_asset_categories();
        self.reset_add_category_value_page();
        self.status_message = None;
    }
    
    fn show_add_category_value_page(&mut self, ui: &mut egui::Ui) {

        ui.label(egui::RichText::new("Add Category Value").heading().size(Self::H2_SIZE));
        ui.add_space(12.0);

        ui.label("Category:");
        egui::ComboBox::from_id_salt("asset_category_value_category")
            .selected_text(self.selected_category_name_for_value())
            .show_ui(ui, |ui| {
                for category in &self.asset_categories {
                    ui.selectable_value(
                        &mut self.selected_category_id_for_value,
                        Some(category.id),
                        &category.name,
                    );
                }
            });
        ui.add_space(8.0);
        ui.label("Name:");
        ui.text_edit_singleline(&mut self.category_value_name_input);
        ui.add_space(12.0);
        if ui.button("Save").clicked() {
            let Some(category_id) = self.selected_category_id_for_value else {
                self.status_message = Some("Please select a category.".into());
                return;
            };
            match self.asset_service.add_asset_category_value(
                category_id,
                self.category_value_name_input.clone(),
            ) {
                Ok(()) => {
                    self.status_message = Some(format!(
                        "Category value '{}' was saved.",
                        self.category_value_name_input.trim()
                    ));
                    self.reset_add_category_value_page();
                }
                Err(err) => {
                    self.status_message = Some(err.to_string());
                }
            }
        }
    }

    fn show_add_category_page(&mut self, ui: &mut egui::Ui) {

        ui.label(egui::RichText::new("Add Category").heading().size(Self::H2_SIZE));
        ui.add_space(12.0);

        ui.label("Name:");
        ui.text_edit_singleline(&mut self.category_name_input);
        ui.add_space(12.0);
        if ui.button("Save").clicked() {
            match self.asset_service.add_category(
                self.category_name_input.clone(),
            ) {
                Ok(()) => {
                    self.status_message = Some(format!(
                        "Category '{}' was saved.", self.category_name_input.trim()
                    ));
                    self.page = Self::MAIN_PAGE;
                }
                Err(err) => {
                    self.status_message = Some(err.to_string());
                }
            }
        }
    }

    fn init_add_asset_page(&mut self) {

        self.reset_add_asset_page();
        self.reload_asset_categories();
        self.category_id_to_selected_value_id.clear();
        self.status_message = None;
    }

    fn show_add_asset_page(&mut self, ui: &mut egui::Ui) {

        ui.label(egui::RichText::new("Add Asset").heading().size(Self::H2_SIZE));
        ui.add_space(12.0);

        ui.label("Name:");
        ui.text_edit_singleline(&mut self.asset_name_input);
        ui.add_space(8.0);

        ui.label("Reference type:");
        egui::ComboBox::from_id_salt("reference_type")
            .selected_text(Self::reference_type_label(self.selected_reference_type))
            .show_ui(ui, |ui| {
                ui.selectable_value(
                    &mut self.selected_reference_type,
                    AssetReferenceType::Iban,
                    Self::reference_type_label(AssetReferenceType::Iban),
                );
                ui.selectable_value(
                    &mut self.selected_reference_type,
                    AssetReferenceType::Isin,
                    Self::reference_type_label(AssetReferenceType::Isin),
                );
                ui.selectable_value(
                    &mut self.selected_reference_type,
                    AssetReferenceType::Ticker,
                    Self::reference_type_label(AssetReferenceType::Ticker),
                );
            });
        ui.add_space(8.0);

        ui.label("Reference value:");
        ui.text_edit_singleline(&mut self.reference_value_input);
        ui.add_space(12.0);
        ui.label("Category Values:");
        egui::ScrollArea::vertical()
            .max_height(260.0)
            .show(ui, |ui| {
                for category_item in &mut self.asset_categories {
                    ui.horizontal(|ui| {

                        // get possible values for this category
                        let category = Category { id: category_item.id, name: category_item.name.clone() };
                        let selectable_values = self
                            .asset_service
                            .list_asset_category_values(&category)
                            .unwrap_or_default();

                        // get category value ID of selected item or None
                        let selected_value_id = self
                            .category_id_to_selected_value_id
                            .entry(category_item.id)
                            .or_insert(None);

                        // get text of selected item or "Select..."
                        let selected_text = selected_value_id
                            .and_then(|id| {
                                selectable_values.iter().find(|v| v.id == id)
                            })
                            .map(|v| v.name.clone())
                            .unwrap_or_else(|| "Select...".to_string());

                        // show drop-down for selecting a value for this category
                        egui::ComboBox::from_id_salt(category_item.id)
                            .selected_text(selected_text)
                            .show_ui(ui, |ui| {
                                for value in &selectable_values {
                                    ui.selectable_value(
                                        selected_value_id,
                                        Some(value.id),
                                        &value.name,
                                    );
                                }
                            });
                            
                        // show category name to the right of the drop-down
                        ui.label(&category_item.name);
                    });
                }
            });
        ui.add_space(12.0);
        if ui.button("Save").clicked() {

            let mut category_value_ids: Vec<i64> = Vec::new();
            let mut category_value_not_set = false;
            for (_, valid_opt) in self.category_id_to_selected_value_id.iter() {
                if let Some(valid) = valid_opt {
                    category_value_ids.push(*valid)
                } else {
                    category_value_not_set = true;
                    break;
                };
            }
            if category_value_not_set {
                self.status_message = Some("All category values must be set".into());
            } else {
                match self.asset_service.add_asset(
                    self.asset_name_input.clone(),
                    self.selected_reference_type,
                    self.reference_value_input.clone(),
                    &category_value_ids
                ) {
                    Ok(()) => {
                        self.status_message = Some(format!(
                            "Asset '{}' was saved.",
                            self.asset_name_input.trim()
                        ));
                        self.reset_add_asset_page();
                        self.page = Self::MAIN_PAGE;
                    }
                    Err(err) => {
                        self.status_message = Some(err.to_string());
                    }
                }
            }
        }
    }
}

impl eframe::App for DesktopApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show_inside(ui, |ui| {
            ui.label(egui::RichText::new("Asset Allocation Tracker").heading().size(Self::H1_SIZE));
            ui.add_space(20.0);
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    self.show_page_button(ui, Page::AllocationDiagram, "Allocation Diagram", Self::init_alocation_diagram_page);
                    self.show_page_button(ui, Page::AddAsset, "Add Asset", Self::init_add_asset_page);
                    self.show_page_button(ui, Page::AddCategory, "Add Category", Self::init_add_category_page);
                    self.show_page_button(ui, Page::AddCategoryValue, "Add Category Value", Self::init_add_category_value_page);
                    self.show_page_button(ui, Page::AddAllocationRecord, "Add Allocation Record", Self::init_add_allocation_record_page);
                });
                ui.add_space(20.0);
                ui.vertical(|ui| {
                    match self.page {
                        Page::AddAsset => self.show_add_asset_page(ui),
                        Page::AllocationDiagram => self.show_allocation_diagram_page(ui),
                        Page::AddCategory => self.show_add_category_page(ui),
                        Page::AddCategoryValue => self.show_add_category_value_page(ui),
                        Page::AddAllocationRecord => self.show_add_allocation_record_page(ui),
                    }
                    ui.add_space(20.0);

                    ui.label(egui::RichText::new("Message").heading().size(Self::H2_SIZE));
                    ui.add_space(12.0);
                    if let Some(message) = &self.status_message {
                        ui.label(message);
                    }
                });
            });
        });
    }
}