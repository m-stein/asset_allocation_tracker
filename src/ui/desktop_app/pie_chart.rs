use egui::{Color32, Pos2, Shape, Stroke};

use crate::app::named_distribution::NamedDistribution;


pub fn draw_pie_chart(ui: &mut egui::Ui, data: &[NamedDistribution]) {
    let desired_size = egui::vec2(300.0, 300.0);
    let (rect, _response) = ui.allocate_exact_size(desired_size, egui::Sense::hover());

    let painter = ui.painter_at(rect);

    let center = rect.center();
    let radius = rect.width().min(rect.height()) / 2.0;

    let mut start_angle = 0.;
    let mut total_amount = 0.;
    for entry in data {
        total_amount += entry.amount
    }
    for (idx, entry) in data.iter().enumerate() {
        let percentage: f32 = entry.amount as f32 / total_amount as f32;
        let angle = percentage as f32 * std::f32::consts::TAU;

        let segments = 32; // je höher, desto runder
        let mut points = vec![center];

        for j in 0..=segments {
            let t: f32 = start_angle + angle * (j as f32 / segments as f32);
            let x = center.x + radius * t.cos();
            let y = center.y + radius * t.sin();
            points.push(Pos2 { x, y });
        }

        let color = egui::Color32::from_rgb(
            (50 + idx * 40 % 200) as u8,
            (100 + idx * 70 % 150) as u8,
            (150 + idx * 90 % 100) as u8,
        );

        painter.add(Shape::convex_polygon(
            points,
            color,
            Stroke::NONE,
        ));
        let mid_angle = start_angle + angle / 2.0;
        let label_pos = Pos2 {
            x: center.x + radius * 0.6 * mid_angle.cos(),
            y: center.y + radius * 0.6 * mid_angle.sin(),
        };

        painter.text(
            label_pos,
            egui::Align2::CENTER_CENTER,
            format!("{}: {:.0}%", entry.name, percentage * 100.0),
            egui::FontId::default(),
            Color32::BLACK,
        );
        start_angle += angle;
    }
}