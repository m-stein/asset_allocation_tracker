use std::collections::HashMap;

use eframe::egui;
use egui_extras::DatePickerButton;
use jiff::civil::Date;
use jiff::Zoned;
use strum::IntoEnumIterator;

use crate::app::allocation_record::AllocationRecord;
use crate::app::asset_input::AssetInput;
use crate::app::asset_service::AssetService;
use crate::app::allocation_record_input::AllocationPositionInput;
use crate::app::category_assignment_input::CategoryAssignmentInput;
use crate::app::named_distribution::DatedDistribution;
use crate::app::asset_reference_type::AssetReferenceType;
use crate::ui::desktop_app::distribution_history::draw_distribution_history;

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

    allocation_record_date: Date,
    allocation_record_assets: Vec<PositionItem>,

    latest_allocation_record: Option<AllocationRecord>,
    asset_name_by_id: HashMap<i64, String>,

    cfg_catgies_name_input_cnt: usize,
    cfg_catgies_name_inputs: Vec<String>,
    cfg_catgies_id_to_value_input_cnt: HashMap<i64, usize>,
    cfg_catgies_id_to_value_inputs: HashMap<i64, Vec<String>>,

    category_value_name_input: String,
    selected_category_id_for_value: Option<i64>,
    asset_categories: Vec<CategoryItem>,

    alloc_diagram_category_id: Option<i64>,
    alloc_diagram_data: Option<Vec<DatedDistribution>>,

    add_asset_asset_input: AssetInput,
    add_asset_catgy_id_to_assignm_input_cnt: HashMap<i64, i64>,
    add_asset_catgy_id_to_assignm_inputs: HashMap<i64, Vec<CategoryAssignmentInput>>,

    message: Option<String>,

    page: Page,
}

impl DesktopApp {
    const H1_SIZE: f32 = 32.0;
    const H2_SIZE: f32 = 24.0;
    const SPACE_1: f32 = 8.0;
    const SPACE_2: f32 = 12.0;
    const SPACE_3: f32 = 24.0;
    const DEFAULT_INPUT_HEIGHT: f32 = 19.0;
    const SYM_BTN_SIZE: f32 = DesktopApp::DEFAULT_INPUT_HEIGHT;

    pub fn new(asset_service: AssetService) -> Self {
        let mut app = Self {
            asset_service,

            allocation_record_date: Zoned::now().date(),
            allocation_record_assets: Vec::new(),

            latest_allocation_record: None,
            asset_name_by_id: HashMap::new(),

            cfg_catgies_name_input_cnt: 1,
            cfg_catgies_name_inputs: Vec::new(),
            cfg_catgies_id_to_value_input_cnt: HashMap::new(),
            cfg_catgies_id_to_value_inputs: HashMap::new(),

            category_value_name_input: String::new(),
            selected_category_id_for_value: None,
            asset_categories: Vec::new(),

            add_asset_asset_input: AssetInput::default(),
            add_asset_catgy_id_to_assignm_input_cnt: HashMap::new(),

            alloc_diagram_category_id: None,
            alloc_diagram_data: None,

            add_asset_catgy_id_to_assignm_inputs: HashMap::new(),

            message: None,

            page: Page::AllocationDiagram,
        };
        app.init_alocation_diagram_page();
        app.reload_latest_allocation_record();
        app.reload_asset_list_for_allocation_record();
        app
    }

    fn reload_latest_allocation_record(&mut self) {
        match self.asset_service.get_latest_allocation_record() {
            Ok(record) => {
                self.latest_allocation_record = record;
                self.message = None;
            }
            Err(err) => {
                self.latest_allocation_record = None;
                self.message = Some(err.to_string());
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
                self.message = Some(err.to_string());
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
        self.add_asset_asset_input = AssetInput::default();
        self.add_asset_catgy_id_to_assignm_input_cnt.clear();
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
                self.message = Some(err.to_string());
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
        self.message = None;
    }

    fn show_add_allocation_record_page(&mut self, ui: &mut egui::Ui) {

        ui.label(egui::RichText::new("Add Allocation Record").heading().size(Self::H2_SIZE));
        ui.add_space(Self::SPACE_2);

        ui.label("Date:");
        ui.add(DatePickerButton::new(&mut self.allocation_record_date));

        ui.add_space(Self::SPACE_2);
        ui.label("Positions:");

        ui.vertical(|ui| {
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

        ui.add_space(Self::SPACE_2);
        if ui.button("Save").clicked() {
            let mut positions = Vec::new();
            let mut validation_error = None;

            for asset in &self.allocation_record_assets {
                let trimmed = asset.amount_input.trim();

                if trimmed.is_empty() {
                    continue;
                }

                let amount = match trimmed.parse::<f64>() {
                    Ok(value) => value,
                    Err(_) => {
                        validation_error = Some(format!(
                            "Invalid amount for asset '{}'",
                            asset.label
                        ));
                        break;
                    }
                };

                if amount <= 0. {
                    validation_error = Some(format!(
                        "Amount must be greater than 0 for asset '{}'",
                        asset.label
                    ));
                    break;
                }

                positions.push(AllocationPositionInput {
                    asset_id: asset.id,
                    amount,
                });
            }

            if let Some(message) = validation_error {
                self.message = Some(message);
            } else {
                match self.asset_service.add_allocation_record(
                    self.allocation_record_date,
                    positions,
                ) {
                    Ok(()) => {
                        self.message = Some(format!(
                            "Allocation record '{}' was saved.",
                            self.allocation_record_date.to_string()
                        ));
                        self.reset_add_allocation_record_page();
                    }
                    Err(err) => {
                        self.message = Some(err.to_string());
                    }
                }
            }
        }
    }

    fn reload_alloc_diagram_data(&mut self) {
        if let Some(category_id) = self.alloc_diagram_category_id {
            match self.asset_service.get_distribution_for_category(category_id, 5) {
                Ok(data) => {
                    self.alloc_diagram_data = Some(data)
                }
                Err(err) => {
                    self.alloc_diagram_data = None;
                    self.message = Some(err.to_string());
                }
            }
        } else {
            self.alloc_diagram_data = None;
        }
    }

    fn init_alocation_diagram_page(&mut self) {
        self.reload_asset_categories();
        self.reload_alloc_diagram_data();
        self.reload_latest_allocation_record();
    }

    fn show_allocation_diagram_page(&mut self, ui: &mut egui::Ui) {

        ui.label(egui::RichText::new("Allocation Diagram").heading().size(Self::H2_SIZE));
        ui.add_space(Self::SPACE_2);

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
        ui.add_space(Self::SPACE_2);

        if prev_category_id != self.alloc_diagram_category_id {
            self.reload_alloc_diagram_data();
            self.reload_latest_allocation_record();
        }
        if let Some(distr_history) = self.alloc_diagram_data.as_ref() {
            draw_distribution_history(ui, self.allocation_diagram_category_selected_text(), distr_history);
        } else if let Some(record) = &self.latest_allocation_record {
            let total: f64 = record.positions.iter().map(|p| p.amount).sum();

            if total <= 0. {
                ui.label("The latest allocation record contains no positive positions.");
                return;
            }

            ui.label(format!(
                "Record from {}:",
                record.date
            ));
            ui.add_space(10.0);

            for position in &record.positions {

                let percentage = (position.amount as f64 / total as f64) * 100.0;
                let fraction = position.amount as f32 / total as f32;

                ui.label(format!(
                    "{} - {} ({:.1}%)",
                    position.asset.name, position.amount, percentage
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
        self.reload_asset_categories();
        self.message = None;
    }

    fn init_add_category_value_page(&mut self) {
        self.reload_asset_categories();
        self.reset_add_category_value_page();
        self.message = None;
    }
    
    fn show_add_category_value_page(&mut self, ui: &mut egui::Ui) {

        ui.label(egui::RichText::new("Add Category Value").heading().size(Self::H2_SIZE));
        ui.add_space(Self::SPACE_2);

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
        ui.add_space(Self::SPACE_2);
        if ui.button("Save").clicked() {
            let Some(category_id) = self.selected_category_id_for_value else {
                self.message = Some("Please select a category.".into());
                return;
            };
            match self.asset_service.add_asset_category_value(
                category_id,
                self.category_value_name_input.clone(),
            ) {
                Ok(()) => {
                    self.message = Some(format!(
                        "Category value '{}' was saved.",
                        self.category_value_name_input.trim()
                    ));
                    self.reset_add_category_value_page();
                }
                Err(err) => {
                    self.message = Some(err.to_string());
                }
            }
        }
    }

    fn show_configure_categories_page(&mut self, ui: &mut egui::Ui) {

        ui.label(egui::RichText::new("Configure Categories").heading().size(Self::H2_SIZE));
        ui.add_space(Self::SPACE_2);
        if ui.button("Save").clicked() {
            match self.asset_service.add_categories(self.cfg_catgies_name_inputs.clone()) {
                Ok(()) => {
                    self.message = Some(format!(
                        "Ok. Categories added: {}", self.cfg_catgies_name_inputs.join(", ")
                    ));
                    self.cfg_catgies_name_input_cnt = 0;
                }
                Err(err) => {
                    let saved_names: Vec<String> = self.cfg_catgies_name_inputs
                        .drain(0..err.0.min(self.cfg_catgies_name_inputs.len())).collect();
                    self.cfg_catgies_name_input_cnt -= saved_names.len();
                    self.message = Some(format!(
                        "Error: {}. Categories added: {}",
                        err.1.to_string(), saved_names.join(", ")
                    ));
                }
            }
            if self.cfg_catgies_name_input_cnt < 1 {
                self.cfg_catgies_name_inputs.clear();
                self.cfg_catgies_name_input_cnt = 1;
            }
            /* FIXME: save also new category values */
            self.reload_asset_categories();
        }
        ui.add_space(Self::SPACE_2);
        ui.horizontal(|ui| {
            if ui
                .add_sized([Self::SYM_BTN_SIZE, Self::SYM_BTN_SIZE], egui::Button::new("+"))
                .clicked()
            {
                self.cfg_catgies_name_input_cnt += 1;
            }
            ui.label("Categories:");
        });
        self.cfg_catgies_name_inputs.resize_with(self.cfg_catgies_name_input_cnt, || String::new());
        for name_input_idx in (0..self.cfg_catgies_name_input_cnt).rev() {
            ui.horizontal(|ui| {
                ui.label("•");
                ui.text_edit_singleline(&mut self.cfg_catgies_name_inputs[name_input_idx]);

                /* FIXME: allow for adding category values also to new categories */
            });
        }
        for catgy in &self.asset_categories {
            ui.label(format!("• {}", catgy.name));
            ui.horizontal(|ui| {
                ui.add_space(Self::SPACE_3);
                ui.vertical(|ui| {
                    ui.horizontal(|ui| {
                        if ui
                            .add_sized([Self::SYM_BTN_SIZE, Self::SYM_BTN_SIZE], egui::Button::new("+"))
                            .clicked()
                        {
                            self.cfg_catgies_id_to_value_input_cnt
                                .entry(catgy.id)
                                .and_modify(|cnt| *cnt += 1)
                                .or_insert(2);
                        }
                        ui.label("Values:");
                    });
                    let val_input_cnt =
                        *self.cfg_catgies_id_to_value_input_cnt
                            .entry(catgy.id)
                            .or_insert(1);
                    let val_inputs =
                        self.cfg_catgies_id_to_value_inputs
                            .entry(catgy.id)
                            .or_insert(Vec::new());
                    val_inputs.resize_with(val_input_cnt, || String::new());
                    for val_input in val_inputs {
                        ui.horizontal(|ui| {
                            ui.label("•");
                            ui.text_edit_singleline(val_input);
                        });
                    }
                    let values = self.asset_service
                        .list_asset_category_values(catgy.id)
                        .unwrap_or_default();
                    for value in values {
                        ui.label(format!("• {}", value.name));
                    }
                });
            });
        }
    }

    fn init_add_asset_page(&mut self) {

        self.reset_add_asset_page();
        self.reload_asset_categories();
        self.add_asset_catgy_id_to_assignm_inputs.clear();
        self.message = None;
    }

    fn show_add_asset_page(&mut self, ui: &mut egui::Ui) {

        ui.label(egui::RichText::new("Add Asset").heading().size(Self::H2_SIZE));
        ui.add_space(Self::SPACE_2);

        ui.label("Name:");
        ui.text_edit_singleline(&mut self.add_asset_asset_input.name);
        ui.add_space(Self::SPACE_2);

        ui.label("Reference type:");
        egui::ComboBox::from_id_salt("reference_type")
            .selected_text(Self::reference_type_label(self.add_asset_asset_input.reference_type))
            .show_ui(ui, |ui| {
                for t in AssetReferenceType::iter() {
                    ui.selectable_value(
                        &mut self.add_asset_asset_input.reference_type, t, Self::reference_type_label(t));
                }
            });
        ui.add_space(Self::SPACE_2);

        ui.label("Reference value:");
        ui.text_edit_singleline(&mut self.add_asset_asset_input.reference_value);
        ui.vertical(|ui| {
            for catgy in &mut self.asset_categories {

                let selectable_vals =
                    self.asset_service
                        .list_asset_category_values(catgy.id)
                        .unwrap_or_default();

                let assignm_inputs =
                    self.add_asset_catgy_id_to_assignm_inputs
                        .entry(catgy.id)
                        .or_insert(Vec::new());

                let assignm_input_cnt =
                    *self.add_asset_catgy_id_to_assignm_input_cnt
                        .entry(catgy.id)
                        .or_insert(1) as usize;

                ui.add_space(Self::SPACE_2);
                ui.horizontal(|ui| {
                    if assignm_input_cnt < selectable_vals.len() {
                        if ui
                            .add_sized([Self::SYM_BTN_SIZE, Self::SYM_BTN_SIZE], egui::Button::new("+"))
                            .clicked()
                        {
                            self.add_asset_catgy_id_to_assignm_input_cnt
                                .entry(catgy.id)
                                .and_modify(|cnt| *cnt = assignm_input_cnt as i64 + 1)
                                .or_insert(2);
                        }
                    }
                    ui.label(format!(" {}:", &catgy.name));
                });
                ui.add_space(Self::SPACE_1);


                assignm_inputs.resize_with(
                    assignm_input_cnt, || CategoryAssignmentInput { value_id: None, percentage: 0. });

                for input_idx in (0..assignm_input_cnt).rev() {
                    
                    let assignm_input = &mut assignm_inputs[input_idx];
                    if assignm_input_cnt == 1 {
                        assignm_input.percentage = 100.;
                    }

                    let selected_text = assignm_input.value_id
                        .and_then(|id| selectable_vals.iter().find(|val| val.id == id))
                        .map(|val| val.name.clone())
                        .unwrap_or_else(|| "Select...".to_string());

                    ui.horizontal(|ui| {
                        ui.add_enabled_ui(assignm_input_cnt > 1, |ui| {
                            ui.add_sized(
                                [70.0, Self::DEFAULT_INPUT_HEIGHT],
                                egui::DragValue::new(&mut assignm_input.percentage)
                                    .range(0.0..=100.0)
                                    .speed(0.1)
                                    .fixed_decimals(2)
                                    .suffix("%"),
                            );
                        });
                        egui::ComboBox::from_id_salt(format!("{}:{}", catgy.id, input_idx))
                            .selected_text(selected_text)
                            .show_ui(ui, |ui| {
                                for value in &selectable_vals {
                                    ui.selectable_value(
                                        &mut assignm_input.value_id,
                                        Some(value.id),
                                        &value.name,
                                    );
                                }
                            });
                        if input_idx == assignm_input_cnt - 1 {
                            if assignm_input_cnt > 1 {
                                if ui
                                    .add_sized([Self::SYM_BTN_SIZE, Self::SYM_BTN_SIZE], egui::Button::new("-"))
                                    .clicked()
                                {
                                    self.add_asset_catgy_id_to_assignm_input_cnt
                                        .entry(catgy.id)
                                        .and_modify(|cnt| *cnt = assignm_input_cnt as i64 - 1);
                                }
                            }
                        }
                    });
                }
            }
        });
        ui.add_space(Self::SPACE_2);
        if ui.button("Save").clicked() {
            match self.asset_service.add_asset(
                &self.add_asset_asset_input,
                &self.add_asset_catgy_id_to_assignm_inputs
            ) {
                Ok(()) => {
                    self.message = Some(format!("Asset '{}' was saved", self.add_asset_asset_input.name.trim()));
                    self.reset_add_asset_page();
                }
                Err(err) => {
                    self.message = Some(err.to_string());
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
                        Page::AddCategory => self.show_configure_categories_page(ui),
                        Page::AddCategoryValue => self.show_add_category_value_page(ui),
                        Page::AddAllocationRecord => self.show_add_allocation_record_page(ui),
                    }
                    ui.add_space(20.0);

                    ui.label(egui::RichText::new("Message").heading().size(Self::H2_SIZE));
                    ui.add_space(Self::SPACE_2);
                    if let Some(message) = &self.message {
                        ui.label(message);
                    }
                });
            });
        });
    }
}