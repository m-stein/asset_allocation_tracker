use std::collections::HashMap;

use egui::{
    Align2, Color32, FontId, Pos2, Rect, Sense, Stroke, StrokeKind,
};

use crate::app::named_distribution::DatedDistribution;


pub fn draw_distribution_history(ui: &mut egui::Ui, title: &str, distr_history: &Vec<DatedDistribution>) {
    let desired_size = egui::vec2(600.0, 300.0);
    let (rect, _response) = ui.allocate_exact_size(desired_size, Sense::hover());
    let painter = ui.painter_at(rect);

    if distr_history.is_empty() {
        painter.text(
            rect.center(),
            Align2::CENTER_CENTER,
            "No data",
            FontId::default(),
            Color32::GRAY,
        );
        return;
    }

    let outer_margin = 12.0;
    let label_height = 20.0;
    let legend_width = 180.0;
    let section_gap = 16.0;

    let chart_rect = Rect::from_min_max(
        Pos2::new(rect.left() + outer_margin, rect.top() + outer_margin),
        Pos2::new(
            rect.right() - outer_margin - legend_width - section_gap,
            rect.bottom() - outer_margin - label_height,
        ),
    );

    let legend_rect = Rect::from_min_max(
        Pos2::new(chart_rect.right() + section_gap, rect.top() + outer_margin),
        Pos2::new(rect.right() - outer_margin, rect.bottom() - outer_margin),
    );

    let bar_count = distr_history.len() as f32;
    let gap = 2.0;
    let total_gap_width = gap * (bar_count - 1.0).max(0.0);
    let bar_width = ((chart_rect.width() - total_gap_width) / bar_count).max(4.0);

    let mut color_map: HashMap<String, Color32> = HashMap::new();
    for dated in distr_history {
        for value in &dated.values {
            color_map
                .entry(value.name.clone())
                .or_insert_with(|| color_from_name(&value.name));
        }
    }

    for (bar_idx, dated) in distr_history.iter().enumerate() {
        let x0 = chart_rect.left() + bar_idx as f32 * (bar_width + gap);
        let x1 = x0 + bar_width;

        let total: f64 = dated.values.iter().map(|v| v.amount).sum();

        if total > 0.0 {
            let mut y_bottom = chart_rect.bottom();

            for (segment_idx, value) in dated.values.iter().enumerate() {
                let fraction = value.amount / total;
                let segment_height = chart_rect.height() * fraction as f32;
                let y_top = y_bottom - segment_height;

                let segment_rect = Rect::from_min_max(
                    Pos2::new(x0, y_top),
                    Pos2::new(x1, y_bottom),
                );

                let color = *color_map.get(&value.name).unwrap_or(&Color32::LIGHT_GRAY);
                painter.rect_filled(segment_rect, 0.0, color);

                let segment_response = ui.interact(
                    segment_rect,
                    egui::Id::new(("distribution_segment", bar_idx, segment_idx)),
                    Sense::hover(),
                );

                if segment_response.hovered() {
                    egui::Tooltip::for_widget(&segment_response).show(|ui| {
                        ui.label(&value.name);
                        ui.label(format!("{:.0}%", fraction * 100.0));
                        ui.label(format!("{:.2}", value.amount));
                    });
                }

                if segment_height >= 18.0 {
                    let percentage_text = format!("{:.0}%", fraction * 100.0);

                    painter.text(
                        segment_rect.center(),
                        Align2::CENTER_CENTER,
                        percentage_text,
                        FontId::proportional(12.0),
                        Color32::BLACK,
                    );
                }

                y_bottom = y_top;
            }
        }

        let bar_rect = Rect::from_min_max(
            Pos2::new(x0, chart_rect.top()),
            Pos2::new(x1, chart_rect.bottom()),
        );

        painter.rect_stroke(
            bar_rect,
            0.0,
            Stroke::new(1.0, Color32::BLACK),
            StrokeKind::Inside,
        );

        let label_pos = Pos2::new((x0 + x1) * 0.5, chart_rect.bottom() + 4.0);

        painter.text(
            label_pos,
            Align2::CENTER_TOP,
            short_date(&dated.date),
            FontId::default(),
            ui.visuals().text_color(),
        );

        let label_rect = Rect::from_center_size(
            label_pos + egui::vec2(0.0, 8.0),
            egui::vec2(bar_width.max(40.0), 20.0),
        );

        let label_response = ui.interact(
            label_rect,
            egui::Id::new(("date_label", bar_idx)),
            Sense::hover(),
        );

        if label_response.hovered() {
            egui::Tooltip::for_widget(&label_response).show(|ui| {
                ui.label(&dated.date);
            });
        }
    }

    draw_distribution_legend(&painter, legend_rect, &color_map, ui, title);
}

fn draw_distribution_legend(
    painter: &egui::Painter,
    rect: Rect,
    color_map: &HashMap<String, Color32>,
    ui: &egui::Ui,
    title: &str,
) {
    let mut entries: Vec<_> = color_map.iter().collect();
    entries.sort_by(|a, b| a.0.cmp(b.0));

    let title_font = FontId::proportional(16.0);
    let text_font = FontId::default();

    painter.text(
        Pos2::new(rect.left(), rect.top()),
        Align2::LEFT_TOP,
        title,
        title_font,
        ui.visuals().text_color(),
    );

    let row_height = 22.0;
    let color_box_size = 14.0;
    let mut y = rect.top() + 28.0;

    for (name, color) in entries {
        if y + row_height > rect.bottom() {
            break;
        }

        let color_rect = Rect::from_min_max(
            Pos2::new(rect.left(), y + 3.0),
            Pos2::new(rect.left() + color_box_size, y + 3.0 + color_box_size),
        );

        painter.rect_filled(color_rect, 2.0, *color);
        painter.rect_stroke(
            color_rect,
            2.0,
            Stroke::new(1.0, Color32::BLACK),
            StrokeKind::Inside,
        );

        painter.text(
            Pos2::new(rect.left() + color_box_size + 8.0, y),
            Align2::LEFT_TOP,
            name,
            text_font.clone(),
            ui.visuals().text_color(),
        );

        y += row_height;
    }
}

fn short_date(date: &str) -> &str {
    if date.len() >= 10 {
        &date[5..10]
    } else {
        date
    }
}

fn color_from_name(name: &str) -> Color32 {
    let mut hash: u32 = 0;
    for b in name.bytes() {
        hash = hash.wrapping_mul(31).wrapping_add(b as u32);
    }

    let r = 80 + ((hash & 0x7F) as u8);
    let g = 80 + (((hash >> 8) & 0x7F) as u8);
    let b = 80 + (((hash >> 16) & 0x7F) as u8);

    Color32::from_rgb(r, g, b)
}