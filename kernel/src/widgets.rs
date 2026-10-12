use crate::gfx::{rgb, Canvas, Rect};

pub const TEXT: u32 = rgb(0xee, 0xee, 0xee);
pub const BORDER: u32 = rgb(0x0a, 0x0b, 0x10);
pub const BUTTON: u32 = rgb(0x2a, 0x30, 0x3c);
pub const BUTTON_HOVER: u32 = rgb(0x3a, 0x42, 0x52);
pub const BUTTON_ACTIVE: u32 = rgb(0x3d, 0x5a, 0x80);
const BAR_FILL: u32 = rgb(0x4a, 0xc0, 0x5a);

pub fn draw_label(canvas: &mut Canvas, x: i32, y: i32, text: &str, scale: i32) {
    canvas.draw_text(x, y, text, TEXT, scale);
}

pub fn draw_button(
    canvas: &mut Canvas,
    rect: Rect,
    label: &str,
    hovered: bool,
    pressed: bool,
    scale: i32,
) {
    let color = match (hovered, pressed) {
        (true, true) => BUTTON_ACTIVE,
        (true, false) => BUTTON_HOVER,
        _ => BUTTON,
    };
    canvas.fill_rect(rect, BORDER);
    canvas.fill_rect(
        Rect::new(
            rect.x + scale,
            rect.y + scale,
            rect.w - 2 * scale,
            rect.h - 2 * scale,
        ),
        color,
    );
    draw_centered_text(canvas, rect, label, scale);
}

pub fn draw_centered_text(canvas: &mut Canvas, rect: Rect, text: &str, scale: i32) {
    let width = text.len() as i32 * 8 * scale;
    let x = rect.x + (rect.w - width) / 2;
    let y = rect.y + (rect.h - 8 * scale) / 2;
    canvas.draw_text(x, y, text, TEXT, scale);
}

pub fn draw_progress(canvas: &mut Canvas, rect: Rect, permille: i32, scale: i32) {
    canvas.fill_rect(rect, BORDER);
    let inner = Rect::new(
        rect.x + scale,
        rect.y + scale,
        rect.w - 2 * scale,
        rect.h - 2 * scale,
    );
    canvas.fill_rect(inner, BUTTON);
    let filled = inner.w * permille.clamp(0, 1000) / 1000;
    canvas.fill_rect(Rect::new(inner.x, inner.y, filled, inner.h), BAR_FILL);
}
