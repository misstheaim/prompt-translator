//! Quality regression harness (plan T15): runs `eval/cases.jsonl` through the real
//! `TranslationService` and a real provider, and writes a Markdown report.
//!
//! Usage (see `eval/README.md`):
//! `cargo run --bin eval -- --provider ollama|claude-code [--model <name>] [--cases <file>]
//! [--out <file>] [--preset <id>]`.
//!
//! No Tauri runtime: the service is built from plain settings and preset stores that live in a
//! scratch directory under the system temp dir, so the user's real app data is never touched.
//! The bundled Claude Code provider reads its working dirs from `system::paths`, so the binary
//! points those roots at the same scratch directory.

use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use prompt_translator_lib::error::{AppError, ErrorKind};
use prompt_translator_lib::presets::PresetStore;
use prompt_translator_lib::settings::SettingsStore;
use prompt_translator_lib::system::paths;
use prompt_translator_lib::translation::model::{Mode, ProviderKind, TranslationRequest};
use prompt_translator_lib::translation::providers::claude_code::{
    ClaudeCodeProvider, Housekeeping, TranslatorPids,
};
use prompt_translator_lib::translation::providers::ollama::OllamaProvider;
use prompt_translator_lib::translation::service::{TranslationProvider, TranslationService};
use serde::Deserialize;
use std::sync::Arc;
use uuid::Uuid;

/// The eval gives a model more time than the app does: the first request may load the model.
const OLLAMA_TIMEOUT_SECS: u32 = 180;
const CLAUDE_TIMEOUT_SECS: u32 = 120;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Case {
    id: String,
    mode: Mode,
    source: String,
    #[serde(default)]
    must_keep: Vec<String>,
    #[serde(default)]
    notes: String,
}

struct Args {
    provider: ProviderKind,
    model: Option<String>,
    cases: PathBuf,
    out: Option<PathBuf>,
    preset: Option<String>,
}

/// One finished case.
struct Outcome {
    case: Case,
    result: Result<Success, AppError>,
    millis: u64,
}

struct Success {
    output: String,
    issues: Vec<String>,
    token_warnings: Vec<String>,
    model: String,
    missed: Vec<String>,
}

impl Outcome {
    /// The answer parsed (the harness's "JSON valid"): a result came back, so the output
    /// parser accepted it. Provider failures (timeout, unreachable) don't count as invalid JSON.
    fn json_valid(&self) -> Option<bool> {
        match &self.result {
            Ok(_) => Some(true),
            Err(e)
                if matches!(
                    e.kind(),
                    ErrorKind::MalformedOutput | ErrorKind::EmptyOutput
                ) =>
            {
                Some(false)
            }
            Err(_) => None,
        }
    }
}

const USAGE: &str = "Usage: cargo run --bin eval -- --provider ollama|claude-code \
[--model <name>] [--cases <file>] [--out <file>] [--preset <id>]";

fn parse_args() -> Result<Args, String> {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut provider = None;
    let mut model = None;
    let mut cases = manifest.join("..").join("eval").join("cases.jsonl");
    let mut out = None;
    let mut preset = None;
    let mut args = std::env::args().skip(1);
    while let Some(flag) = args.next() {
        if flag == "--help" || flag == "-h" {
            return Err(USAGE.into());
        }
        let value = args
            .next()
            .ok_or_else(|| format!("{flag} needs a value\n{USAGE}"))?;
        match flag.as_str() {
            "--provider" => {
                provider = Some(match value.as_str() {
                    "ollama" => ProviderKind::Ollama,
                    "claude-code" => ProviderKind::ClaudeCode,
                    other => {
                        return Err(format!(
                            "Unknown provider \"{other}\" (use ollama or claude-code)"
                        ));
                    }
                });
            }
            "--model" => model = Some(value),
            "--cases" => cases = PathBuf::from(value),
            "--out" => out = Some(PathBuf::from(value)),
            "--preset" => preset = Some(value),
            other => return Err(format!("Unknown argument \"{other}\"\n{USAGE}")),
        }
    }
    let provider = provider.ok_or_else(|| format!("--provider is required\n{USAGE}"))?;
    Ok(Args {
        provider,
        model,
        cases,
        out,
        preset,
    })
}

fn load_cases(path: &Path) -> Result<Vec<Case>, String> {
    let text =
        fs::read_to_string(path).map_err(|e| format!("Can't read {}: {e}", path.display()))?;
    text.lines()
        .enumerate()
        .filter(|(_, line)| !line.trim().is_empty())
        .map(|(i, line)| {
            serde_json::from_str::<Case>(line)
                .map_err(|e| format!("{}:{}: {e}", path.display(), i + 1))
        })
        .collect()
}

#[tokio::main]
async fn main() -> ExitCode {
    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("{message}");
            ExitCode::FAILURE
        }
    }
}

async fn run() -> Result<(), String> {
    let args = parse_args()?;
    let cases = load_cases(&args.cases)?;
    if cases.is_empty() {
        return Err(format!("{} has no cases", args.cases.display()));
    }

    // A scratch "app data" dir; presets are re-seeded from the bundled ones on every run.
    let work = std::env::temp_dir().join("prompt-translator-eval");
    let (config, local) = (work.join("config"), work.join("local"));
    let presets_dir = config.join("presets");
    if presets_dir.exists() {
        fs::remove_dir_all(&presets_dir).map_err(|e| format!("Can't reset presets: {e}"))?;
    }
    paths::init_roots(config.clone(), local);

    let settings_store = SettingsStore::load(config.join("settings.json"));
    let mut settings = settings_store.get();
    settings.default_provider = args.provider;
    settings.history_enabled = false;
    settings.ollama.timeout_secs = OLLAMA_TIMEOUT_SECS;
    settings.claude.timeout_secs = CLAUDE_TIMEOUT_SECS;
    if let Some(model) = &args.model {
        match args.provider {
            ProviderKind::Ollama => settings.ollama.model = model.clone(),
            _ => settings.claude.model = model.clone(),
        }
    }
    let preset_id = args
        .preset
        .clone()
        .unwrap_or_else(|| settings.default_preset_id.clone());
    let settings = settings_store
        .save(settings)
        .map_err(|e| format!("Settings: {e}"))?;
    let model_name = match args.provider {
        ProviderKind::Ollama => settings.ollama.model.clone(),
        _ => settings.claude.model.clone(),
    };

    let presets = PresetStore::open(presets_dir);
    if presets.get(&preset_id).is_none() {
        return Err(format!("The preset \"{preset_id}\" doesn't exist"));
    }
    let providers: Vec<Arc<dyn TranslationProvider>> = vec![
        Arc::new(OllamaProvider::new()),
        Arc::new(ClaudeCodeProvider::new(
            TranslatorPids::new(),
            Housekeeping::new(),
        )),
    ];
    let service = TranslationService::new(settings_store, presets, providers, None);

    eprintln!(
        "Running {} cases with {:?} ({model_name}), preset {preset_id}",
        cases.len(),
        args.provider
    );
    let total = cases.len();
    let mut outcomes = Vec::with_capacity(total);
    for (i, case) in cases.into_iter().enumerate() {
        let request = TranslationRequest {
            request_id: Uuid::new_v4().to_string(),
            source_text: case.source.clone(),
            mode: case.mode,
            provider: args.provider,
            preset_id: preset_id.clone(),
        };
        let started = std::time::Instant::now();
        let result = service.translate(request).await;
        let wall = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
        let outcome = to_outcome(case, result, wall);
        eprintln!(
            "[{}/{total}] {} ({} ms): {}",
            i + 1,
            outcome.case.id,
            outcome.millis,
            status_line(&outcome)
        );
        outcomes.push(outcome);
    }

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .as_secs();
    let report = render_report(&args, &model_name, &preset_id, &outcomes, now);
    let out = args.out.clone().unwrap_or_else(|| {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("eval")
            .join("reports")
            .join(format!("{}.md", file_stamp(now)))
    });
    if let Some(parent) = out.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Can't create {}: {e}", parent.display()))?;
    }
    fs::write(&out, report).map_err(|e| format!("Can't write {}: {e}", out.display()))?;
    eprintln!("Report written to {}", out.display());
    Ok(())
}

fn to_outcome(
    case: Case,
    result: Result<prompt_translator_lib::translation::model::TranslationResult, AppError>,
    wall: u64,
) -> Outcome {
    match result {
        Ok(r) => {
            let missed = case
                .must_keep
                .iter()
                .filter(|token| !r.output.contains(token.as_str()))
                .cloned()
                .collect();
            let issues = r
                .issues
                .iter()
                .map(|i| {
                    format!(
                        "\"{}\" -> \"{}\" ({}): {}",
                        i.original,
                        i.corrected,
                        serde_json::to_string(&i.category)
                            .unwrap_or_default()
                            .trim_matches('"'),
                        i.explanation
                    )
                })
                .collect();
            let token_warnings = r
                .token_warnings
                .iter()
                .map(|w| format!("{} ({:?})", w.token, w.kind))
                .collect();
            Outcome {
                millis: r.duration_ms,
                result: Ok(Success {
                    output: r.output,
                    issues,
                    token_warnings,
                    model: r.model,
                    missed,
                }),
                case,
            }
        }
        Err(e) => Outcome {
            case,
            result: Err(e),
            millis: wall,
        },
    }
}

fn status_line(outcome: &Outcome) -> String {
    match &outcome.result {
        Ok(s) if s.missed.is_empty() => "ok".into(),
        Ok(s) => format!("missing {}", s.missed.join(", ")),
        Err(e) => format!("error {:?}", e.kind()),
    }
}

fn render_report(args: &Args, model: &str, preset: &str, outcomes: &[Outcome], now: u64) -> String {
    let mut md = String::new();
    let _ = writeln!(md, "# Eval report {}", file_stamp(now));
    let _ = writeln!(md);
    let _ = writeln!(md, "- Provider: {:?}", args.provider);
    let _ = writeln!(md, "- Model: {model}");
    let _ = writeln!(md, "- Preset: {preset}");
    let _ = writeln!(md, "- Cases file: {}", args.cases.display());
    let _ = writeln!(md);

    // Summary.
    let validity: Vec<bool> = outcomes.iter().filter_map(Outcome::json_valid).collect();
    let valid = validity.iter().filter(|v| **v).count();
    let with_keep: Vec<&Outcome> = outcomes
        .iter()
        .filter(|o| !o.case.must_keep.is_empty())
        .collect();
    let keep_pass = with_keep
        .iter()
        .filter(|o| matches!(&o.result, Ok(s) if s.missed.is_empty()))
        .count();
    let mut latencies: Vec<u64> = outcomes
        .iter()
        .filter(|o| o.result.is_ok())
        .map(|o| o.millis)
        .collect();
    latencies.sort_unstable();
    let failed = outcomes.iter().filter(|o| o.result.is_err()).count();

    let _ = writeln!(md, "## Summary");
    let _ = writeln!(md);
    let _ = writeln!(md, "| Metric | Value |");
    let _ = writeln!(md, "|---|---|");
    let _ = writeln!(md, "| Cases | {} |", outcomes.len());
    let _ = writeln!(md, "| Failed (error returned) | {failed} |");
    let _ = writeln!(md, "| JSON-valid rate | {} |", rate(valid, validity.len()));
    let _ = writeln!(
        md,
        "| mustKeep pass rate | {} |",
        rate(keep_pass, with_keep.len())
    );
    if latencies.is_empty() {
        let _ = writeln!(md, "| Latency avg / p95 | n/a |");
    } else {
        let avg = latencies.iter().sum::<u64>() / latencies.len() as u64;
        let _ = writeln!(
            md,
            "| Latency avg / p95 | {avg} ms / {} ms |",
            percentile(&latencies, 95)
        );
    }
    let _ = writeln!(md);
    let _ = writeln!(
        md,
        "JSON-valid rate counts the cases whose answer reached the output parser; provider \
         failures (timeout, unreachable) are left out. mustKeep pass rate is over cases with a \
         non-empty `mustKeep` list; a failed case counts as a miss. The first case may include \
         model load time."
    );
    let _ = writeln!(md);

    // Per case.
    let _ = writeln!(md, "## Cases");
    for o in outcomes {
        let _ = writeln!(md);
        let _ = writeln!(md, "### {} ({:?}, {} ms)", o.case.id, o.case.mode, o.millis);
        let _ = writeln!(md);
        if !o.case.notes.is_empty() {
            let _ = writeln!(md, "_{}_", o.case.notes);
            let _ = writeln!(md);
        }
        let _ = writeln!(md, "- **Source:** {}", one_line(&o.case.source));
        match &o.result {
            Ok(s) => {
                let _ = writeln!(md, "- **Output:** {}", one_line(&s.output));
                let _ = writeln!(md, "- **Model:** {}", s.model);
                list(&mut md, "Issues", &s.issues);
                list(&mut md, "Token warnings", &s.token_warnings);
                if o.case.must_keep.is_empty() {
                    let _ = writeln!(md, "- **mustKeep misses:** n/a (no tokens)");
                } else {
                    list(&mut md, "mustKeep misses", &s.missed);
                }
            }
            Err(e) => {
                let p = e.payload();
                let _ = writeln!(md, "- **Error:** {:?}: {}", p.kind, p.message);
                if let Some(detail) = p.detail {
                    let _ = writeln!(md, "- **Detail:** {}", one_line(&detail));
                }
            }
        }
    }
    md
}

fn list(md: &mut String, title: &str, items: &[String]) {
    if items.is_empty() {
        let _ = writeln!(md, "- **{title}:** none");
        return;
    }
    let _ = writeln!(md, "- **{title}:**");
    for item in items {
        let _ = writeln!(md, "  - {}", one_line(item));
    }
}

/// Keeps a multi-line value on one Markdown list line.
fn one_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn rate(part: usize, whole: usize) -> String {
    if whole == 0 {
        return "n/a".into();
    }
    format!(
        "{:.1}% ({part}/{whole})",
        part as f64 * 100.0 / whole as f64
    )
}

/// Nearest-rank percentile of an ascending, non-empty slice.
fn percentile(sorted: &[u64], pct: usize) -> u64 {
    let rank = (pct * sorted.len()).div_ceil(100).max(1);
    sorted[rank - 1]
}

/// `YYYYMMDD-HHMMSS` in UTC.
fn file_stamp(unix_secs: u64) -> String {
    let days = (unix_secs / 86_400) as i64;
    let secs = unix_secs % 86_400;
    // Civil date from days since 1970-01-01 (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}{month:02}{day:02}-{:02}{:02}{:02}",
        secs / 3600,
        secs % 3600 / 60,
        secs % 60
    )
}
