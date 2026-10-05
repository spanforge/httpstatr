use std::env;
use std::path::PathBuf;

use clap::{Args, CommandFactory, Parser, Subcommand, ValueEnum};

use crate::error::AppError;

pub const MAX_REPEAT: usize = 10_000;
pub const MAX_WARMUPS: usize = 10_000;

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum CompletionShell {
    Bash,
    Zsh,
    Fish,
    #[value(name = "powershell")]
    PowerShell,
}

pub enum Action {
    Doctor(String),
    Run(Box<Config>),
    History(HistoryCommand),
    Completion(CompletionShell),
    ManPage,
}

#[derive(Clone, Copy, Debug, PartialEq, ValueEnum)]
pub enum OutputFormat {
    Pretty,
    Json,
    Jsonl,
    Csv,
    Junit,
    Markdown,
    Openmetrics,
    Har,
}

#[derive(Clone, Debug)]
pub struct HistoryCommand {
    pub path: PathBuf,
    pub action: HistoryAction,
}

#[derive(Clone, Debug)]
pub enum HistoryAction {
    List {
        last: usize,
        name: Option<String>,
        url: Option<String>,
        tag: Option<String>,
        commit: Option<String>,
        since: Option<u64>,
        until: Option<u64>,
        chart: bool,
    },
    Prune {
        keep: usize,
    },
    Import {
        source: PathBuf,
    },
    Export {
        output: Option<PathBuf>,
    },
}

#[derive(Debug, Parser)]
#[command(
    name = "httpstatr history",
    about = "Query and maintain local measurement history"
)]
struct HistoryArguments {
    /// Append-only JSONL history file.
    #[arg(long, default_value = ".httpstatr/history.jsonl")]
    history: PathBuf,
    #[command(subcommand)]
    command: Option<HistorySubcommand>,
}

#[derive(Debug, Subcommand)]
enum HistorySubcommand {
    /// List matching records, newest last.
    List(HistoryListArguments),
    /// Atomically retain only the newest records.
    Prune {
        #[arg(long)]
        keep: usize,
    },
    /// Import compatible JSONL records.
    Import { source: PathBuf },
    /// Export all records to stdout or a file.
    Export {
        #[arg(long)]
        output: Option<PathBuf>,
    },
}

#[derive(Debug, Args)]
struct HistoryListArguments {
    #[arg(long, default_value_t = 30)]
    last: usize,
    #[arg(long)]
    name: Option<String>,
    #[arg(long)]
    url: Option<String>,
    #[arg(long)]
    tag: Option<String>,
    #[arg(long)]
    commit: Option<String>,
    /// Earliest Unix timestamp (seconds), inclusive.
    #[arg(long)]
    since: Option<u64>,
    /// Latest Unix timestamp (seconds), inclusive.
    #[arg(long)]
    until: Option<u64>,
    #[arg(long)]
    chart: bool,
}

#[derive(Debug, Parser)]
#[command(
    name = "httpstatr",
    version,
    about = "Visualize curl request timings and enforce HTTP performance policies",
    trailing_var_arg = true
)]
struct Arguments {
    /// Read a versioned TOML endpoint suite.
    #[arg(long)]
    file: Option<PathBuf>,

    /// Select variables from a named suite environment profile.
    #[arg(long, requires = "file")]
    profile: Option<String>,

    /// Maximum number of suite endpoints running concurrently.
    #[arg(long, value_parser = positive_integer)]
    concurrency: Option<usize>,

    /// Suite-level minimum endpoint pass rate percentage.
    #[arg(long, value_parser = percentage)]
    suite_min_success_rate: Option<f64>,

    /// Suite-level maximum failed endpoint count.
    #[arg(long)]
    suite_max_failures: Option<usize>,

    /// Generate completion code for a shell and exit.
    #[arg(long, value_enum)]
    generate_completion: Option<CompletionShell>,

    /// Generate the roff man page on stdout and exit.
    #[arg(long)]
    generate_man: bool,

    /// Output format.
    #[arg(short = 'f', long = "format", value_enum, default_value = "pretty")]
    format: OutputFormat,

    /// SLO thresholds such as total=500,connect=100.
    #[arg(long)]
    slo: Option<String>,

    /// Save the structured result to this path.
    #[arg(long)]
    save: Option<PathBuf>,

    /// Append the redacted aggregate result to a JSONL history file.
    #[arg(long)]
    history: Option<PathBuf>,

    /// Name stored with a history record.
    #[arg(long)]
    history_name: Option<String>,

    /// Tag stored with a history record. Repeatable.
    #[arg(long = "tag")]
    history_tags: Vec<String>,

    /// Source-control commit stored with a history record.
    #[arg(long)]
    commit: Option<String>,

    /// curl connection timeout in seconds.
    #[arg(long, value_parser = positive_number)]
    connect_timeout: Option<f64>,

    /// curl total request timeout in seconds.
    #[arg(long, value_parser = positive_number)]
    timeout: Option<f64>,

    /// Maximum response download size in bytes (default 64 MiB).
    #[arg(long, default_value_t = 67_108_864, value_parser = clap::value_parser!(u64).range(1..))]
    max_download_bytes: u64,

    /// Maximum wall-clock duration of the whole run, in seconds.
    #[arg(long, value_parser = positive_number)]
    run_timeout: Option<f64>,

    /// Validate a suite and its policies without making requests.
    #[arg(long, requires = "file")]
    validate: bool,

    /// curl executable path or command name.
    #[arg(long)]
    curl_bin: Option<String>,

    /// Expected status code or range, such as 200 or 200-299. Repeatable.
    #[arg(long = "expect-status")]
    expect_status: Vec<String>,

    /// Expected header as NAME or NAME:VALUE. Repeatable.
    #[arg(long = "expect-header")]
    expect_header: Vec<String>,

    /// Allow sensitive values in debug and response output.
    #[arg(long)]
    show_secrets: bool,

    /// Number of measured requests.
    #[arg(long, default_value_t = 1, value_parser = measured_request_count)]
    repeat: usize,

    /// Number of unmeasured warmup requests.
    #[arg(long, default_value_t = 0, value_parser = warmup_count)]
    warmup: usize,

    /// Delay between requests in seconds.
    #[arg(long, default_value_t = 0.0, value_parser = nonnegative_number)]
    delay: f64,

    /// Minimum expected response body size in bytes.
    #[arg(long)]
    min_body_bytes: Option<u64>,

    /// Maximum expected response body size in bytes.
    #[arg(long)]
    max_body_bytes: Option<u64>,

    /// Require the response body to contain this text. Repeatable.
    #[arg(long = "expect-body-contains")]
    expect_body_contains: Vec<String>,

    /// Require the response body to match this regular expression. Repeatable.
    #[arg(long = "expect-body-regex")]
    expect_body_regex: Vec<String>,

    /// JSON Pointer assertion: /path=JSON or /path:type=TYPE. Repeatable.
    #[arg(long = "expect-json")]
    expect_json: Vec<String>,

    /// Compare aggregate timings with a saved schema v2 baseline.
    #[arg(long)]
    compare: Option<PathBuf>,

    /// Fail on a regression rule, such as 'p95.total > baseline*1.10'. Repeatable.
    #[arg(long = "fail-if")]
    fail_if: Vec<String>,

    /// URL to request.
    url: Option<String>,

    /// Additional options passed to curl.
    #[arg(allow_hyphen_values = true)]
    curl_args: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct Config {
    pub url: String,
    pub curl_args: Vec<String>,
    pub format: OutputFormat,
    pub slo: Option<String>,
    pub save: Option<PathBuf>,
    pub history: Option<PathBuf>,
    pub history_name: Option<String>,
    pub history_tags: Vec<String>,
    pub commit: Option<String>,
    pub show_body: bool,
    pub show_ip: bool,
    pub show_speed: bool,
    pub save_body: bool,
    pub curl_bin: String,
    pub curl_version: Option<String>,
    pub debug: bool,
    pub connect_timeout: Option<f64>,
    pub timeout: Option<f64>,
    pub canceled: std::sync::Arc<std::sync::atomic::AtomicBool>,
    pub deadline: Option<std::time::Instant>,
    pub max_download_bytes: u64,
    pub run_timeout: Option<f64>,
    pub validate: bool,
    pub expect_status: Vec<String>,
    pub expect_header: Vec<String>,
    pub show_secrets: bool,
    pub repeat: usize,
    pub warmup: usize,
    pub delay: f64,
    pub min_body_bytes: Option<u64>,
    pub max_body_bytes: Option<u64>,
    pub expect_body_contains: Vec<String>,
    pub expect_body_regex: Vec<String>,
    pub expect_json: Vec<String>,
    pub compare: Option<PathBuf>,
    pub fail_if: Vec<String>,
    pub suite_file: Option<PathBuf>,
    pub profile: Option<String>,
    pub concurrency: Option<usize>,
    pub suite_min_success_rate: Option<f64>,
    pub suite_max_failures: Option<usize>,
}

pub fn parse(raw: Vec<String>) -> Result<Action, AppError> {
    if raw.first().is_some_and(|value| value == "doctor") {
        #[derive(Parser)]
        #[command(
            name = "httpstatr doctor",
            about = "Check local curl and runtime configuration without making requests"
        )]
        struct DoctorArguments {
            #[arg(long)]
            curl_bin: Option<String>,
        }
        let arguments = DoctorArguments::try_parse_from(
            std::iter::once("httpstatr doctor".to_string()).chain(raw.into_iter().skip(1)),
        )
        .map_err(|e| AppError::new(e.to_string(), if e.use_stderr() { 2 } else { 0 }))?;
        return Ok(Action::Doctor(
            arguments.curl_bin.unwrap_or_else(default_curl_bin),
        ));
    }
    if raw.first().is_some_and(|value| value == "history") {
        let mut args = vec!["httpstatr history".to_string()];
        args.extend(raw.into_iter().skip(1));
        let parsed = HistoryArguments::try_parse_from(args).map_err(|error| {
            AppError::new(error.to_string(), if error.use_stderr() { 2 } else { 0 })
        })?;
        let action = match parsed
            .command
            .unwrap_or(HistorySubcommand::List(HistoryListArguments {
                last: 30,
                name: None,
                url: None,
                tag: None,
                commit: None,
                since: None,
                until: None,
                chart: false,
            })) {
            HistorySubcommand::List(value) => HistoryAction::List {
                last: value.last,
                name: value.name,
                url: value.url,
                tag: value.tag,
                commit: value.commit,
                since: value.since,
                until: value.until,
                chart: value.chart,
            },
            HistorySubcommand::Prune { keep } => HistoryAction::Prune { keep },
            HistorySubcommand::Import { source } => HistoryAction::Import { source },
            HistorySubcommand::Export { output } => HistoryAction::Export { output },
        };
        return Ok(Action::History(HistoryCommand {
            path: parsed.history,
            action,
        }));
    }
    let normalized = normalize(raw)?;
    let arguments = Arguments::try_parse_from(normalized).map_err(|error| {
        let code = if error.use_stderr() { 2 } else { 0 };
        AppError::new(error.to_string(), code)
    })?;
    if let Some(shell) = arguments.generate_completion {
        return Ok(Action::Completion(shell));
    }
    if arguments.generate_man {
        return Ok(Action::ManPage);
    }
    if arguments.file.is_some() && arguments.url.is_some() {
        return Err(AppError::new("URL and --file cannot be used together", 2));
    }
    if arguments.file.is_some()
        && matches!(
            arguments.format,
            OutputFormat::Openmetrics | OutputFormat::Har
        )
    {
        return Err(AppError::new(
            "OpenMetrics and HAR output require a single URL and cannot be combined with --file",
            2,
        ));
    }
    if arguments.file.is_none() && arguments.url.is_none() {
        return Err(AppError::new("a URL or --file is required", 2));
    }
    if arguments.history.is_some() && arguments.show_secrets {
        return Err(AppError::new(
            "--history cannot be combined with --show-secrets",
            2,
        ));
    }
    if arguments.history.is_some() && arguments.file.is_some() {
        return Err(AppError::new(
            "--history currently records one endpoint and cannot be combined with --file",
            2,
        ));
    }
    let metrics_only = env_bool("HTTPSTAT_METRICS_ONLY", false)?;
    let format = if metrics_only && arguments.format == OutputFormat::Pretty {
        OutputFormat::Json
    } else {
        arguments.format
    };
    validate_curl_args(&arguments.curl_args)?;
    Ok(Action::Run(Box::new(Config {
        url: arguments.url.unwrap_or_default(),
        curl_args: arguments.curl_args,
        format,
        slo: arguments.slo,
        save: arguments.save,
        history: arguments.history,
        history_name: arguments.history_name,
        history_tags: arguments.history_tags,
        commit: arguments.commit,
        show_body: env_bool("HTTPSTAT_SHOW_BODY", false)?,
        show_ip: env_bool("HTTPSTAT_SHOW_IP", true)?,
        show_speed: env_bool("HTTPSTAT_SHOW_SPEED", false)?,
        save_body: env_bool("HTTPSTAT_SAVE_BODY", true)?,
        curl_bin: arguments.curl_bin.unwrap_or_else(default_curl_bin),
        curl_version: None,
        debug: env_bool("HTTPSTAT_DEBUG", false)?,
        connect_timeout: arguments.connect_timeout,
        timeout: arguments.timeout,
        canceled: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
        deadline: None,
        max_download_bytes: arguments.max_download_bytes,
        run_timeout: arguments.run_timeout,
        validate: arguments.validate,
        expect_status: arguments.expect_status,
        expect_header: arguments.expect_header,
        show_secrets: arguments.show_secrets,
        repeat: arguments.repeat,
        warmup: arguments.warmup,
        delay: arguments.delay,
        min_body_bytes: arguments.min_body_bytes,
        max_body_bytes: arguments.max_body_bytes,
        expect_body_contains: arguments.expect_body_contains,
        expect_body_regex: arguments.expect_body_regex,
        expect_json: arguments.expect_json,
        compare: arguments.compare,
        fail_if: arguments.fail_if,
        suite_file: arguments.file,
        profile: arguments.profile,
        concurrency: arguments.concurrency,
        suite_min_success_rate: arguments.suite_min_success_rate,
        suite_max_failures: arguments.suite_max_failures,
    })))
}

pub fn help() -> String {
    format!(
        "{}\nOther commands: doctor, history\n",
        Arguments::command().render_long_help()
    )
}

fn default_curl_bin() -> String {
    env::var("HTTPSTAT_CURL_BIN").unwrap_or_else(|_| {
        if cfg!(windows) {
            "curl.exe".into()
        } else {
            "curl".into()
        }
    })
}

pub fn command() -> clap::Command {
    Arguments::command()
}

fn normalize(raw: Vec<String>) -> Result<Vec<String>, AppError> {
    let mut tool_args = Vec::new();
    let mut remaining = Vec::new();
    let mut index = 0;
    while index < raw.len() {
        let arg = &raw[index];
        if arg == "--" {
            remaining.extend(raw[index + 1..].iter().cloned());
            break;
        }
        if curl_option_takes_value(arg) {
            remaining.push(arg.clone());
            let value = raw
                .get(index + 1)
                .ok_or_else(|| AppError::new("curl option requires a value", 2))?;
            remaining.push(value.clone());
            index += 2;
            continue;
        }
        if matches!(arg.as_str(), "-h" | "--help" | "-V" | "--version") {
            tool_args.push(arg.clone());
            index += 1;
            continue;
        }
        if matches!(
            arg.as_str(),
            "--show-secrets" | "--generate-man" | "--validate"
        ) {
            tool_args.push(arg.clone());
            index += 1;
            continue;
        }
        let value_option = matches!(
            arg.as_str(),
            "-f" | "--format"
                | "--slo"
                | "--save"
                | "--connect-timeout"
                | "--timeout"
                | "--max-download-bytes"
                | "--run-timeout"
                | "--curl-bin"
                | "--expect-status"
                | "--expect-header"
                | "--repeat"
                | "--warmup"
                | "--delay"
                | "--min-body-bytes"
                | "--max-body-bytes"
                | "--expect-body-contains"
                | "--expect-body-regex"
                | "--expect-json"
                | "--compare"
                | "--fail-if"
                | "--file"
                | "--profile"
                | "--concurrency"
                | "--suite-min-success-rate"
                | "--suite-max-failures"
                | "--generate-completion"
                | "--history"
                | "--history-name"
                | "--tag"
                | "--commit"
        );
        if value_option {
            let Some(value) = raw.get(index + 1) else {
                return Err(AppError::new(format!("{arg} requires a value"), 2));
            };
            tool_args.push(arg.clone());
            tool_args.push(value.clone());
            index += 2;
            continue;
        }
        if [
            "--format=",
            "--slo=",
            "--save=",
            "--connect-timeout=",
            "--timeout=",
            "--max-download-bytes=",
            "--run-timeout=",
            "--curl-bin=",
            "--expect-status=",
            "--expect-header=",
            "--repeat=",
            "--warmup=",
            "--delay=",
            "--min-body-bytes=",
            "--max-body-bytes=",
            "--expect-body-contains=",
            "--expect-body-regex=",
            "--expect-json=",
            "--compare=",
            "--fail-if=",
            "--file=",
            "--profile=",
            "--concurrency=",
            "--suite-min-success-rate=",
            "--suite-max-failures=",
            "--generate-completion=",
            "--history=",
            "--history-name=",
            "--tag=",
            "--commit=",
        ]
        .iter()
        .any(|prefix| arg.starts_with(prefix))
        {
            tool_args.push(arg.clone());
        } else {
            remaining.push(arg.clone());
        }
        index += 1;
    }
    let mut normalized = vec!["httpstatr".to_string()];
    normalized.extend(tool_args);
    if !remaining.is_empty() {
        normalized.push("--".to_string());
        normalized.extend(remaining);
    }
    Ok(normalized)
}

fn positive_number(value: &str) -> Result<f64, String> {
    let parsed = value
        .parse::<f64>()
        .map_err(|_| "must be a positive number".to_string())?;
    if parsed.is_finite() && parsed > 0.0 && parsed <= 31_536_000.0 {
        Ok(parsed)
    } else {
        Err("must be positive and at most 31536000 seconds".to_string())
    }
}

fn nonnegative_number(value: &str) -> Result<f64, String> {
    let parsed = value
        .parse::<f64>()
        .map_err(|_| "must be a nonnegative number".to_string())?;
    if parsed.is_finite() && (0.0..=31_536_000.0).contains(&parsed) {
        Ok(parsed)
    } else {
        Err("must be nonnegative and at most 31536000 seconds".to_string())
    }
}

fn positive_integer(value: &str) -> Result<usize, String> {
    let parsed = value
        .parse::<usize>()
        .map_err(|_| "must be a positive integer".to_string())?;
    if parsed > 0 {
        Ok(parsed)
    } else {
        Err("must be a positive integer".to_string())
    }
}

fn measured_request_count(value: &str) -> Result<usize, String> {
    bounded_count(value, 1, MAX_REPEAT, "repeat")
}

fn warmup_count(value: &str) -> Result<usize, String> {
    bounded_count(value, 0, MAX_WARMUPS, "warmup")
}

fn bounded_count(
    value: &str,
    minimum: usize,
    maximum: usize,
    label: &str,
) -> Result<usize, String> {
    let parsed = value
        .parse::<usize>()
        .map_err(|_| format!("{label} must be an integer between {minimum} and {maximum}"))?;
    if (minimum..=maximum).contains(&parsed) {
        Ok(parsed)
    } else {
        Err(format!("{label} must be between {minimum} and {maximum}"))
    }
}

fn percentage(value: &str) -> Result<f64, String> {
    let parsed = nonnegative_number(value)?;
    if parsed <= 100.0 {
        Ok(parsed)
    } else {
        Err("must be between 0 and 100".to_string())
    }
}

fn env_bool(name: &str, default: bool) -> Result<bool, AppError> {
    let Ok(value) = env::var(name) else {
        return Ok(default);
    };
    match value.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Ok(true),
        "0" | "false" | "no" | "off" => Ok(false),
        _ => Err(AppError::local(format!(
            "invalid boolean value for {name}: {value:?}"
        ))),
    }
}

pub(crate) fn validate_curl_args(args: &[String]) -> Result<(), AppError> {
    const FORBIDDEN: &[&str] = &[
        "-w",
        "--write-out",
        "-D",
        "--dump-header",
        "-o",
        "--output",
        "-s",
        "--silent",
        "-S",
        "--show-error",
        "--config",
        "-K",
        "--next",
        "-:",
        "--parallel",
        "-Z",
        "--url",
        "--remote-name",
        "-O",
        "--remote-name-all",
        "--output-dir",
        "--trace",
        "--trace-ascii",
        "--stderr",
        "--max-time",
        "-m",
        "--max-filesize",
        "--retry",
        "--retry-all-errors",
        "--no-disable",
        "--no-globoff",
        "--proto",
        "--proto-redir",
    ];
    let mut index = 0;
    while index < args.len() {
        let arg = &args[index];
        let name = arg.split('=').next().unwrap_or(arg);
        if arg == "--" || arg.starts_with("http://") || arg.starts_with("https://") {
            return Err(AppError::local(
                "extra curl arguments cannot add another URL; use a suite for multiple endpoints",
            ));
        }
        let mut attached_reserved = false;
        if arg.starts_with('-') && !arg.starts_with("--") {
            for (position, option) in arg[1..].chars().enumerate() {
                if "wDosSK:ZOm".contains(option) {
                    attached_reserved = true;
                    break;
                }
                // Value-taking options consume the rest of a short-option token.
                if "HubdXAxUeTrECYyz".contains(option) {
                    if position > 0 {
                        attached_reserved = true;
                    }
                    break;
                }
            }
        }
        let reserved_abbreviation =
            name.starts_with("--") && FORBIDDEN.iter().any(|option| option.starts_with(name));
        if FORBIDDEN.contains(&name)
            || attached_reserved
            || reserved_abbreviation
            || name.starts_with("--retry")
            || name.starts_with("--parallel")
        {
            return Err(AppError::local(
                "extra curl arguments must not override output, limits, configuration, or transfer mode; use separate value-taking short options",
            ));
        }
        let takes_value = curl_option_takes_value(arg);
        index += if takes_value { 2 } else { 1 };
    }
    Ok(())
}

fn curl_option_takes_value(arg: &str) -> bool {
    matches!(
        arg,
        "-H" | "--header"
            | "-u"
            | "--user"
            | "-U"
            | "--proxy-user"
            | "-d"
            | "--data"
            | "--data-raw"
            | "--data-binary"
            | "--data-urlencode"
            | "--json"
            | "-b"
            | "--cookie"
            | "--oauth2-bearer"
            | "-X"
            | "--request"
            | "-A"
            | "--user-agent"
            | "-e"
            | "--referer"
            | "-x"
            | "--proxy"
            | "--cacert"
            | "--capath"
            | "-E"
            | "--cert"
            | "--key"
            | "--resolve"
            | "--connect-to"
            | "-T"
            | "--upload-file"
            | "-F"
            | "--form"
            | "--form-string"
            | "--interface"
            | "--limit-rate"
            | "--proxy-header"
            | "--preproxy"
            | "--doh-url"
            | "--request-target"
            | "--url-query"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_reserved_options_and_accepts_sensitive_values() {
        for option in [
            "-ofile",
            "-w%{url}",
            "-Dheaders",
            "-sS",
            "-Kconfig",
            "--config=x",
            "--next",
            "--parallel",
            "--url=x",
            "--max-time=1",
            "--trace=file",
        ] {
            assert!(validate_curl_args(&[option.into()]).is_err(), "{option}");
        }
        for args in [
            vec!["-uuser:password"],
            vec!["-XPOST"],
            vec!["-H", "-sensitive"],
            vec!["--data", "-some-data"],
            vec!["-v", "-L"],
        ] {
            assert!(
                validate_curl_args(&args.into_iter().map(String::from).collect::<Vec<_>>()).is_ok()
            );
        }
    }

    #[test]
    fn rejects_secret_history_and_unrepresentable_durations() {
        assert!(
            parse(vec![
                "https://example.test".into(),
                "--history=h".into(),
                "--show-secrets".into()
            ])
            .is_err()
        );
        assert!(positive_number("1e300").is_err());
        assert!(nonnegative_number("1e300").is_err());
    }

    #[test]
    fn curl_values_do_not_enable_tool_flags() {
        let Action::Run(config) = parse(vec![
            "https://example.test".into(),
            "-d".into(),
            "--show-secrets".into(),
        ])
        .unwrap() else {
            panic!("expected request");
        };
        assert!(!config.show_secrets);
        assert_eq!(config.curl_args, ["-d", "--show-secrets"]);
        let Action::Run(config) = parse(vec![
            "https://example.test".into(),
            "--".into(),
            "-H".into(),
            "--timeout=secret".into(),
        ])
        .unwrap() else {
            panic!("expected request");
        };
        assert_eq!(config.timeout, None);
        assert_eq!(config.curl_args, ["-H", "--timeout=secret"]);
    }

    #[test]
    fn extracts_tool_options_after_url() {
        let normalized = normalize(vec![
            "https://example.com".into(),
            "-X".into(),
            "POST".into(),
            "--format".into(),
            "json".into(),
            "--timeout=2".into(),
        ])
        .unwrap();
        assert_eq!(
            normalized,
            vec![
                "httpstatr",
                "--format",
                "json",
                "--timeout=2",
                "--",
                "https://example.com",
                "-X",
                "POST"
            ]
        );
    }

    #[test]
    fn validates_positive_timeout() {
        assert_eq!(positive_number("1.5").unwrap(), 1.5);
        assert!(positive_number("0").is_err());
        assert!(positive_number("NaN").is_err());
        assert_eq!(nonnegative_number("0").unwrap(), 0.0);
        assert!(nonnegative_number("-1").is_err());
        assert_eq!(positive_integer("2").unwrap(), 2);
        assert!(positive_integer("0").is_err());
    }
}
