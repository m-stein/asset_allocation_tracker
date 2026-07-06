use crate::app_backend::AppBackend;
use crate::eframe_app::EframeApp;
use core_lib::{APP_NAME, AccessGrant, UnlockPatternInput};
use eframe::egui;
use std::sync::mpsc::Receiver;

enum LockScreenState<B: AppBackend> {
    Locked,
    Unlocking(Receiver<eyre::Result<AccessGrant>>),
    Unlocked(Box<EframeApp<B>>),
}

pub struct LockScreen<B: AppBackend> {
    backend: Option<B>,
    state: LockScreenState<B>,
    pattern: Vec<u8>,
    message: Option<String>,
}

impl<B: AppBackend> LockScreen<B> {
    const GRID_SIZE: f32 = 240.0;
    const POINT_RADIUS: f32 = 8.0;
    const HIT_RADIUS: f32 = 34.0;
    const LINE_THICKNESS: f32 = 4.0;

    pub fn new(backend: B) -> Self {
        Self {
            backend: Some(backend),
            state: LockScreenState::Locked,
            pattern: Vec::new(),
            message: None,
        }
    }

    fn start_unlock(&mut self) {
        let pattern = self
            .pattern
            .iter()
            .map(u8::to_string)
            .collect::<Vec<_>>()
            .join("");
        self.pattern.clear();
        self.message = None;

        let Some(backend) = &self.backend else {
            self.message = Some("Unlock backend is not available".to_string());
            return;
        };

        self.state =
            LockScreenState::Unlocking(backend.start_unlock(UnlockPatternInput { pattern }));
    }

    fn poll_unlock(&mut self) {
        let LockScreenState::Unlocking(rx) = &self.state else {
            return;
        };
        let Ok(result) = rx.try_recv() else {
            return;
        };

        match result {
            Ok(_) => {
                let Some(backend) = self.backend.take() else {
                    self.message = Some("Unlock backend is not available".to_string());
                    self.state = LockScreenState::Locked;
                    return;
                };

                match EframeApp::new(backend) {
                    Ok(app) => {
                        self.state = LockScreenState::Unlocked(Box::new(app));
                    }
                    Err(err) => {
                        self.message = Some(err.to_string());
                        self.state = LockScreenState::Locked;
                    }
                }
            }
            Err(err) => {
                self.message = Some(err.to_string());
                self.state = LockScreenState::Locked;
            }
        }
    }

    fn show_locked(&mut self, ui: &mut egui::Ui) {
        let is_unlocking = matches!(self.state, LockScreenState::Unlocking(_));
        ui.vertical_centered(|ui| {
            ui.add_space(44.0);
            ui.heading(APP_NAME);
            ui.add_space(24.0);
            ui.label(if is_unlocking {
                "Unlocking..."
            } else {
                "Draw unlock pattern"
            });
            ui.add_space(18.0);
            self.show_pattern_grid(ui, is_unlocking);
            ui.add_space(16.0);
            if let Some(message) = &self.message {
                ui.colored_label(egui::Color32::RED, message);
            }
        });
    }

    fn show_pattern_grid(&mut self, ui: &mut egui::Ui, is_unlocking: bool) {
        let (rect, response) = ui.allocate_exact_size(
            egui::vec2(Self::GRID_SIZE, Self::GRID_SIZE),
            egui::Sense::drag(),
        );
        let painter = ui.painter_at(rect);
        let points = Self::grid_points(rect);
        let pattern_color = ui.visuals().selection.bg_fill;

        if !is_unlocking {
            if response.drag_started() {
                self.pattern.clear();
                self.message = None;
            }

            if response.dragged()
                && let Some(pointer_pos) = response.interact_pointer_pos()
                && let Some(point) = Self::point_at(pointer_pos, &points)
                && !self.pattern.contains(&point)
            {
                self.pattern.push(point);
            }

            if response.drag_stopped() {
                self.start_unlock();
            }
        }

        for pair in self.pattern.windows(2) {
            let from = points[(pair[0] - 1) as usize];
            let to = points[(pair[1] - 1) as usize];
            painter.line_segment(
                [from, to],
                egui::Stroke::new(Self::LINE_THICKNESS, pattern_color),
            );
        }

        for pos in points {
            painter.circle_filled(pos, Self::POINT_RADIUS, pattern_color);
        }
    }

    fn grid_points(rect: egui::Rect) -> [egui::Pos2; 9] {
        let step = Self::GRID_SIZE / 3.0;
        std::array::from_fn(|idx| {
            let col = idx % 3;
            let row = idx / 3;
            egui::pos2(
                rect.left() + step * (col as f32 + 0.5),
                rect.top() + step * (row as f32 + 0.5),
            )
        })
    }

    fn point_at(pointer_pos: egui::Pos2, points: &[egui::Pos2; 9]) -> Option<u8> {
        points
            .iter()
            .position(|point| point.distance(pointer_pos) <= Self::HIT_RADIUS)
            .map(|idx| (idx + 1) as u8)
    }
}

impl<B: AppBackend> eframe::App for LockScreen<B> {
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        self.poll_unlock();
        match &mut self.state {
            LockScreenState::Unlocked(app) => app.ui(ui, frame),
            LockScreenState::Locked | LockScreenState::Unlocking(_) => {
                egui::CentralPanel::default().show_inside(ui, |ui| {
                    self.show_locked(ui);
                });
                if matches!(self.state, LockScreenState::Unlocking(_)) {
                    ui.ctx()
                        .request_repaint_after(std::time::Duration::from_millis(100));
                }
            }
        }
    }
}
