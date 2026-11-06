use std::{
    env, fs,
    fs::{File, OpenOptions},
    io::{self, BufRead, BufReader, Write},
    net::TcpListener,
    path::{Path, PathBuf},
    process::{Command, ExitCode},
    time::{SystemTime, UNIX_EPOCH},
};

const MAX_LOG_BYTES: u64 = 5 * 1024 * 1024;
struct Logger {
    file: Option<File>,
    session: String,
}
impl Logger {
    fn new() -> Self {
        let session = format!("{}-{}", std::process::id(), unix_millis());
        let path = env::var_os("RUSTCRAFT_LOG")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                env::current_dir()
                    .unwrap_or_else(|_| PathBuf::from("."))
                    .join("rustcraft.log")
            });
        let file = Self::open_rotated(&path).ok();
        let mut logger = Self { file, session };
        logger.info(
            "logger_started",
            &format!(
                "version={} path={}",
                env!("CARGO_PKG_VERSION"),
                path.display()
            ),
        );
        logger
    }
    fn open_rotated(path: &Path) -> io::Result<File> {
        if path.exists() && fs::metadata(path)?.len() >= MAX_LOG_BYTES {
            let rotated = path.with_extension("log.1");
            let _ = fs::remove_file(&rotated);
            fs::rename(path, rotated)?;
        }
        OpenOptions::new().create(true).append(true).open(path)
    }
    fn write(&mut self, level: &str, event: &str, details: &str) {
        let line = format!(
            "{} [{}] session={} event={} {}",
            unix_millis(),
            level,
            self.session,
            event,
            details
        );
        eprintln!("{line}");
        if let Some(file) = &mut self.file {
            if writeln!(file, "{line}").is_err() {
                self.file = None;
            }
        }
    }
    fn info(&mut self, event: &str, details: &str) {
        self.write("INFO", event, details);
    }
    fn warn(&mut self, event: &str, details: &str) {
        self.write("WARN", event, details);
    }
    fn error(&mut self, event: &str, details: &str) {
        self.write("ERROR", event, details);
    }
}
fn unix_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}
fn config_path() -> PathBuf {
    env::var_os("RUSTCRAFT_CONFIG")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            env::current_dir()
                .unwrap_or_else(|_| PathBuf::from("."))
                .join("rustcraft.conf")
        })
}
fn read_command(path: &Path) -> io::Result<(String, Vec<String>)> {
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
    let mut logger = Logger::new();
    let default_path = config_path();
    if env::args().any(|a| a == "--help" || a == "-h") {
        println!("Rustcraft {}\n\nStarts a user-configured Minecraft Java command immediately.\nConfig: {}\nLog: RUSTCRAFT_LOG or rustcraft.log\n\nOptions: --config <path>, --listen, --help", env!("CARGO_PKG_VERSION"), default_path.display());
        return ExitCode::SUCCESS;
    }
    let mut config = default_path;
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
            logger.error(
                "config_read_failed",
                &format!("path={} error={error}", config.display()),
            );
            return ExitCode::from(2);
        }
    };
    logger.info(
        "config_loaded",
        &format!("path={} executable={}", config.display(), executable),
    );
    if listen {
        return listen_for_trigger(&mut logger, &executable, &game_args);
    }
    logger.info("launch_requested", &format!("executable={executable}"));
    match Command::new(&executable).args(game_args).status() {
        Ok(status) => {
            logger.info("process_exited", &format!("status={status}"));
            ExitCode::from(status.code().unwrap_or(1) as u8)
        }
        Err(error) => {
            logger.error(
                "launch_failed",
                &format!("executable={executable} error={error}"),
            );
            ExitCode::from(3)
        }
    }
}
fn listen_for_trigger(logger: &mut Logger, executable: &str, game_args: &[String]) -> ExitCode {
    let listener = match TcpListener::bind(("127.0.0.1", 39281)) {
        Ok(listener) => listener,
        Err(error) => {
            logger.error(
                "listener_bind_failed",
                &format!("address=127.0.0.1:39281 error={error}"),
            );
            return ExitCode::from(4);
        }
    };
    let token = env::var("RUSTCRAFT_TRIGGER_TOKEN").unwrap_or_else(|_| "local-only".to_owned());
    let expected_request = format!("RUSTCRAFT {token}");
    logger.info("listener_started", "address=127.0.0.1:39281");
    for stream in listener.incoming() {
        let mut stream = match stream {
            Ok(stream) => stream,
            Err(error) => {
                logger.warn("connection_failed", &format!("error={error}"));
                continue;
            }
        };
        let _ = stream.set_nodelay(true);
        let _ = stream.set_read_timeout(Some(std::time::Duration::from_millis(500)));
        let _ = stream.set_write_timeout(Some(std::time::Duration::from_millis(500)));
        let mut line = String::new();
        if BufReader::new(&mut stream).read_line(&mut line).is_ok()
            && is_authorized_request(&line, &expected_request)
        {
            logger.info("trigger_authorized", "source=loopback");
            let result = Command::new(executable).args(game_args).spawn();
            let response = if result.is_ok() { "OK\n" } else { "ERROR\n" };
            logger.info("trigger_launch", &format!("result={}", response.trim()));
            let _ = stream.write_all(response.as_bytes());
        } else {
            logger.warn("trigger_denied", "reason=invalid_request_or_token");
            let _ = stream.write_all(b"DENIED\n");
        }
    }
    ExitCode::SUCCESS
}

fn is_authorized_request(line: &str, expected_request: &str) -> bool {
    line.trim() == expected_request
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_args_preserves_quoted_instance_id() {
        assert_eq!(
            split_args(r#"--launch "Rustcraft testing" --profile test"#).unwrap(),
            vec!["--launch", "Rustcraft testing", "--profile", "test"]
        );
    }

    #[test]
    fn split_args_rejects_unterminated_quotes() {
        assert!(split_args(r#"--launch "Rustcraft testing"#).is_err());
    }

    #[test]
    fn read_command_rejects_unknown_settings() {
        let path = std::env::temp_dir().join(format!("rustcraft-test-{}", std::process::id()));
        fs::write(&path, "executable=java\nunknown=value\n").unwrap();
        let result = read_command(&path);
        let _ = fs::remove_file(&path);
        assert!(result.is_err());
    }

    #[test]
    fn protocol_accepts_only_exact_trimmed_request() {
        assert!(is_authorized_request(
            "RUSTCRAFT local-only\n",
            "RUSTCRAFT local-only"
        ));
        assert!(!is_authorized_request(
            "RUSTCRAFT local-only-extra",
            "RUSTCRAFT local-only"
        ));
        assert!(!is_authorized_request("RUSTCRAFT", "RUSTCRAFT local-only"));
    }
}
