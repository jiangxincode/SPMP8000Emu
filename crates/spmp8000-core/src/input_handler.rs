// Input handler for SPMP8000 emulator
//
// Maps host input (keyboard/gamepad) to SPMP8000 button state

/// Button indices
pub const BUTTON_UP: usize = 0;
pub const BUTTON_DOWN: usize = 1;
pub const BUTTON_LEFT: usize = 2;
pub const BUTTON_RIGHT: usize = 3;
pub const BUTTON_O: usize = 4;      // A/Cross button
pub const BUTTON_X: usize = 5;      // B/Circle button
pub const BUTTON_START: usize = 11;
pub const BUTTON_SELECT: usize = 10;

/// Input handler state
#[derive(Debug)]
pub struct InputHandler {
    /// Current button state (bitmask)
    buttons: u32,
    /// Key mappings (button index -> host key code)
    key_map: Vec<Option<u32>>,
    /// Typematic delay (ms)
    repeat_delay: u32,
    /// Typematic period (ms)
    repeat_period: u32,
}

impl InputHandler {
    /// Create a new input handler
    pub fn new() -> Self {
        let mut handler = Self {
            buttons: 0,
            key_map: vec![None; 32],
            repeat_delay: 500,
            repeat_period: 100,
        };

        // Set default key mappings
        handler.set_default_mappings();
        handler
    }

    /// Set default keyboard mappings
    fn set_default_mappings(&mut self) {
        // Arrow keys
        self.key_map[BUTTON_UP] = Some(0x26);    // Up arrow
        self.key_map[BUTTON_DOWN] = Some(0x28);  // Down arrow
        self.key_map[BUTTON_LEFT] = Some(0x25);  // Left arrow
        self.key_map[BUTTON_RIGHT] = Some(0x27); // Right arrow

        // Action buttons
        self.key_map[BUTTON_O] = Some(0x5A);     // Z key
        self.key_map[BUTTON_X] = Some(0x58);     // X key

        // System buttons
        self.key_map[BUTTON_START] = Some(0x0D);  // Enter
        self.key_map[BUTTON_SELECT] = Some(0x1B); // Escape
    }

    /// Set button state directly
    pub fn set_buttons(&mut self, buttons: u32) {
        self.buttons = buttons;
    }

    /// Get current button state
    pub fn get_buttons(&self) -> u32 {
        self.buttons
    }

    /// Press a button
    pub fn press_button(&mut self, button: usize) {
        if button < 32 {
            self.buttons |= 1 << button;
        }
    }

    /// Release a button
    pub fn release_button(&mut self, button: usize) {
        if button < 32 {
            self.buttons &= !(1 << button);
        }
    }

    /// Check if a button is pressed
    pub fn is_button_pressed(&self, button: usize) -> bool {
        if button < 32 {
            (self.buttons & (1 << button)) != 0
        } else {
            false
        }
    }

    /// Process a key press event
    pub fn key_down(&mut self, key_code: u32) {
        let buttons_to_press: Vec<usize> = self.key_map
            .iter()
            .enumerate()
            .filter_map(|(button, mapping)| {
                if let Some(mapped_key) = mapping {
                    if *mapped_key == key_code {
                        Some(button)
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .collect();

        for button in buttons_to_press {
            self.press_button(button);
        }
    }

    /// Process a key release event
    pub fn key_up(&mut self, key_code: u32) {
        let buttons_to_release: Vec<usize> = self.key_map
            .iter()
            .enumerate()
            .filter_map(|(button, mapping)| {
                if let Some(mapped_key) = mapping {
                    if *mapped_key == key_code {
                        Some(button)
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .collect();

        for button in buttons_to_release {
            self.release_button(button);
        }
    }

    /// Set custom key mapping
    pub fn set_key_mapping(&mut self, button: usize, key_code: Option<u32>) {
        if button < self.key_map.len() {
            self.key_map[button] = key_code;
        }
    }

    /// Get key mapping for a button
    pub fn get_key_mapping(&self, button: usize) -> Option<u32> {
        if button < self.key_map.len() {
            self.key_map[button]
        } else {
            None
        }
    }

    /// Set typematic timing
    pub fn set_repeat_timing(&mut self, delay: u32, period: u32) {
        self.repeat_delay = delay;
        self.repeat_period = period;
    }

    /// Get typematic delay
    pub fn get_repeat_delay(&self) -> u32 {
        self.repeat_delay
    }

    /// Get typematic period
    pub fn get_repeat_period(&self) -> u32 {
        self.repeat_period
    }

    /// Clear all button states
    pub fn clear(&mut self) {
        self.buttons = 0;
    }
}

impl Default for InputHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_input_handler_creation() {
        let handler = InputHandler::new();
        assert_eq!(handler.get_buttons(), 0);
    }

    #[test]
    fn test_button_press_release() {
        let mut handler = InputHandler::new();

        handler.press_button(BUTTON_UP);
        assert!(handler.is_button_pressed(BUTTON_UP));
        assert!(!handler.is_button_pressed(BUTTON_DOWN));

        handler.press_button(BUTTON_O);
        assert!(handler.is_button_pressed(BUTTON_UP));
        assert!(handler.is_button_pressed(BUTTON_O));

        handler.release_button(BUTTON_UP);
        assert!(!handler.is_button_pressed(BUTTON_UP));
        assert!(handler.is_button_pressed(BUTTON_O));
    }

    #[test]
    fn test_key_mapping() {
        let mut handler = InputHandler::new();

        handler.key_down(0x26); // Up arrow
        assert!(handler.is_button_pressed(BUTTON_UP));

        handler.key_up(0x26);
        assert!(!handler.is_button_pressed(BUTTON_UP));

        handler.key_down(0x5A); // Z key
        assert!(handler.is_button_pressed(BUTTON_O));
    }

    #[test]
    fn test_custom_mapping() {
        let mut handler = InputHandler::new();

        // Remap O button to space bar
        handler.set_key_mapping(BUTTON_O, Some(0x20));

        handler.key_down(0x20);
        assert!(handler.is_button_pressed(BUTTON_O));

        // Original Z key should not work anymore
        handler.key_down(0x5A);
        assert!(handler.is_button_pressed(BUTTON_O)); // Still pressed from space
    }
}
