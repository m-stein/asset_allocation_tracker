use std::{
    env,
    ffi::OsString,
    fs,
    path::PathBuf,
    process::{Command, Stdio},
};

use chrono::Local;
use eyre::{Result, WrapErr, bail, eyre};
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};

const DEFAULT_APP: &str = "tallytail";
const DEFAULT_REMOTE_DATA_DIR: &str = "/app/data";
const DEFAULT_FLYCTL: &str = "flyctl";
const ACCESS_TOKEN_HEADER: &str = "x-tallytail-access-token";

#[derive(Debug)]
struct Config {
    output_dir: PathBuf,
    app: String,
    backend_origin: String,
    unlock_pattern: String,
    remote_data_dir: String,
    flyctl: String,
}

#[derive(Debug, Deserialize)]
struct FlyMachine {
    id: String,
    state: String,
    config: Option<FlyMachineConfig>,
}

#[derive(Debug, Deserialize)]
struct FlyMachineConfig {
    mounts: Option<Vec<FlyMachineMount>>,
}

#[derive(Debug, Deserialize)]
struct FlyMachineMount {
    path: String,
}

#[derive(Debug, Serialize)]
struct UnlockPatternInput {
    pattern: String,
}

#[derive(Debug, Deserialize)]
struct AccessGrant {
    access_token: String,
}

fn main() -> Result<()> {
    let config = Config::parse(env::args_os().skip(1))?;
    run_backup(&config)
}

impl Config {
    fn parse(args: impl IntoIterator<Item = OsString>) -> Result<Self> {
        let mut output_dir = None;
        let mut app = DEFAULT_APP.to_owned();
        let mut backend_origin = None;
        let mut unlock_pattern = None;
        let mut remote_data_dir = DEFAULT_REMOTE_DATA_DIR.to_owned();
        let mut flyctl = DEFAULT_FLYCTL.to_owned();

        let mut args = args.into_iter();
        while let Some(arg) = args.next() {
            let arg_text = arg
                .to_str()
                .ok_or_else(|| eyre!("arguments must be valid UTF-8"))?;

            match arg_text {
                "-h" | "--help" => {
                    print_usage();
                    return Ok(Self {
                        output_dir: PathBuf::new(),
                        app,
                        backend_origin: String::new(),
                        unlock_pattern: String::new(),
                        remote_data_dir,
                        flyctl,
                    });
                }
                "--out-dir" => {
                    output_dir = Some(PathBuf::from(take_value(&mut args, "--out-dir")?));
                }
                "--app" => app = take_value(&mut args, "--app")?,
                "--url" => backend_origin = Some(take_value(&mut args, "--url")?),
                "--unlock-pattern" => {
                    unlock_pattern = Some(take_value(&mut args, "--unlock-pattern")?)
                }
                "--remote-data-dir" => {
                    remote_data_dir = take_value(&mut args, "--remote-data-dir")?;
                }
                "--flyctl" => flyctl = take_value(&mut args, "--flyctl")?,
                unknown if unknown.starts_with('-') => bail!("unknown option: {unknown}"),
                _ => bail!("unexpected positional argument: {arg_text}"),
            }
        }

        let output_dir = output_dir.ok_or_else(|| eyre!("missing --out-dir argument"))?;
        let backend_origin = backend_origin.ok_or_else(|| eyre!("missing --url argument"))?;
        let unlock_pattern =
            unlock_pattern.ok_or_else(|| eyre!("missing --unlock-pattern argument"))?;

        Ok(Self {
            output_dir,
            app,
            backend_origin,
            unlock_pattern,
            remote_data_dir,
            flyctl,
        })
    }
}

fn take_value(args: &mut impl Iterator<Item = OsString>, option: &str) -> Result<String> {
    args.next()
        .ok_or_else(|| eyre!("missing value for {option}"))?
        .into_string()
        .map_err(|_| eyre!("{option} value must be valid UTF-8"))
}

fn run_backup(config: &Config) -> Result<()> {
    if config.output_dir.as_os_str().is_empty() {
        return Ok(());
    }

    fs::create_dir_all(&config.output_dir)
        .wrap_err_with(|| format!("failed to create {}", config.output_dir.display()))?;

    let machine = ensure_machine_started(config)?;
    println!("using Fly machine {machine}");

    let timestamp = Local::now().format("%Y%m%d-%H%M%S");
    let backup_name = format!("{}-data-{timestamp}.tar.gz", config.app);
    let local_archive = config.output_dir.join(&backup_name);

    let archive = request_backup_archive(config)?;
    fs::write(&local_archive, archive)
        .wrap_err_with(|| format!("failed to write {}", local_archive.display()))?;

    println!("backup written to {}", local_archive.display());
    Ok(())
}

fn ensure_machine_started(config: &Config) -> Result<String> {
    let machine = select_machine(config)?;

    if machine.state == "started" {
        println!("using started Fly machine {}", machine.id);
        return Ok(machine.id);
    }

    println!(
        "starting Fly machine {} because it is {}",
        machine.id, machine.state
    );
    run_flyctl(
        config,
        [
            "machine",
            "start",
            machine.id.as_str(),
            "-a",
            config.app.as_str(),
        ],
    )
    .wrap_err("failed to start Fly machine")?;

    Ok(machine.id)
}

fn select_machine(config: &Config) -> Result<FlyMachine> {
    let output = run_flyctl_output(
        config,
        ["machine", "list", "-a", config.app.as_str(), "--json"],
    )
    .wrap_err("failed to list Fly machines")?;
    let machines: Vec<FlyMachine> =
        serde_json::from_slice(&output).wrap_err("failed to parse Fly machine list")?;

    if machines.is_empty() {
        bail!("app {} has no Fly machines", config.app);
    }

    let remote_data_dir = config.remote_data_dir.trim_end_matches('/');
    machines
        .into_iter()
        .filter(|machine| machine.has_mount_at(remote_data_dir))
        .min_by_key(|machine| machine.state != "started")
        .ok_or_else(|| eyre!("no Fly machine has a mount at {remote_data_dir}"))
}

impl FlyMachine {
    fn has_mount_at(&self, path: &str) -> bool {
        self.config
            .as_ref()
            .and_then(|config| config.mounts.as_ref())
            .is_some_and(|mounts| {
                mounts
                    .iter()
                    .any(|mount| mount.path.trim_end_matches('/') == path)
            })
    }
}

fn request_backup_archive(config: &Config) -> Result<Vec<u8>> {
    let client = Client::new();
    let origin = config.backend_origin.trim_end_matches('/');
    let unlock_url = format!("{origin}/unlock");
    let backup_url = format!("{origin}/create_data_backup");

    println!("unlocking backup request at {unlock_url}");
    let access_grant = client
        .post(&unlock_url)
        .json(&UnlockPatternInput {
            pattern: config.unlock_pattern.clone(),
        })
        .send()?
        .error_for_status()?
        .json::<AccessGrant>()?;

    println!("requesting data backup from {backup_url}");
    let archive = client
        .post(&backup_url)
        .header(ACCESS_TOKEN_HEADER, access_grant.access_token)
        .json(&())
        .send()?
        .error_for_status()?
        .json::<Vec<u8>>()?;

    Ok(archive)
}

fn run_flyctl<'a>(config: &Config, args: impl IntoIterator<Item = &'a str>) -> Result<()> {
    let args: Vec<&str> = args.into_iter().collect();
    println!("running: {} {}", config.flyctl, args.join(" "));

    let status = Command::new(&config.flyctl)
        .args(&args)
        .stdin(Stdio::null())
        .status()
        .wrap_err_with(|| format!("failed to run {}", config.flyctl))?;

    if !status.success() {
        bail!("{} exited with {status}", config.flyctl);
    }

    Ok(())
}

fn run_flyctl_output<'a>(
    config: &Config,
    args: impl IntoIterator<Item = &'a str>,
) -> Result<Vec<u8>> {
    let args: Vec<&str> = args.into_iter().collect();
    println!("running: {} {}", config.flyctl, args.join(" "));

    let output = Command::new(&config.flyctl)
        .args(&args)
        .stdin(Stdio::null())
        .output()
        .wrap_err_with(|| format!("failed to run {}", config.flyctl))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("{} exited with {}: {stderr}", config.flyctl, output.status);
    }

    Ok(output.stdout)
}

fn print_usage() {
    println!(
        "Usage: fly_data_backup --url <origin> --out-dir <dir> --unlock-pattern <pattern> [--app <app>] [--remote-data-dir <path>] [--flyctl <path>]"
    );
}
