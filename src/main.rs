// EH3D-PHOENIX-3DP - command line and window.
// Copyright (C) 2026 JuanenRac (Electro Hobby 3D). GPL-3.0-or-later.
#![forbid(unsafe_code)]

use eh3d_phoenix_3dp::cli;
use std::io::Write;
use std::process::ExitCode;

fn env_lang() -> Option<String> {
    std::env::var("PHOENIX_3DP_LANG")
        .or_else(|_| std::env::var("LANG"))
        .ok()
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let lang = env_lang();
    if args.first().map(String::as_str) == Some("gui") {
        return window(&args[1..], lang.as_deref());
    }
    let outcome = cli::run(&args, lang.as_deref());
    let _ = std::io::stdout().write_all(outcome.out.as_bytes());
    let _ = std::io::stderr().write_all(outcome.err.as_bytes());
    ExitCode::from(u8::try_from(outcome.code).unwrap_or(1))
}

#[cfg(feature = "gui")]
fn window(args: &[String], lang: Option<&str>) -> ExitCode {
    use eh3d_phoenix_3dp::config::AppConfig;
    use eh3d_phoenix_3dp::i18n::Lang;
    let config = match args.first() {
        Some(path) => match std::fs::read_to_string(path)
            .map_err(|e| e.to_string())
            .and_then(|t| AppConfig::from_text(&t))
        {
            Ok(c) => c,
            Err(e) => {
                eprintln!("{path}: {e}");
                return ExitCode::from(1);
            }
        },
        None => AppConfig::default(),
    };
    let language = config.lang.unwrap_or_else(|| Lang::detect(None, lang));
    match eh3d_phoenix_3dp::gui::run(config, language) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("the window could not start: {e}");
            ExitCode::from(1)
        }
    }
}

#[cfg(not(feature = "gui"))]
fn window(_args: &[String], _lang: Option<&str>) -> ExitCode {
    eprintln!("this build has no window: build it with `cargo build --release --features gui`");
    ExitCode::from(2)
}
