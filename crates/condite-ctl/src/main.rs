use anyhow::{Context, Result, bail};
use clap::{Args, Parser, Subcommand};
use condite_shared::control::{ControlCommand, ControlResponse, MAX_CONTROL_FRAME};
use std::{
    io::{self, Read, Write},
    os::unix::net::UnixStream,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

#[derive(Parser)]
#[command(version, about = "Administer Condite through its local Unix socket")]
struct Cli {
    /// TOML configuration file (optional for local Unix discovery)
    #[arg(long, global = true)]
    config: Option<PathBuf>,
    /// Explicit control socket (otherwise try XDG_RUNTIME_DIR, then /run)
    #[arg(long, global = true)]
    socket: Option<PathBuf>,
    /// Print the response as JSON
    #[arg(long, global = true)]
    json: bool,
    /// Control request timeout in seconds
    #[arg(long, global = true, value_parser = clap::value_parser!(u64).range(1..=300))]
    timeout: Option<u64>,
    #[command(subcommand)]
    command: Command,
}
#[derive(Subcommand)]
enum Command {
    Status,
    Shutdown,
    Reload,
    SetLogLevel {
        level: String,
    },
    Apikey {
        #[command(subcommand)]
        command: KeyCommand,
    },
    Upload {
        #[command(subcommand)]
        command: UploadCommand,
    },
}
#[derive(Subcommand)]
enum KeyCommand {
    /// List key identities and usage without exposing credentials
    List {
        #[arg(long)]
        limit: Option<u32>,
        #[arg(long)]
        after: Option<String>,
    },
    /// Inspect one key
    Inspect { id: String },
    /// Generate a key; save the secret, which is shown only once
    Generate {
        /// Whole quantity with suffix; MB and MiB both mean 1,048,576 bytes
        #[arg(long, default_value = "100MiB", value_parser = condite_ctl::parse_size)]
        max_per_upload_size: u64,
        /// Whole quantity with suffix; GB and GiB both mean 1,073,741,824 bytes
        #[arg(long, default_value = "1GiB", value_parser = condite_ctl::parse_size)]
        max_total_storage_size: u64,
    },
    /// Change limits, preserving unspecified values and existing files
    Update {
        id: String,
        #[command(flatten)]
        limits: Limits,
    },
    /// Revoke a key without deleting its files
    Delete { id: String },
}
#[derive(Args)]
#[group(required = true, multiple = true)]
struct Limits {
    #[arg(long, value_parser = condite_ctl::parse_size)]
    max_per_upload_size: Option<u64>,
    #[arg(long, value_parser = condite_ctl::parse_size)]
    max_total_storage_size: Option<u64>,
}
#[derive(Subcommand)]
enum UploadCommand {
    Delete { id: String },
}

fn connect(explicit: Option<&Path>, timeout: Duration) -> Result<UnixStream> {
    let mut paths = Vec::new();
    if let Some(path) = explicit {
        paths.push(path.to_owned());
    } else {
        if let Some(runtime) = std::env::var_os("XDG_RUNTIME_DIR").filter(|s| !s.is_empty()) {
            let runtime = PathBuf::from(runtime);
            if runtime.is_absolute() {
                paths.push(runtime.join("condite/condite.sock"));
            }
        }
        paths.push(PathBuf::from("/run/condite/condite.sock"));
    }
    for (index, path) in paths.iter().enumerate() {
        // Only an absent socket triggers discovery fallback.
        match connect_with_timeout(path, timeout) {
            Ok(stream) => return Ok(stream),
            Err(e) if e.kind() == io::ErrorKind::NotFound && index + 1 < paths.len() => continue,
            Err(e) => {
                return Err(e).with_context(|| format!("could not connect to {}", path.display()));
            }
        }
    }
    bail!("no control socket found")
}

fn connect_with_timeout(path: &Path, timeout: Duration) -> io::Result<UnixStream> {
    // socket2 implements bounded connect, including Unix-domain sockets.
    use socket2::{Domain, SockAddr, Socket, Type};
    let socket = Socket::new(Domain::UNIX, Type::STREAM, None)?;
    socket.connect_timeout(&SockAddr::unix(path)?, timeout)?;
    let fd: std::os::fd::OwnedFd = socket.into();
    Ok(UnixStream::from(fd))
}

fn run(cli: Cli) -> Result<()> {
    let command = match cli.command {
        Command::Status => ControlCommand::Status,
        Command::Shutdown => ControlCommand::Shutdown,
        Command::Reload => ControlCommand::Reload,
        Command::SetLogLevel { level } => ControlCommand::SetLogLevel { level },
        Command::Upload {
            command: UploadCommand::Delete { id },
        } => ControlCommand::DeleteUpload { id },
        Command::Apikey { command } => match command {
            KeyCommand::List { limit, after } => ControlCommand::ListApiKeys { limit, after },
            KeyCommand::Inspect { id } => ControlCommand::InspectApiKey { id },
            KeyCommand::Generate {
                max_per_upload_size,
                max_total_storage_size,
            } => ControlCommand::GenerateApiKey {
                per_upload_quota: max_per_upload_size,
                storage_quota: max_total_storage_size,
            },
            KeyCommand::Update { id, limits } => ControlCommand::UpdateApiKey {
                id,
                per_upload_quota: limits.max_per_upload_size,
                storage_quota: limits.max_total_storage_size,
            },
            KeyCommand::Delete { id } => ControlCommand::DeleteApiKey { id },
        },
    };
    let _ = dotenv::dotenv();
    let config_file = cli.config.as_deref().or_else(|| {
        Path::new("ctl.toml")
            .exists()
            .then_some(Path::new("ctl.toml"))
    });
    let mut config: CtlConfig = match config_file {
        Some(path) => condite_config::load(path)?,
        None => CtlConfig::default(),
    };
    if let (Some(path), Some(socket)) = (config_file, config.socket.as_mut())
        && socket.is_relative()
    {
        *socket = path.parent().unwrap_or(Path::new(".")).join(&*socket);
    }
    let seconds = cli.timeout.or(config.timeout_seconds).unwrap_or(10);
    anyhow::ensure!(
        (1..=300).contains(&seconds),
        "timeout_seconds must be between 1 and 300"
    );
    let timeout = Duration::from_secs(seconds);
    let response = if let Some(endpoint) = config.endpoint.as_ref().filter(|_| cli.socket.is_none())
    {
        condite_config::validate_http_url(endpoint)?;
        let url = reqwest::Url::parse(endpoint)?;
        if url.scheme() == "http"
            && !matches!(url.host_str(), Some("localhost" | "127.0.0.1" | "[::1]"))
            && !config.allow_insecure_http
        {
            bail!(
                "remote HTTP requires TLS or allow_insecure_http = true for a trusted Docker network"
            );
        }
        let key = config
            .api_key
            .as_deref()
            .context("HTTP transport requires api_key in TOML")?;
        let client = reqwest::blocking::Client::builder()
            .timeout(timeout)
            .redirect(reqwest::redirect::Policy::none())
            .build()?;
        let response = client
            .post(format!("{}/v1/control", endpoint.trim_end_matches('/')))
            .bearer_auth(key)
            .json(&command)
            .send()?
            .error_for_status()?;
        let mut bytes = Vec::new();
        response.take(1_048_577).read_to_end(&mut bytes)?;
        if bytes.len() > 1_048_576 {
            bail!("oversized control response");
        }
        serde_json::from_slice::<ControlResponse>(&bytes).context("invalid JSON response")?
    } else {
        unix_request(
            cli.socket.as_deref().or(config.socket.as_deref()),
            timeout,
            &command,
        )?
    };
    print_response(response, cli.json)
}

#[derive(Default, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct CtlConfig {
    timeout_seconds: Option<u64>,
    endpoint: Option<String>,
    api_key: Option<String>,
    socket: Option<PathBuf>,
    #[serde(default)]
    allow_insecure_http: bool,
}

fn unix_request(
    socket: Option<&Path>,
    timeout: Duration,
    command: &ControlCommand,
) -> Result<ControlResponse> {
    let mut stream = connect(socket, timeout)?;
    let mut bytes = serde_json::to_vec(command)?;
    bytes.push(b'\n');
    if bytes.len() as u64 > MAX_CONTROL_FRAME {
        bail!("control command exceeds the frame limit");
    }
    let deadline = Instant::now() + timeout;
    let remaining = || {
        deadline
            .checked_duration_since(Instant::now())
            .filter(|d| !d.is_zero())
            .context("control request timed out")
    };
    let mut written = 0;
    while written < bytes.len() {
        stream.set_write_timeout(Some(remaining()?))?;
        match stream.write(&bytes[written..]) {
            Ok(0) => bail!("socket closed while writing command"),
            Ok(n) => written += n,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(e).context("could not write control command"),
        }
    }
    let mut response = Vec::new();
    loop {
        stream.set_read_timeout(Some(remaining()?))?;
        let mut chunk = [0u8; 1024];
        let count = match stream.read(&mut chunk) {
            Ok(0) => bail!("socket closed before a complete response"),
            Ok(n) => n,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(e).context("could not read control response"),
        };
        let end = chunk[..count]
            .iter()
            .position(|b| *b == b'\n')
            .map(|i| i + 1);
        response.extend_from_slice(&chunk[..end.unwrap_or(count)]);
        if response.len() > 1_048_576 {
            bail!("oversized control response");
        }
        if end.is_some() {
            break;
        }
    }
    let response: ControlResponse =
        serde_json::from_slice(&response).context("invalid JSON response")?;
    Ok(response)
}

fn print_response(response: ControlResponse, json: bool) -> Result<()> {
    match &response {
        ControlResponse::Error { code, message } => bail!("{code}: {message}"),
        ControlResponse::CommandNotImplemented => {
            bail!("command is not implemented by this server")
        }
        _ => {}
    }
    if json {
        println!("{}", serde_json::to_string(&response)?);
    } else {
        match response {
            ControlResponse::ApiKey { id, api_key } => println!("Key ID: {id}\nAPI key: {api_key}"),
            ControlResponse::Status(status) => println!(
                "Healthy: {}\nStarted: {}\nRuntime: {}",
                status.healthy,
                status.started_at,
                condite_ctl::format_uptime(&status.runtime)
            ),
            ControlResponse::Keys(keys) => println!("{}", serde_json::to_string_pretty(&keys)?),
            ControlResponse::Key(key) => println!("{}", serde_json::to_string_pretty(&key)?),
            _ => println!("OK"),
        }
    }
    Ok(())
}
fn main() {
    if let Err(error) = run(Cli::parse()) {
        eprintln!("condite-ctl: {error:#}");
        std::process::exit(1);
    }
}
