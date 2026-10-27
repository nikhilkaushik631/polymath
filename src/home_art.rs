//! Original document silhouettes for the Home launch controls. No document data is read.
use crate::workspace::Workspace;
use egui::{Align2, Color32, FontId, Painter, Pos2, Rect, Stroke, StrokeKind, pos2, vec2};

fn gray(value: u8) -> Color32 {
    Color32::from_gray(value)
}
fn block(p: &Painter, origin: Pos2, width: f32, height: f32, color: u8) {
    p.rect_filled(
        Rect::from_min_size(origin, vec2(width, height)),
        1,
        gray(color),
    );
}
fn sheet(p: &Painter, rect: Rect, dark: bool) {
    p.rect_filled(
        rect.translate(vec2(0.0, 4.0)),
        4,
        gray(if dark { 14 } else { 206 }),
    );
    p.rect_filled(rect, 4, Color32::WHITE);
}

pub fn document(p: &Painter, center: Pos2, workspace: Workspace, dark: bool) {
    let ink = gray(31);
    match workspace {
        Workspace::Tools => {
            let board = Rect::from_center_size(center, vec2(136.0, 108.0));
            sheet(p, board, dark);
            p.text(
                board.min + vec2(13.0, 14.0),
                Align2::LEFT_CENTER,
                "PDF",
                FontId::proportional(13.0),
                ink,
            );
            for (index, width) in [73.0, 58.0, 66.0].into_iter().enumerate() {
                let y = board.top() + 39.0 + index as f32 * 20.0;
                p.circle_stroke(pos2(board.left() + 20.0, y), 5.0, Stroke::new(1.4, ink));
                block(p, pos2(board.left() + 35.0, y - 2.0), width, 3.0, 96);
            }
        }
        Workspace::Reader => {
            let back = Rect::from_center_size(center + vec2(11.0, -5.0), vec2(85.0, 113.0));
            p.rect_filled(back, 4, gray(if dark { 92 } else { 201 }));
            let page = Rect::from_center_size(center + vec2(-3.0, 3.0), vec2(87.0, 113.0));
            sheet(p, page, dark);
            let start = page.min + vec2(12.0, 14.0);
            block(p, start, 43.0, 5.0, 31);
            block(p, start + vec2(0.0, 12.0), 30.0, 3.0, 129);
            block(p, start + vec2(-2.0, 29.0), 65.0, 10.0, 218);
            for (i, width) in [60.0, 60.0, 42.0, 60.0, 51.0, 60.0].into_iter().enumerate() {
                block(p, start + vec2(0.0, 32.0 + i as f32 * 8.0), width, 2.0, 136);
            }
            let badge =
                Rect::from_min_size(page.left_bottom() + vec2(-12.0, -26.0), vec2(43.0, 23.0));
            p.rect_filled(badge, 4, ink);
            p.text(
                badge.center(),
                Align2::CENTER_CENTER,
                "PDF",
                FontId::proportional(11.0),
                Color32::WHITE,
            );
        }
        Workspace::Writer => {
            let page = Rect::from_center_size(center, vec2(90.0, 120.0));
            sheet(p, page, dark);
            let start = page.min + vec2(14.0, 13.0);
            p.text(
                start,
                Align2::LEFT_TOP,
                "Aa",
                FontId::proportional(31.0),
                ink,
            );
            p.line_segment(
                [start + vec2(47.0, 2.0), start + vec2(47.0, 33.0)],
                Stroke::new(1.0, gray(152)),
            );
            block(p, start + vec2(0.0, 44.0), 35.0, 4.0, 62);
            for (i, width) in [61.0, 61.0, 46.0, 61.0, 56.0].into_iter().enumerate() {
                block(p, start + vec2(0.0, 56.0 + i as f32 * 7.0), width, 2.0, 171);
            }
            p.text(
                page.center_bottom() - vec2(0.0, 8.0),
                Align2::CENTER_CENTER,
                "1",
                FontId::proportional(6.0),
                gray(113),
            );
        }
        Workspace::Sheets => {
            let page = Rect::from_center_size(center, vec2(138.0, 99.0));
            sheet(p, page, dark);
            let table = page.shrink(8.0);
            p.rect_filled(
                Rect::from_min_size(table.min, vec2(table.width(), 15.0)),
                2,
                ink,
            );
            for (i, label) in ["A", "B", "C", "D"].into_iter().enumerate() {
                p.text(
                    table.min + vec2(23.0 + i as f32 * 27.0, 7.5),
                    Align2::CENTER_CENTER,
                    label,
                    FontId::proportional(7.0),
                    Color32::WHITE,
                );
            }
            for i in 1..6 {
                let y = table.top() + 15.0 + i as f32 * 11.0;
                p.line_segment(
                    [pos2(table.left(), y), pos2(table.right(), y)],
                    Stroke::new(0.6, gray(219)),
                );
                p.text(
                    pos2(table.left() + 4.0, y - 5.0),
                    Align2::CENTER_CENTER,
                    i,
                    FontId::proportional(6.0),
                    gray(134),
                );
            }
            for i in 0..5 {
                let x = table.left() + 10.0 + i as f32 * 27.0;
                p.line_segment(
                    [pos2(x, table.top() + 15.0), pos2(x, table.bottom())],
                    Stroke::new(0.6, gray(219)),
                );
            }
            let selected = Rect::from_min_size(table.min + vec2(37.0, 26.0), vec2(27.0, 11.0));
            p.rect_filled(selected, 0, gray(231));
            p.rect_stroke(selected, 0, Stroke::new(1.2, ink), StrokeKind::Inside);
            p.rect_filled(Rect::from_center_size(selected.max, vec2(3.0, 3.0)), 0, ink);
            for (i, height) in [17.0, 30.0, 24.0, 40.0].into_iter().enumerate() {
                block(
                    p,
                    page.right_bottom() + vec2(-55.0 + i as f32 * 10.0, -12.0 - height),
                    6.0,
                    height,
                    55 + i as u8 * 24,
                );
            }
        }
        Workspace::Slides => {
            let page = Rect::from_center_size(center, vec2(145.0, 91.0));
            p.rect_filled(
                page.translate(vec2(5.0, 6.0)),
                4,
                gray(if dark { 70 } else { 199 }),
            );
            p.rect_filled(page, 4, ink);
            p.text(
                page.min + vec2(13.0, 13.0),
                Align2::LEFT_TOP,
                "Your\nstory.",
                FontId::proportional(18.0),
                Color32::WHITE,
            );
            block(p, page.min + vec2(14.0, 66.0), 41.0, 2.0, 155);
            let c = page.right_center() + vec2(-30.0, 3.0);
            p.circle_stroke(c, 18.0, Stroke::new(7.0, gray(236)));
            p.circle_filled(c + vec2(10.0, -10.0), 7.0, gray(132));
            block(p, page.left_bottom() + vec2(14.0, -10.0), 117.0, 1.0, 91);
        }
        Workspace::Images => {
            let frame = Rect::from_center_size(center, vec2(136.0, 105.0));
            p.rect_filled(
                frame.translate(vec2(5.0, 6.0)),
                5,
                gray(if dark { 70 } else { 199 }),
            );
            p.rect_filled(frame, 5, Color32::WHITE);
            let photo = frame.shrink(8.0);
            p.rect_filled(photo, 2, gray(220));
            p.circle_filled(photo.min + vec2(87.0, 24.0), 11.0, gray(160));
            let ridge = [
                photo.left_bottom() + vec2(0.0, -13.0),
                photo.left_bottom() + vec2(43.0, -61.0),
                photo.left_bottom() + vec2(70.0, -29.0),
                photo.right_bottom() + vec2(0.0, -56.0),
                photo.right_bottom(),
            ];
            p.add(egui::Shape::convex_polygon(
                ridge.to_vec(),
                ink,
                Stroke::NONE,
            ));
            p.line_segment(
                [
                    photo.left_bottom() + vec2(2.0, -10.0),
                    photo.right_bottom() + vec2(-2.0, -10.0),
                ],
                Stroke::new(1.0, gray(152)),
            );
        }
        Workspace::Home => {}
    }
}

pub fn arrow(p: &Painter, center: Pos2, color: Color32) {
    let stroke = Stroke::new(1.5, color);
    p.line_segment([center - vec2(4.0, 0.0), center + vec2(4.0, 0.0)], stroke);
    p.line_segment([center + vec2(0.0, -4.0), center + vec2(4.0, 0.0)], stroke);
    p.line_segment([center + vec2(0.0, 4.0), center + vec2(4.0, 0.0)], stroke);
}
