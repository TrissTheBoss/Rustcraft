use std::{
    env, fs,
    io::{self, BufRead, BufReader, Write},
    net::TcpListener,
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
    let mut listen = false;
    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--config" {
            if let Some(value) = args.next() {
                config = PathBuf::from(value);
            }
        } else if arg == "--listen" {
            listen = true;
        }
    }
    let (executable, game_args) = match read_command(&config) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("Rustcraft: cannot read {}: {error}", config.display());
            return ExitCode::from(2);
        }
    };
    if listen {
        return listen_for_trigger(&executable, &game_args);
    }
    println!("Starting Minecraft command: {executable}");
    match Command::new(&executable).args(game_args).status() {
        Ok(status) => ExitCode::from(status.code().unwrap_or(1) as u8),
        Err(error) => {
            eprintln!("Rustcraft: failed to start {executable}: {error}");
            ExitCode::from(3)
        }
    }
}

fn listen_for_trigger(executable: &str, game_args: &[String]) -> ExitCode {
    let listener = match TcpListener::bind(("127.0.0.1", 39281)) {
        Ok(listener) => listener,
        Err(error) => {
            eprintln!("Rustcraft: cannot listen on 127.0.0.1:39281: {error}");
            return ExitCode::from(4);
        }
    };
    let token = env::var("RUSTCRAFT_TRIGGER_TOKEN").unwrap_or_else(|_| "local-only".to_owned());
    println!("Rustcraft trigger listening on 127.0.0.1:39281");
    for stream in listener.incoming() {
        let mut stream = match stream {
            Ok(stream) => stream,
            Err(error) => {
                eprintln!("Rustcraft: trigger connection failed: {error}");
                continue;
            }
        };
        let mut line = String::new();
        if BufReader::new(&mut stream).read_line(&mut line).is_ok()
            && line.trim() == format!("RUSTCRAFT {token}")
        {
            let result = Command::new(executable).args(game_args).spawn();
            let response = if result.is_ok() { "OK\n" } else { "ERROR\n" };
            let _ = stream.write_all(response.as_bytes());
        } else {
            let _ = stream.write_all(b"DENIED\n");
        }
    }
    ExitCode::SUCCESS
}
