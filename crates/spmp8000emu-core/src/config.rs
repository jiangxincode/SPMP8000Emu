use crate::input_handler::Button;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnknownInstructionPolicy {
    Stop,
    Skip,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoreConfig {
    pub volume: u32,
    pub swap_o_x: bool,
    pub debug_logging: bool,
    pub unknown_instruction_policy: UnknownInstructionPolicy,
}

impl CoreConfig {
    pub fn normalized(mut self) -> Self {
        self.volume = self.volume.min(100);
        self
    }

    pub fn map_buttons(&self, buttons: u32) -> u32 {
        if !self.swap_o_x {
            return buttons;
        }

        let o_pressed = buttons & Button::O.mask() != 0;
        let x_pressed = buttons & Button::X.mask() != 0;
        let mut mapped = buttons & !(Button::O.mask() | Button::X.mask());
        if o_pressed {
            mapped |= Button::X.mask();
        }
        if x_pressed {
            mapped |= Button::O.mask();
        }
        mapped
    }
}

impl Default for CoreConfig {
    fn default() -> Self {
        Self {
            volume: 100,
            swap_o_x: false,
            debug_logging: false,
            unknown_instruction_policy: UnknownInstructionPolicy::Stop,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_frontend_defaults() {
        assert_eq!(CoreConfig::default().volume, 100);
        assert!(!CoreConfig::default().swap_o_x);
        assert!(!CoreConfig::default().debug_logging);
        assert_eq!(
            CoreConfig::default().unknown_instruction_policy,
            UnknownInstructionPolicy::Stop
        );
    }

    #[test]
    fn button_swap_only_exchanges_o_and_x() {
        let config = CoreConfig {
            swap_o_x: true,
            ..CoreConfig::default()
        };
        assert_eq!(
            config.map_buttons(Button::Up.mask() | Button::O.mask()),
            Button::Up.mask() | Button::X.mask()
        );
        assert_eq!(
            config.map_buttons(Button::O.mask() | Button::X.mask()),
            Button::O.mask() | Button::X.mask()
        );
    }
}
