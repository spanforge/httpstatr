mod analysis;
mod cli;
mod comparison;
mod diagnostics;
mod error;
mod history;
mod model;
mod output;
mod policy;
mod redact;
mod runner;
mod suite;

use std::collections::VecDeque;
use std::env;
use std::fs;
use std::io::{self, Write};
use std::process::ExitCode;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
    mpsc,
};
use std::thread;
use std::time::{Duration, Instant};
use std::time::{SystemTime, UNIX_EPOCH};

use cli::{Action, CompletionShell, Config, OutputFormat};
use comparison::{
    REGRESSION_EXIT_CODE, Rule, compare, evaluate as evaluate_regression, load_baseline,
    parse_rules,
};
use error::AppError;
use model::{
    AssertionSummary, BatchConfiguration, BatchResult, BatchSample, BatchSummary,
    ConnectionDetails, JsonResult, ResponseHeaders, Violation,
};
use policy::{ASSERTION_EXIT_CODE, Expectations, check_slo, parse_slo};
use redact::{
    redact_assertions, redact_command, redact_display_headers, redact_header_line, redact_headers,
    redact_url,
};
use runner::{CurlRunner, Request, RunOutput, Runner};
use suite::{SuiteEndpointResult, SuiteResult, SuiteSummary};

const PARTIAL_FAILURE_EXIT_CODE: u8 = 6;
const MAX_BODY_ASSERTION_BYTES: u64 = 16 * 1024 * 1024;

fn main() -> ExitCode {
    let raw: Vec<String> = env::args().skip(1).collect();
    if raw.is_empty() {
        print!("{}", cli::help());
        return ExitCode::SUCCESS;
    }
    let action = match cli::parse(raw) {
        Ok(action) => action,
        Err(error) if error.code == 0 => {
            print!("{}", error.message);
            return ExitCode::SUCCESS;
        }
        Err(error) => {
            eprint!("{}", error.message);
            if !error.message.ends_with('\n') {
                eprintln!();
            }
            return ExitCode::from(error.code);
        }
    };
    let config = match action {
        Action::Run(config) => *config,
        Action::History(command) => {
            return match history::run(command) {
                Ok(()) => ExitCode::SUCCESS,
                Err(error) => {
                    eprintln!("Error: {error}");
                    ExitCode::from(error.code)
                }
            };
        }
        Action::Completion(shell) => {
            if let Err(error) = generate_completion(shell) {
                eprintln!("Error: {error}");
                return ExitCode::from(error.code);
            }
            return ExitCode::SUCCESS;
        }
        Action::ManPage => {
            if let Err(error) = generate_man_page() {
                eprintln!("Error: {error}");
                return ExitCode::from(error.code);
            }
            return ExitCode::SUCCESS;
        }
    };
    match run(config) {
        Ok(code) => ExitCode::from(code),
        Err(error) => {
            eprintln!("Error: {error}");
            ExitCode::from(error.code)
        }
    }
}

fn run(mut config: Config) -> Result<u8, AppError> {
    if config.suite_file.is_some() {
        return run_suite(config);
    }
    let slo = config.slo.as_deref().map(parse_slo).transpose()?;
    let expectations = Expectations::parse(
        &config.expect_status,
        &config.expect_header,
        config.min_body_bytes,
        config.max_body_bytes,
        &config.expect_body_contains,
        &config.expect_body_regex,
    )?;
    let baseline = config.compare.as_deref().map(load_baseline).transpose()?;
    let regression_rules = parse_rules(&config.fail_if, baseline.is_some())?;
    configure_curl(&mut config)?;
    log_config(&config);
    if config.repeat > 1
        || config.warmup > 0
        || config.compare.is_some()
        || !config.fail_if.is_empty()
        || config.history.is_some()
        || matches!(
            config.format,
            OutputFormat::Csv
                | OutputFormat::Junit
                | OutputFormat::Markdown
                | OutputFormat::Openmetrics
                | OutputFormat::Har
        )
    {
        let result = execute_batch(
            &config,
            slo.as_deref(),
            expectations.as_ref(),
            baseline.as_ref(),
            &regression_rules,
        )?;
        render_batch(&config, &result)
    } else {
        run_single(config, slo, expectations)
    }
}

fn run_single(
    config: Config,
    slo: Option<Vec<(String, u64)>>,
    expectations: Option<Expectations>,
) -> Result<u8, AppError> {
    let runner = CurlRunner;
    let output = runner.execute(&request(&config))?;
    let evaluation = evaluate(&config, slo.as_deref(), expectations.as_ref(), &output);
    if evaluation.is_err() {
        let _ = fs::remove_file(&output.body_path);
    }
    let evaluated = evaluation?;
    let execution = (|| -> Result<(), AppError> {
        match config.format {
            OutputFormat::Json => println!("{}", output::json::render(&evaluated.result, true)?),
            OutputFormat::Jsonl => {
                println!("{}", output::json::render(&evaluated.result, false)?)
            }
            OutputFormat::Pretty => output::pretty::render(
                &config,
                &output.metrics,
                &evaluated.display_headers,
                &output.body_path,
                &evaluated.violations,
                evaluated.assertions.as_ref(),
            )?,
            OutputFormat::Csv | OutputFormat::Openmetrics | OutputFormat::Har => {
                unreachable!("aggregate output uses the batch path")
            }
            OutputFormat::Junit | OutputFormat::Markdown => {
                unreachable!("CI reports use the batch path")
            }
        }
        save_json(&config, &evaluated.result)?;
        Ok(())
    })();
    if !config.save_body || config.format != OutputFormat::Pretty {
        let _ = fs::remove_file(&output.body_path);
    }
    execution?;
    Ok(evaluated.result.exit_code)
}

fn execute_batch(
    config: &Config,
    slo: Option<&[(String, u64)]>,
    expectations: Option<&Expectations>,
    baseline: Option<&BatchResult>,
    regression_rules: &[Rule],
) -> Result<BatchResult, AppError> {
    let runner = CurlRunner;
    let delay = Duration::from_secs_f64(config.delay);
    let mut warmup_failures = 0;
    for index in 0..config.warmup {
        match runner.execute(&request(config)) {
            Ok(output) => {
                let _ = fs::remove_file(output.body_path);
            }
            Err(error) => {
                warmup_failures += 1;
                if config.debug {
                    eprintln!("warmup #{} failed: {error}", index + 1);
                }
            }
        }
        if !delay.is_zero() {
            thread::sleep(delay);
        }
    }

    let started = Instant::now();
    let mut samples = Vec::with_capacity(config.repeat);
    let mut successful_metrics = Vec::with_capacity(config.repeat);
    let mut transport_failures = 0;
    let mut assertion_failures = 0;
    let mut slo_failures = 0;
    for index in 1..=config.repeat {
        let started_at_unix_ms = unix_time_ms()?;
        match runner.execute(&request(config)) {
            Ok(output) => {
                let evaluation = evaluate(config, slo, expectations, &output);
                let _ = fs::remove_file(&output.body_path);
                let evaluated = evaluation?;
                if evaluated
                    .assertions
                    .as_ref()
                    .is_some_and(|summary| !summary.pass)
                {
                    assertion_failures += 1;
                }
                if !evaluated.violations.is_empty() {
                    slo_failures += 1;
                }
                successful_metrics.push(output.metrics.clone());
                samples.push(BatchSample::success(
                    index,
                    started_at_unix_ms,
                    evaluated.result,
                    evaluated.body_size,
                    output
                        .redirects
                        .iter()
                        .map(|hop| {
                            let mut hop = hop.clone();
                            if !config.show_secrets {
                                hop.source_url = redact_url(&hop.source_url);
                                hop.destination_url = redact_url(&hop.destination_url);
                            }
                            hop
                        })
                        .collect(),
                    ConnectionDetails {
                        remote_ip: output.metrics.remote_ip.clone(),
                        remote_port: output.metrics.remote_port.clone(),
                        local_ip: output.metrics.local_ip.clone(),
                        local_port: output.metrics.local_port.clone(),
                        http_version: output.metrics.http_version.clone(),
                        effective_url: if config.show_secrets {
                            output.metrics.effective_url.clone()
                        } else {
                            redact_url(&output.metrics.effective_url)
                        },
                        connection_count: output.metrics.connection_count,
                        reused: output.metrics.connection_count
                            < output.metrics.redirect_count.saturating_add(1),
                        redirect_count: output.metrics.redirect_count,
                        redirect_time_ms: output.metrics.redirect_time,
                    },
                ));
            }
            Err(error) => {
                transport_failures += 1;
                samples.push(BatchSample::failure(
                    index,
                    started_at_unix_ms,
                    error.code,
                    error.message,
                ));
            }
        }
        if index < config.repeat && !delay.is_zero() {
            thread::sleep(delay);
        }
    }
    let elapsed = started.elapsed();
    let passed = samples.iter().filter(|sample| sample.ok).count();
    let failed = config.repeat - passed;
    let duration_ms = elapsed.as_millis().min(u64::MAX as u128) as u64;
    let requests_per_second = if elapsed.as_secs_f64() > 0.0 {
        config.repeat as f64 / elapsed.as_secs_f64()
    } else {
        0.0
    };
    let base_exit_code = if transport_failures > 0 {
        PARTIAL_FAILURE_EXIT_CODE
    } else if assertion_failures > 0 {
        ASSERTION_EXIT_CODE
    } else if slo_failures > 0 {
        4
    } else {
        0
    };
    let display_url = if config.show_secrets {
        config.url.clone()
    } else {
        redact_url(&config.url)
    };
    let display_curl_args = if config.show_secrets {
        config.curl_args.clone()
    } else {
        let mut args = redact_command("", &config.curl_args);
        args.pop();
        args
    };
    let display_expected_headers = if config.show_secrets {
        config.expect_header.clone()
    } else {
        config
            .expect_header
            .iter()
            .map(|header| redact_header_line(header))
            .collect()
    };
    let mut result = BatchResult {
        schema_version: 3,
        url: display_url,
        ok: base_exit_code == 0,
        exit_code: base_exit_code,
        warmup_count: config.warmup,
        warmup_failures,
        configuration: BatchConfiguration {
            repeat: config.repeat,
            warmup: config.warmup,
            delay_seconds: config.delay,
            connect_timeout_seconds: config.connect_timeout,
            timeout_seconds: config.timeout,
            curl_args: display_curl_args,
            curl_version: config.curl_version.clone(),
            slo: config.slo.clone(),
            expect_status: config.expect_status.clone(),
            expect_header: display_expected_headers,
            min_body_bytes: config.min_body_bytes,
            max_body_bytes: config.max_body_bytes,
            expect_body_contains: config.expect_body_contains.clone(),
            expect_body_regex: config.expect_body_regex.clone(),
            comparison_baseline: config
                .compare
                .as_ref()
                .map(|path| path.display().to_string()),
            regression_rules: config.fail_if.clone(),
        },
        summary: BatchSummary {
            requested: config.repeat,
            transport_successful: successful_metrics.len(),
            passed,
            failed,
            success_rate: round_two(passed as f64 / config.repeat as f64 * 100.0),
            duration_ms,
            requests_per_second: round_two(requests_per_second),
        },
        aggregate: analysis::aggregate(&successful_metrics),
        samples,
        comparison: None,
        regression: None,
        diagnostics: Vec::new(),
    };

    if let Some(baseline) = baseline {
        result.comparison = Some(compare(&result, baseline)?);
    }
    if !regression_rules.is_empty() {
        result.regression = Some(evaluate_regression(regression_rules, &result, baseline)?);
    }
    if base_exit_code == 0
        && result
            .regression
            .as_ref()
            .is_some_and(|regression| !regression.pass)
    {
        result.exit_code = REGRESSION_EXIT_CODE;
        result.ok = false;
    }
    result.diagnostics = diagnostics::analyze(&result);

    Ok(result)
}

fn render_batch(config: &Config, result: &BatchResult) -> Result<u8, AppError> {
    match config.format {
        OutputFormat::Pretty => output::pretty::render_batch(result),
        OutputFormat::Json => println!("{}", output::json::render(&result, true)?),
        OutputFormat::Jsonl => println!("{}", output::json::render(&result, false)?),
        OutputFormat::Csv => print!("{}", output::csv::render(result)),
        OutputFormat::Junit => print!("{}", output::junit::render(result)),
        OutputFormat::Markdown => print!("{}", output::markdown::render(result)),
        OutputFormat::Openmetrics => print!("{}", output::openmetrics::render(result)),
        OutputFormat::Har => println!("{}", output::har::render(result)?),
    }
    save_json(config, result)?;
    if let Some(path) = &config.history {
        history::append(
            path,
            result,
            config.history_name.clone(),
            config.history_tags.clone(),
            config.commit.clone(),
        )?;
    }
    Ok(result.exit_code)
}

struct Evaluated {
    result: JsonResult,
    display_headers: ResponseHeaders,
    violations: Vec<Violation>,
    assertions: Option<AssertionSummary>,
    body_size: u64,
}

fn evaluate(
    config: &Config,
    slo: Option<&[(String, u64)]>,
    expectations: Option<&Expectations>,
    output: &RunOutput,
) -> Result<Evaluated, AppError> {
    let body_size = fs::metadata(&output.body_path)
        .map_err(|error| AppError::local(format!("could not inspect response body: {error}")))?
        .len();
    let body = if expectations.is_some_and(Expectations::requires_body_content) {
        Some(read_body_for_assertions(&output.body_path, body_size)?)
    } else {
        None
    };
    let violations = check_slo(slo, &output.metrics);
    let assertion_summary = expectations
        .map(|expectations| expectations.check(&output.headers, body_size, body.as_deref()));
    let assertions_failed = assertion_summary
        .as_ref()
        .is_some_and(|summary| !summary.pass);
    let exit_code = if assertions_failed {
        ASSERTION_EXIT_CODE
    } else if !violations.is_empty() {
        4
    } else {
        0
    };
    let display_headers = if config.show_secrets {
        output.headers.clone()
    } else {
        let mut headers = output.headers.clone();
        headers.fields = redact_headers(&headers.fields);
        headers.display_text = redact_display_headers(&headers.display_text);
        headers
    };
    let display_assertions = assertion_summary.as_ref().map(|summary| {
        if config.show_secrets {
            summary.clone()
        } else {
            redact_assertions(summary)
        }
    });
    let display_url = if config.show_secrets {
        config.url.clone()
    } else {
        redact_url(&config.url)
    };
    let result = JsonResult::build(
        &display_url,
        &output.metrics,
        &display_headers,
        slo.map(|_| violations.clone()),
        display_assertions.clone(),
        exit_code,
    );
    Ok(Evaluated {
        result,
        display_headers,
        violations,
        assertions: display_assertions,
        body_size,
    })
}

fn request<'a>(config: &'a Config) -> Request<'a> {
    Request {
        url: &config.url,
        curl_args: &config.curl_args,
        curl_bin: &config.curl_bin,
        connect_timeout: config.connect_timeout,
        timeout: config.timeout,
        debug: config.debug,
        show_secrets: config.show_secrets,
    }
}

fn save_json(config: &Config, value: &impl serde::Serialize) -> Result<(), AppError> {
    let Some(path) = &config.save else {
        return Ok(());
    };
    let pretty = config.format != OutputFormat::Jsonl;
    let serialized = output::json::render(value, pretty)?;
    fs::write(path, format!("{serialized}\n"))
        .map_err(|error| AppError::local(format!("could not save {}: {error}", path.display())))
}

fn log_config(config: &Config) {
    if !config.debug {
        return;
    }
    eprintln!("HTTPSTAT_SHOW_BODY={}", config.show_body);
    eprintln!("HTTPSTAT_SHOW_IP={}", config.show_ip);
    eprintln!("HTTPSTAT_SHOW_SPEED={}", config.show_speed);
    eprintln!("HTTPSTAT_SAVE_BODY={}", config.save_body);
    eprintln!("HTTPSTAT_CURL_BIN={}", config.curl_bin);
    eprintln!("HTTPSTAT_DEBUG=true");
    eprintln!("connect_timeout={:?}", config.connect_timeout);
    eprintln!("timeout={:?}", config.timeout);
    eprintln!("repeat={}", config.repeat);
    eprintln!("warmup={}", config.warmup);
    eprintln!("delay={}", config.delay);
}

fn round_two(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

fn unix_time_ms() -> Result<u64, AppError> {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| AppError::local(format!("system clock error: {error}")))?
        .as_millis();
    Ok(millis.min(u64::MAX as u128) as u64)
}

fn configure_curl(config: &mut Config) -> Result<(), AppError> {
    let runtime = runner::validate_installation(&config.curl_bin)?;
    config.curl_version = Some(runtime.version);
    Ok(())
}

fn read_body_for_assertions(path: &std::path::Path, body_size: u64) -> Result<String, AppError> {
    if body_size > MAX_BODY_ASSERTION_BYTES {
        return Err(AppError::local(format!(
            "response body is {body_size} bytes; content assertions are limited to {MAX_BODY_ASSERTION_BYTES} bytes"
        )));
    }
    let bytes = fs::read(path)
        .map_err(|error| AppError::local(format!("could not read response body: {error}")))?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

fn generate_completion(shell: CompletionShell) -> Result<(), AppError> {
    let shell = match shell {
        CompletionShell::Bash => clap_complete::Shell::Bash,
        CompletionShell::Zsh => clap_complete::Shell::Zsh,
        CompletionShell::Fish => clap_complete::Shell::Fish,
        CompletionShell::PowerShell => clap_complete::Shell::PowerShell,
    };
    let mut command = cli::command();
    clap_complete::generate(shell, &mut command, "httpstatr", &mut io::stdout());
    Ok(())
}

fn generate_man_page() -> Result<(), AppError> {
    let command = cli::command();
    let mut buffer = Vec::new();
    clap_mangen::Man::new(command)
        .render(&mut buffer)
        .map_err(|error| AppError::local(format!("could not generate man page: {error}")))?;
    io::stdout()
        .write_all(&buffer)
        .map_err(|error| AppError::local(format!("could not write man page: {error}")))
}

#[derive(Clone)]
struct SuiteJob {
    index: usize,
    name: String,
    url: String,
    config: Config,
}

fn run_suite(mut config: Config) -> Result<u8, AppError> {
    let suite_path = config
        .suite_file
        .as_ref()
        .ok_or_else(|| AppError::local("missing suite path"))?
        .clone();
    let definition = suite::load(&suite_path)?;
    configure_curl(&mut config)?;
    let concurrency = suite::effective_concurrency(config.concurrency, definition.concurrency)?
        .min(definition.requests.len());
    let jobs: VecDeque<_> = definition
        .requests
        .iter()
        .enumerate()
        .map(|(index, endpoint)| SuiteJob {
            index,
            name: endpoint.name.clone(),
            url: if config.show_secrets {
                endpoint.url.clone()
            } else {
                redact_url(&endpoint.url)
            },
            config: suite::endpoint_config(&config, &suite_path, &definition.defaults, endpoint),
        })
        .collect();
    let jobs = Arc::new(Mutex::new(jobs));
    let canceled = Arc::new(AtomicBool::new(false));
    let signal = Arc::clone(&canceled);
    ctrlc::set_handler(move || {
        signal.store(true, Ordering::SeqCst);
    })
    .map_err(|error| AppError::local(format!("could not install Ctrl-C handler: {error}")))?;
    let (sender, receiver) = mpsc::channel();
    let started = Instant::now();
    let mut workers = Vec::with_capacity(concurrency);
    for _ in 0..concurrency {
        let jobs = Arc::clone(&jobs);
        let canceled = Arc::clone(&canceled);
        let sender = sender.clone();
        workers.push(thread::spawn(move || {
            loop {
                let job = {
                    let mut queue = jobs.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
                    queue.pop_front()
                };
                let Some(job) = job else {
                    break;
                };
                let result = if canceled.load(Ordering::SeqCst) {
                    Err(AppError::new("suite endpoint canceled", 130))
                } else {
                    execute_suite_endpoint(&job.config)
                };
                if sender.send((job, result)).is_err() {
                    break;
                }
            }
        }));
    }
    drop(sender);
    let mut endpoints = Vec::with_capacity(definition.requests.len());
    for (job, result) in receiver {
        endpoints.push(match result {
            Ok(result) => SuiteEndpointResult {
                index: job.index,
                name: job.name,
                url: job.url,
                ok: result.ok,
                exit_code: result.exit_code,
                result: Some(result),
                error: None,
            },
            Err(error) => SuiteEndpointResult {
                index: job.index,
                name: job.name,
                url: job.url,
                ok: false,
                exit_code: error.code,
                result: None,
                error: Some(error.message),
            },
        });
    }
    for worker in workers {
        worker
            .join()
            .map_err(|_| AppError::local("suite worker thread panicked"))?;
    }
    if endpoints.len() != definition.requests.len() {
        return Err(AppError::local(
            "suite ended before every endpoint produced a result",
        ));
    }
    endpoints.sort_by_key(|endpoint| endpoint.index);
    let passed = endpoints.iter().filter(|endpoint| endpoint.ok).count();
    let total = endpoints.len();
    let policies = suite::evaluate_policies(
        &definition.policies,
        config.suite_min_success_rate,
        config.suite_max_failures,
        passed,
        total,
    );
    let total_samples = endpoints
        .iter()
        .filter_map(|endpoint| endpoint.result.as_ref())
        .map(|result| result.summary.requested)
        .sum();
    let was_canceled = canceled.load(Ordering::SeqCst);
    let result = SuiteResult {
        schema_version: 1,
        ok: policies.pass && !was_canceled,
        exit_code: if was_canceled {
            130
        } else if policies.pass {
            0
        } else {
            suite::SUITE_FAILURE_EXIT_CODE
        },
        source: suite_path.display().to_string(),
        concurrency,
        canceled: was_canceled,
        summary: SuiteSummary {
            total_endpoints: total,
            passed,
            failed: total - passed,
            success_rate: round_two(passed as f64 / total as f64 * 100.0),
            total_samples,
            duration_ms: started.elapsed().as_millis().min(u64::MAX as u128) as u64,
        },
        policies,
        endpoints,
    };
    match config.format {
        OutputFormat::Pretty => output::suite::pretty(&result),
        OutputFormat::Json => println!("{}", output::json::render(&result, true)?),
        OutputFormat::Jsonl => println!("{}", output::json::render(&result, false)?),
        OutputFormat::Csv => print!("{}", output::suite::csv(&result)),
        OutputFormat::Junit => print!("{}", output::suite::junit(&result)),
        OutputFormat::Markdown => print!("{}", output::suite::markdown(&result)),
        OutputFormat::Openmetrics | OutputFormat::Har => {
            return Err(AppError::local(
                "OpenMetrics and HAR output currently require a single endpoint",
            ));
        }
    }
    save_json(&config, &result)?;
    Ok(result.exit_code)
}

fn execute_suite_endpoint(config: &Config) -> Result<BatchResult, AppError> {
    cli::validate_curl_args(&config.curl_args)?;
    let slo = config.slo.as_deref().map(parse_slo).transpose()?;
    let expectations = Expectations::parse(
        &config.expect_status,
        &config.expect_header,
        config.min_body_bytes,
        config.max_body_bytes,
        &config.expect_body_contains,
        &config.expect_body_regex,
    )?;
    let baseline = config.compare.as_deref().map(load_baseline).transpose()?;
    let regression_rules = parse_rules(&config.fail_if, baseline.is_some())?;
    execute_batch(
        config,
        slo.as_deref(),
        expectations.as_ref(),
        baseline.as_ref(),
        &regression_rules,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_oversized_body_before_reading_it() {
        let error = read_body_for_assertions(
            std::path::Path::new("file-does-not-need-to-exist"),
            MAX_BODY_ASSERTION_BYTES + 1,
        )
        .unwrap_err();
        assert!(error.message.contains("limited"));
    }
}
