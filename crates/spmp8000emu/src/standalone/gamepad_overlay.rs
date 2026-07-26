// Virtual gamepad overlay for standalone diagnostics and demonstrations.

use spmp8000emu_core::input_handler::Button;

const COLOR_IDLE: u32 = 0x00505050;
const COLOR_DPAD_PRESSED: u32 = 0x0000DDFF;
const COLOR_O_PRESSED: u32 = 0x0000EE44;
const COLOR_X_PRESSED: u32 = 0x00FF8800;
const COLOR_START_PRESSED: u32 = 0x00FFE040;
const COLOR_SELECT_PRESSED: u32 = 0x00D070FF;
const COLOR_LABEL: u32 = 0x00E8E8E8;
const COLOR_BACKGROUND: u32 = 0xA01A1A1A;

pub struct GamepadOverlay;

impl GamepadOverlay {
    /// Draw the effective logical button state into a native-resolution XRGB frame.
    pub fn draw(buffer: &mut [u32], width: u32, height: u32, buttons: u32) {
        let Some(expected_len) = (width as usize).checked_mul(height as usize) else {
            return;
        };
        if width == 0 || height == 0 || buffer.len() < expected_len {
            return;
        }

        let unit = (width.min(height) / 48).clamp(1, 6) as i32;
        let margin = 2 * unit;
        let width = width as i32;
        let height = height as i32;

        let dpad_x = margin + 4 * unit;
        let dpad_y = height - margin - 4 * unit;
        fill_rect_alpha(
            buffer,
            width,
            height,
            dpad_x - 4 * unit,
            dpad_y - 4 * unit,
            9 * unit,
            9 * unit,
            COLOR_BACKGROUND,
        );
        draw_dpad_button(
            buffer,
            width,
            height,
            dpad_x - unit,
            dpad_y - 4 * unit,
            unit,
            "U",
            is_pressed(buttons, Button::Up),
        );
        draw_dpad_button(
            buffer,
            width,
            height,
            dpad_x - unit,
            dpad_y + 2 * unit,
            unit,
            "D",
            is_pressed(buttons, Button::Down),
        );
        draw_dpad_button(
            buffer,
            width,
            height,
            dpad_x - 4 * unit,
            dpad_y - unit,
            unit,
            "L",
            is_pressed(buttons, Button::Left),
        );
        draw_dpad_button(
            buffer,
            width,
            height,
            dpad_x + 2 * unit,
            dpad_y - unit,
            unit,
            "R",
            is_pressed(buttons, Button::Right),
        );
        fill_rect(
            buffer,
            width,
            height,
            dpad_x - unit,
            dpad_y - unit,
            3 * unit,
            3 * unit,
            COLOR_IDLE,
        );

        let o_x = width - margin - 3 * unit;
        let o_y = height - margin - 5 * unit;
        let x_x = width - margin - 8 * unit;
        let x_y = height - margin - 3 * unit;
        fill_rect_alpha(
            buffer,
            width,
            height,
            x_x - 3 * unit,
            o_y - 3 * unit,
            12 * unit,
            9 * unit,
            COLOR_BACKGROUND,
        );
        draw_action_button(
            buffer,
            width,
            height,
            o_x,
            o_y,
            2 * unit,
            "O",
            if is_pressed(buttons, Button::O) {
                COLOR_O_PRESSED
            } else {
                COLOR_IDLE
            },
        );
        draw_action_button(
            buffer,
            width,
            height,
            x_x,
            x_y,
            2 * unit,
            "X",
            if is_pressed(buttons, Button::X) {
                COLOR_X_PRESSED
            } else {
                COLOR_IDLE
            },
        );

        let system_y = height - margin - 2 * unit;
        let start_x = width / 2 - 8 * unit;
        let select_x = width / 2 + unit;
        draw_system_button(
            buffer,
            width,
            height,
            start_x,
            system_y,
            7 * unit,
            unit * 2,
            "START",
            if is_pressed(buttons, Button::Start) {
                COLOR_START_PRESSED
            } else {
                COLOR_IDLE
            },
        );
        draw_system_button(
            buffer,
            width,
            height,
            select_x,
            system_y,
            8 * unit,
            unit * 2,
            "SELECT",
            if is_pressed(buttons, Button::Select) {
                COLOR_SELECT_PRESSED
            } else {
                COLOR_IDLE
            },
        );
    }
}

fn is_pressed(buttons: u32, button: Button) -> bool {
    buttons & button.mask() != 0
}

#[allow(clippy::too_many_arguments)]
fn draw_dpad_button(
    buffer: &mut [u32],
    width: i32,
    height: i32,
    x: i32,
    y: i32,
    unit: i32,
    label: &str,
    pressed: bool,
) {
    fill_rect(
        buffer,
        width,
        height,
        x,
        y,
        3 * unit,
        3 * unit,
        if pressed {
            COLOR_DPAD_PRESSED
        } else {
            COLOR_IDLE
        },
    );
    draw_text_centered(
        buffer,
        width,
        height,
        x,
        y,
        3 * unit,
        3 * unit,
        label,
        COLOR_LABEL,
    );
}

#[allow(clippy::too_many_arguments)]
fn draw_action_button(
    buffer: &mut [u32],
    width: i32,
    height: i32,
    center_x: i32,
    center_y: i32,
    radius: i32,
    label: &str,
    color: u32,
) {
    fill_circle(buffer, width, height, center_x, center_y, radius, color);
    draw_text_centered(
        buffer,
        width,
        height,
        center_x - radius,
        center_y - radius,
        radius * 2 + 1,
        radius * 2 + 1,
        label,
        COLOR_LABEL,
    );
}

#[allow(clippy::too_many_arguments)]
fn draw_system_button(
    buffer: &mut [u32],
    width: i32,
    height: i32,
    x: i32,
    y: i32,
    button_width: i32,
    button_height: i32,
    label: &str,
    color: u32,
) {
    fill_rect_alpha(
        buffer,
        width,
        height,
        x - 2,
        y - 2,
        button_width + 4,
        button_height + 4,
        COLOR_BACKGROUND,
    );
    fill_rect(
        buffer,
        width,
        height,
        x,
        y,
        button_width,
        button_height,
        color,
    );
    draw_text_centered(
        buffer,
        width,
        height,
        x,
        y,
        button_width,
        button_height,
        label,
        COLOR_LABEL,
    );
}

#[allow(clippy::too_many_arguments)]
fn fill_rect(
    buffer: &mut [u32],
    width: i32,
    height: i32,
    x: i32,
    y: i32,
    rect_width: i32,
    rect_height: i32,
    color: u32,
) {
    for pixel_y in y.max(0)..(y + rect_height).min(height) {
        for pixel_x in x.max(0)..(x + rect_width).min(width) {
            buffer[pixel_y as usize * width as usize + pixel_x as usize] = color;
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn fill_rect_alpha(
    buffer: &mut [u32],
    width: i32,
    height: i32,
    x: i32,
    y: i32,
    rect_width: i32,
    rect_height: i32,
    color: u32,
) {
    let alpha = (color >> 24) & 0xFF;
    let inverse_alpha = 255 - alpha;
    let source_r = (color >> 16) & 0xFF;
    let source_g = (color >> 8) & 0xFF;
    let source_b = color & 0xFF;
    for pixel_y in y.max(0)..(y + rect_height).min(height) {
        for pixel_x in x.max(0)..(x + rect_width).min(width) {
            let index = pixel_y as usize * width as usize + pixel_x as usize;
            let destination = buffer[index];
            let destination_r = (destination >> 16) & 0xFF;
            let destination_g = (destination >> 8) & 0xFF;
            let destination_b = destination & 0xFF;
            let r = (source_r * alpha + destination_r * inverse_alpha) / 255;
            let g = (source_g * alpha + destination_g * inverse_alpha) / 255;
            let b = (source_b * alpha + destination_b * inverse_alpha) / 255;
            buffer[index] = (r << 16) | (g << 8) | b;
        }
    }
}

fn fill_circle(
    buffer: &mut [u32],
    width: i32,
    height: i32,
    center_x: i32,
    center_y: i32,
    radius: i32,
    color: u32,
) {
    let radius_squared = radius * radius;
    for offset_y in -radius..=radius {
        let pixel_y = center_y + offset_y;
        if pixel_y < 0 || pixel_y >= height {
            continue;
        }
        for offset_x in -radius..=radius {
            let pixel_x = center_x + offset_x;
            if pixel_x >= 0
                && pixel_x < width
                && offset_x * offset_x + offset_y * offset_y <= radius_squared
            {
                buffer[pixel_y as usize * width as usize + pixel_x as usize] = color;
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_text_centered(
    buffer: &mut [u32],
    width: i32,
    height: i32,
    x: i32,
    y: i32,
    cell_width: i32,
    cell_height: i32,
    text: &str,
    color: u32,
) {
    let scale = (cell_height / 7).max(1);
    let text_width = text.chars().count() as i32 * 4 * scale - scale;
    let origin_x = x + (cell_width - text_width) / 2;
    let origin_y = y + (cell_height - 5 * scale) / 2;
    for (index, character) in text.chars().enumerate() {
        let Some(glyph) = glyph(character) else {
            continue;
        };
        for (row, bits) in glyph.iter().enumerate() {
            for column in 0..3 {
                if bits & (1 << (2 - column)) != 0 {
                    fill_rect(
                        buffer,
                        width,
                        height,
                        origin_x + (index as i32 * 4 + column) * scale,
                        origin_y + row as i32 * scale,
                        scale,
                        scale,
                        color,
                    );
                }
            }
        }
    }
}

fn glyph(character: char) -> Option<[u8; 5]> {
    match character {
        'A' => Some([0b010, 0b101, 0b111, 0b101, 0b101]),
        'C' => Some([0b011, 0b100, 0b100, 0b100, 0b011]),
        'D' => Some([0b110, 0b101, 0b101, 0b101, 0b110]),
        'E' => Some([0b111, 0b100, 0b110, 0b100, 0b111]),
        'L' => Some([0b100, 0b100, 0b100, 0b100, 0b111]),
        'O' => Some([0b010, 0b101, 0b101, 0b101, 0b010]),
        'R' => Some([0b110, 0b101, 0b110, 0b101, 0b101]),
        'S' => Some([0b011, 0b100, 0b010, 0b001, 0b110]),
        'T' => Some([0b111, 0b010, 0b010, 0b010, 0b010]),
        'U' => Some([0b101, 0b101, 0b101, 0b101, 0b111]),
        'X' => Some([0b101, 0b101, 0b010, 0b101, 0b101]),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_pressed_button_has_a_distinct_highlight() {
        let mut frame = vec![0x00101010; 320 * 240];
        let buttons = Button::ALL
            .iter()
            .fold(0, |buttons, button| buttons | button.mask());
        GamepadOverlay::draw(&mut frame, 320, 240, buttons);

        for color in [
            COLOR_DPAD_PRESSED,
            COLOR_O_PRESSED,
            COLOR_X_PRESSED,
            COLOR_START_PRESSED,
            COLOR_SELECT_PRESSED,
        ] {
            assert!(frame.contains(&color), "missing highlight {color:08X}");
        }
    }

    #[test]
    fn idle_overlay_does_not_use_pressed_colors() {
        let mut frame = vec![0x00101010; 320 * 240];
        GamepadOverlay::draw(&mut frame, 320, 240, 0);
        assert!(frame.contains(&COLOR_IDLE));
        assert!(!frame.contains(&COLOR_O_PRESSED));
        assert!(!frame.contains(&COLOR_X_PRESSED));
    }

    #[test]
    fn varied_and_tiny_frame_sizes_are_clipped_safely() {
        for (width, height) in [(1, 1), (13, 9), (160, 120), (320, 240), (640, 480)] {
            let mut frame = vec![0; width * height];
            GamepadOverlay::draw(
                &mut frame,
                width as u32,
                height as u32,
                Button::O.mask() | Button::Select.mask(),
            );
            assert_eq!(frame.len(), width * height);
        }
    }

    #[test]
    fn short_or_zero_sized_buffers_are_ignored() {
        let mut short = vec![0x00123456; 3];
        GamepadOverlay::draw(&mut short, 2, 2, Button::O.mask());
        assert_eq!(short, [0x00123456; 3]);

        let mut empty = Vec::new();
        GamepadOverlay::draw(&mut empty, 0, 0, Button::O.mask());
        assert!(empty.is_empty());
    }
}
