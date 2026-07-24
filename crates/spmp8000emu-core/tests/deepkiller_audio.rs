use std::path::{Path, PathBuf};

use spmp8000emu_core::emulator::Emulator;
use spmp8000emu_core::memory::{RAM_BASE, VRAM_BASE};

const GAME_NAMES: [&str; 2] = [
    "DeepKiller-1.2.6_P_new.bin",
    "DeepKiller-1.2.6_SPCA8000_320x240_EN_P_new.bin",
];

fn game_directory() -> Option<PathBuf> {
    if let Ok(directory) = std::env::var("SPMP8000_GAME_DIR") {
        let path = PathBuf::from(directory);
        return path.is_dir().then_some(path);
    }

    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map(|root| root.join("tmp").join("GameCollection"))
        .filter(|path| path.is_dir())
}

#[test]
#[ignore = "requires local DeepKiller game assets (set SPMP8000_GAME_DIR)"]
fn deepkiller_versions_generate_music_during_startup() {
    let Some(directory) = game_directory() else {
        return;
    };

    let mut tested_versions = 0;
    for game_name in GAME_NAMES {
        let path = directory.join(game_name);
        if !path.is_file() {
            continue;
        }
        tested_versions += 1;

        let mut emulator = Emulator::from_path(path, 100).unwrap();
        emulator.start();
        let mut heard_audio = false;
        for _ in 0..60 {
            emulator.tick();
            heard_audio |= emulator
                .get_audio_samples()
                .iter()
                .any(|sample| *sample != 0);
        }

        assert!(heard_audio, "{game_name} did not generate startup audio");
    }
    assert!(tested_versions > 0, "no DeepKiller game assets were found");
}

#[test]
#[ignore = "requires local DeepKiller game assets (set SPMP8000_GAME_DIR)"]
fn deepkiller_save_state_replays_the_same_next_frame() {
    let Some(directory) = game_directory() else {
        return;
    };
    let path = directory.join(GAME_NAMES[0]);
    if !path.is_file() {
        return;
    }

    let mut emulator = Emulator::from_path(path, 100).unwrap();
    emulator.start();
    for _ in 0..5 {
        emulator.tick();
    }

    let saved_pc = emulator.cpu.regs.pc;
    let saved_instruction_count = emulator.cpu.instruction_count;
    let saved_ram = emulator.memory.read_u32(RAM_BASE + 0x1000).unwrap();
    let saved_vram = emulator.memory.read_u32(VRAM_BASE).unwrap();
    let saved_tick = emulator.get_tick_count();
    let mut state = vec![0u8; emulator.serialize_size()];
    emulator.serialize(&mut state).unwrap();

    emulator.tick();
    let expected_pc = emulator.cpu.regs.pc;
    let expected_instruction_count = emulator.cpu.instruction_count;
    let expected_framebuffer = emulator.get_framebuffer().to_vec();
    let expected_audio = emulator.get_audio_samples().to_vec();

    emulator.deserialize(&state).unwrap();
    assert_eq!(emulator.cpu.regs.pc, saved_pc);
    assert_eq!(emulator.cpu.instruction_count, saved_instruction_count);
    assert_eq!(
        emulator.memory.read_u32(RAM_BASE + 0x1000).unwrap(),
        saved_ram
    );
    assert_eq!(emulator.memory.read_u32(VRAM_BASE).unwrap(), saved_vram);
    assert_eq!(emulator.get_tick_count(), saved_tick);
    assert!(emulator.is_running());

    emulator.tick();
    assert_eq!(emulator.cpu.regs.pc, expected_pc);
    assert_eq!(emulator.cpu.instruction_count, expected_instruction_count);
    assert_eq!(emulator.get_framebuffer(), expected_framebuffer);
    assert_eq!(emulator.get_audio_samples(), expected_audio);
}
