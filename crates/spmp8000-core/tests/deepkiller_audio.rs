use std::path::{Path, PathBuf};

use spmp8000_core::emulator::Emulator;

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
