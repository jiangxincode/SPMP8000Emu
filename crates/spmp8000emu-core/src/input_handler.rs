// Platform-independent SPMP8000 logical button state.

use serde::{Deserialize, Serialize};

/// Logical buttons shared by standalone and libretro frontends.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Button {
    Up = 0,
    Down = 1,
    Left = 2,
    Right = 3,
    O = 4,
    X = 5,
    Select = 10,
    Start = 11,
}

impl Button {
    pub const ALL: [Self; 8] = [
        Self::Up,
        Self::Down,
        Self::Left,
        Self::Right,
        Self::O,
        Self::X,
        Self::Start,
        Self::Select,
    ];

    pub const fn index(self) -> usize {
        self as usize
    }

    pub const fn mask(self) -> u32 {
        1 << self.index()
    }
}

pub const BUTTON_UP: usize = Button::Up.index();
pub const BUTTON_DOWN: usize = Button::Down.index();
pub const BUTTON_LEFT: usize = Button::Left.index();
pub const BUTTON_RIGHT: usize = Button::Right.index();
pub const BUTTON_O: usize = Button::O.index();
pub const BUTTON_X: usize = Button::X.index();
pub const BUTTON_START: usize = Button::Start.index();
pub const BUTTON_SELECT: usize = Button::Select.index();

pub const SUPPORTED_BUTTON_MASK: u32 = Button::Up.mask()
    | Button::Down.mask()
    | Button::Left.mask()
    | Button::Right.mask()
    | Button::O.mask()
    | Button::X.mask()
    | Button::Start.mask()
    | Button::Select.mask();

/// Current logical button state.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct InputHandler {
    buttons: u32,
}

impl InputHandler {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_buttons(&mut self, buttons: u32) {
        self.buttons = buttons & SUPPORTED_BUTTON_MASK;
    }

    pub fn get_buttons(&self) -> u32 {
        self.buttons
    }

    pub fn press_button(&mut self, button: usize) {
        if button < 32 {
            self.buttons |= 1 << button;
            self.buttons &= SUPPORTED_BUTTON_MASK;
        }
    }

    pub fn release_button(&mut self, button: usize) {
        if button < 32 {
            self.buttons &= !(1 << button);
        }
    }

    pub fn is_button_pressed(&self, button: usize) -> bool {
        button < 32 && self.buttons & (1 << button) != 0
    }

    pub fn clear(&mut self) {
        self.buttons = 0;
    }

    pub(crate) fn validate_state(&self) -> anyhow::Result<()> {
        if self.buttons & !SUPPORTED_BUTTON_MASK != 0 {
            anyhow::bail!("save state contains unsupported input button bits");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn button_masks_cover_all_supported_buttons() {
        let combined = Button::ALL
            .iter()
            .fold(0, |buttons, button| buttons | button.mask());
        assert_eq!(combined, SUPPORTED_BUTTON_MASK);
        assert_eq!(Button::ALL.len(), 8);
    }

    #[test]
    fn button_state_supports_press_release_and_clear() {
        let mut handler = InputHandler::new();
        handler.press_button(BUTTON_UP);
        handler.press_button(BUTTON_O);
        assert!(handler.is_button_pressed(BUTTON_UP));
        assert!(handler.is_button_pressed(BUTTON_O));
        assert!(!handler.is_button_pressed(BUTTON_DOWN));

        handler.release_button(BUTTON_UP);
        assert!(!handler.is_button_pressed(BUTTON_UP));
        assert!(handler.is_button_pressed(BUTTON_O));

        handler.clear();
        assert_eq!(handler.get_buttons(), 0);
    }

    #[test]
    fn unsupported_button_bits_are_discarded() {
        let mut handler = InputHandler::new();
        handler.set_buttons(SUPPORTED_BUTTON_MASK | (1 << 31));
        assert_eq!(handler.get_buttons(), SUPPORTED_BUTTON_MASK);
        handler.press_button(31);
        assert_eq!(handler.get_buttons(), SUPPORTED_BUTTON_MASK);
    }
}
