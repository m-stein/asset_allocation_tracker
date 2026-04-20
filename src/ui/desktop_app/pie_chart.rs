use egui::{Color32, Pos2, Stroke, Shape, Mesh};

use crate::app::named_distribution::NamedDistribution;


pub fn draw_pie_chart(ui: &mut egui::Ui, data: &[NamedDistribution]) {
    let desired_size = egui::vec2(300.0, 300.0);
    let (rect, _response) = ui.allocate_exact_size(desired_size, egui::Sense::hover());

    let painter = ui.painter_at(rect);
    if data.is_empty() {
        return;
    }
    let total_amount: f32 = data.iter().map(|entry| entry.amount as f32).sum();
    if total_amount <= 0.0 {
        return;
    }
    let center = rect.center();
    let radius = rect.width().min(rect.height()) / 2.0;

    let mut start_angle = 0.0_f32;
    for (idx, entry) in data.iter().enumerate() {
        let percentage = entry.amount as f32 / total_amount;
        let angle = percentage * std::f32::consts::TAU;

        let color = Color32::from_rgb(
            (50 + idx * 40 % 200) as u8,
            (100 + idx * 70 % 150) as u8,
            (150 + idx * 90 % 100) as u8,
        );

        if angle >= std::f32::consts::TAU - 0.0001 {
            painter.circle_filled(center, radius, color);
        } else {
            let segments = ((angle / std::f32::consts::TAU) * 64.0).ceil().max(2.0) as usize;

            let mut mesh = Mesh::default();
            let center_idx = mesh.vertices.len() as u32;
            mesh.colored_vertex(center, color);

            let mut arc_indices = Vec::with_capacity(segments + 1);

            for j in 0..=segments {
                let t = start_angle + angle * (j as f32 / segments as f32);
                let x = center.x + radius * t.cos();
                let y = center.y + radius * t.sin();
                let idx = mesh.vertices.len() as u32;
                mesh.colored_vertex(Pos2 { x, y }, color);
                arc_indices.push(idx);
            }

            for j in 0..segments {
                mesh.add_triangle(center_idx, arc_indices[j], arc_indices[j + 1]);
            }

            painter.add(Shape::mesh(mesh));
        }

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
    painter.circle_stroke(center, radius, Stroke::new(1.0, Color32::BLACK));
}