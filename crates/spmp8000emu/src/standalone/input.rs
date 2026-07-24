// Standalone keyboard-to-logical-button mapping.

use std::str::FromStr;

use minifb::{Key, Window};
use spmp8000emu_core::input_handler::Button;

const DEFAULT_KEY_MAP: [(Button, Key); 8] = [
    (Button::Up, Key::Up),
    (Button::Down, Key::Down),
    (Button::Left, Key::Left),
    (Button::Right, Key::Right),
    (Button::O, Key::Z),
    (Button::X, Key::X),
    (Button::Start, Key::Enter),
    (Button::Select, Key::Backspace),
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RemapSpec {
    button: Button,
    key: Key,
}

impl FromStr for RemapSpec {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let (button, key) = value
            .split_once(':')
            .ok_or_else(|| "expected BUTTON:KEY, for example o:space".to_string())?;
        let button = parse_button(button.trim())?;
        let key = parse_key(key.trim())?;
        Ok(Self { button, key })
    }
}

pub struct KeyboardMapper {
    mappings: [(Button, Key); 8],
}

impl KeyboardMapper {
    pub fn new(remappings: &[RemapSpec]) -> Self {
        let mut mappings = DEFAULT_KEY_MAP;
        for remapping in remappings {
            if let Some((_, key)) = mappings
                .iter_mut()
                .find(|(button, _)| *button == remapping.button)
            {
                *key = remapping.key;
            }
        }
        Self { mappings }
    }

    pub fn pressed_buttons(&self, window: &Window) -> u32 {
        self.buttons_from_key_state(|key| window.is_key_down(key))
    }

    fn buttons_from_key_state(&self, mut is_down: impl FnMut(Key) -> bool) -> u32 {
        self.mappings
            .iter()
            .filter(|(_, key)| is_down(*key))
            .fold(0, |buttons, (button, _)| buttons | button.mask())
    }
}

fn parse_button(name: &str) -> Result<Button, String> {
    match name.to_ascii_lowercase().as_str() {
        "up" => Ok(Button::Up),
        "down" => Ok(Button::Down),
        "left" => Ok(Button::Left),
        "right" => Ok(Button::Right),
        "o" => Ok(Button::O),
        "x" => Ok(Button::X),
        "start" => Ok(Button::Start),
        "select" => Ok(Button::Select),
        _ => Err(format!(
            "unknown button '{name}'; expected up, down, left, right, o, x, start, or select"
        )),
    }
}

fn parse_key(name: &str) -> Result<Key, String> {
    let normalized = name.to_ascii_lowercase();
    let key = match normalized.as_str() {
        "a" => Key::A,
        "b" => Key::B,
        "c" => Key::C,
        "d" => Key::D,
        "e" => Key::E,
        "f" => Key::F,
        "g" => Key::G,
        "h" => Key::H,
        "i" => Key::I,
        "j" => Key::J,
        "k" => Key::K,
        "l" => Key::L,
        "m" => Key::M,
        "n" => Key::N,
        "o" => Key::O,
        "p" => Key::P,
        "q" => Key::Q,
        "r" => Key::R,
        "s" => Key::S,
        "t" => Key::T,
        "u" => Key::U,
        "v" => Key::V,
        "w" => Key::W,
        "x" => Key::X,
        "y" => Key::Y,
        "z" => Key::Z,
        "0" => Key::Key0,
        "1" => Key::Key1,
        "2" => Key::Key2,
        "3" => Key::Key3,
        "4" => Key::Key4,
        "5" => Key::Key5,
        "6" => Key::Key6,
        "7" => Key::Key7,
        "8" => Key::Key8,
        "9" => Key::Key9,
        "f1" => Key::F1,
        "f2" => Key::F2,
        "f3" => Key::F3,
        "f4" => Key::F4,
        "f5" => Key::F5,
        "f6" => Key::F6,
        "f7" => Key::F7,
        "f8" => Key::F8,
        "f9" => Key::F9,
        "f10" => Key::F10,
        "f11" => Key::F11,
        "f12" => Key::F12,
        "up" => Key::Up,
        "down" => Key::Down,
        "left" => Key::Left,
        "right" => Key::Right,
        "space" => Key::Space,
        "enter" | "return" => Key::Enter,
        "backspace" => Key::Backspace,
        "tab" => Key::Tab,
        "delete" => Key::Delete,
        "home" => Key::Home,
        "end" => Key::End,
        "pageup" => Key::PageUp,
        "pagedown" => Key::PageDown,
        "leftshift" => Key::LeftShift,
        "rightshift" => Key::RightShift,
        "leftctrl" => Key::LeftCtrl,
        "rightctrl" => Key::RightCtrl,
        "leftalt" => Key::LeftAlt,
        "rightalt" => Key::RightAlt,
        "escape" | "esc" => {
            return Err("escape is reserved for exiting the standalone emulator".to_string());
        }
        _ => return Err(format!("unknown key '{name}'")),
    };
    Ok(key)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn remap(value: &str) -> RemapSpec {
        value.parse().unwrap()
    }

    #[test]
    fn default_mapping_covers_all_eight_logical_buttons() {
        let mapper = KeyboardMapper::new(&[]);
        let buttons = mapper.buttons_from_key_state(|_| true);
        let expected = Button::ALL
            .iter()
            .fold(0, |buttons, button| buttons | button.mask());
        assert_eq!(buttons, expected);
    }

    #[test]
    fn remapping_replaces_the_default_key() {
        let mapper = KeyboardMapper::new(&[remap("o:space")]);
        assert_eq!(
            mapper.buttons_from_key_state(|key| key == Key::Space),
            Button::O.mask()
        );
        assert_eq!(mapper.buttons_from_key_state(|key| key == Key::Z), 0);
    }

    #[test]
    fn last_duplicate_remapping_wins() {
        let mapper = KeyboardMapper::new(&[remap("select:tab"), remap("select:rightshift")]);
        assert_eq!(mapper.buttons_from_key_state(|key| key == Key::Tab), 0);
        assert_eq!(
            mapper.buttons_from_key_state(|key| key == Key::RightShift),
            Button::Select.mask()
        );
    }

    #[test]
    fn parser_accepts_all_logical_button_names() {
        for button in ["up", "down", "left", "right", "o", "x", "start", "select"] {
            assert!(format!("{button}:space").parse::<RemapSpec>().is_ok());
        }
    }

    #[test]
    fn parser_rejects_invalid_specs_and_reserved_escape() {
        assert!("o".parse::<RemapSpec>().is_err());
        assert!("a:space".parse::<RemapSpec>().is_err());
        assert!("o:not-a-key".parse::<RemapSpec>().is_err());
        assert!("select:escape".parse::<RemapSpec>().is_err());
    }
}
