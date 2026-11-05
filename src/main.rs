use std::{
    env, fs, io,
    path::PathBuf,
    process::{Command, ExitCode},
};

fn config_path() -> PathBuf {
    env::var_os("RUSTCRAFT_CONFIG")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            env::current_dir()
                .unwrap_or_else(|_| PathBuf::from("."))
                .join("rustcraft.conf")
        })
}

fn read_command(path: &PathBuf) -> io::Result<(String, Vec<String>)> {
    let text = fs::read_to_string(path)?;
    let mut executable = None;
    let mut args = Vec::new();
    for line in text.lines().map(str::trim) {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(value) = line.strip_prefix("executable=") {
            executable = Some(value.trim().trim_matches('"').to_owned());
        } else if let Some(value) = line.strip_prefix("args=") {
            args = split_args(value.trim())?;
        } else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("unknown setting: {line}"),
            ));
        }
    }
    executable
        .map(|exe| (exe, args))
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "missing executable= in config"))
}

fn split_args(value: &str) -> io::Result<Vec<String>> {
    let mut result = Vec::new();
    let mut current = String::new();
    let mut quoted = false;
    for character in value.chars() {
        match character {
            '"' => quoted = !quoted,
            character if character.is_whitespace() && !quoted => {
                if !current.is_empty() {
                    result.push(std::mem::take(&mut current));
                }
            }
            character => current.push(character),
        }
    }
    if quoted {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "unterminated quote in args",
        ));
    }
    if !current.is_empty() {
        result.push(current);
    }
    Ok(result)
}

fn main() -> ExitCode {
    let path = config_path();
    if env::args().any(|a| a == "--help" || a == "-h") {
        println!("Rustcraft {}\n\nStarts a user-configured Minecraft Java command immediately.\nConfig: {}\n\nOptions: --config <path>, --help", env!("CARGO_PKG_VERSION"), path.display());
        return ExitCode::SUCCESS;
    }
    let mut config = path.clone();
    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--config" {
            if let Some(value) = args.next() {
                config = PathBuf::from(value);
            }
        }
    }
    let (executable, game_args) = match read_command(&config) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("Rustcraft: cannot read {}: {error}", config.display());
            return ExitCode::from(2);
        }
    };
    println!("Starting Minecraft command: {executable}");
    match Command::new(&executable).args(game_args).status() {
        Ok(status) => ExitCode::from(status.code().unwrap_or(1) as u8),
        Err(error) => {
            eprintln!("Rustcraft: failed to start {executable}: {error}");
            ExitCode::from(3)
        }
    }
}
