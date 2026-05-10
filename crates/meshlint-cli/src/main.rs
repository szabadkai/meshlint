use clap::{Parser, ValueEnum};
use meshlint_core::{LintOptions, Severity, fix_mesh_bytes, lint_mesh_bytes};
use std::io;
use std::path::{Path, PathBuf};

const CONFIG_FILE_NAME: &str = ".meshlintrc";

#[derive(Debug, Parser)]
#[command(name = "meshlint", about = "Like ESLint, but for 3D models.")]
struct Args {
    model: PathBuf,

    #[arg(long)]
    process: Option<String>,

    #[arg(long)]
    fix: bool,

    #[arg(long)]
    out: Option<PathBuf>,

    #[arg(long)]
    wall_min: Option<f32>,

    #[arg(long)]
    tiny_shell_volume: Option<f32>,

    #[arg(long)]
    weld_tolerance: Option<f32>,

    #[arg(long)]
    layer_height: Option<f32>,

    #[arg(long)]
    large_cross_section: Option<f32>,

    #[arg(long)]
    tiny_edge: Option<f32>,

    #[arg(long)]
    bad_aspect_ratio: Option<f32>,

    #[arg(long)]
    suspicious_min_dimension: Option<f32>,

    #[arg(long)]
    suspicious_max_dimension: Option<f32>,

    #[arg(long)]
    max_self_intersection_tests: Option<usize>,

    #[arg(long)]
    max_findings_per_rule: Option<usize>,

    #[arg(long, value_enum, default_value_t = OutputFormat::Text)]
    format: OutputFormat,
}

#[derive(Clone, Debug, ValueEnum)]
enum OutputFormat {
    Text,
    Json,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("ERROR: {error}");
        std::process::exit(2);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let bytes = std::fs::read(&args.model)?;
    let format = infer_format(&args.model);
    let options = options_from_args(&args)?;

    if args.fix {
        let report = fix_mesh_bytes(&bytes, &format, options)?;
        if matches!(args.format, OutputFormat::Json) {
            println!("{}", serde_json::to_string_pretty(&report)?);
        } else {
            print_fix_report(&args.model, &report);
        }

        let out = args.out.unwrap_or_else(|| fixed_path(&args.model));
        std::fs::write(&out, &report.fixed_bytes)?;
        if matches!(args.format, OutputFormat::Text) {
            println!();
            println!("Wrote: {}", out.display());
        }

        if report
            .findings
            .iter()
            .any(|finding| finding.severity == Severity::Error)
        {
            std::process::exit(1);
        }
    } else {
        let report = lint_mesh_bytes(&bytes, &format, options)?;
        if matches!(args.format, OutputFormat::Json) {
            println!("{}", serde_json::to_string_pretty(&report)?);
        } else {
            print_text_report(&args.model, &[], &report.findings);
        }

        if report
            .findings
            .iter()
            .any(|finding| finding.severity == Severity::Error)
        {
            std::process::exit(1);
        }
    }

    Ok(())
}

fn options_from_args(args: &Args) -> Result<LintOptions, Box<dyn std::error::Error>> {
    let mut options = load_home_config()?.unwrap_or_default();

    if let Some(process) = &args.process {
        options.process.clone_from(process);
    }

    apply_threshold_flag(&mut options.thresholds.wall_min_mm, args.wall_min);
    apply_threshold_flag(
        &mut options.thresholds.tiny_shell_volume_mm3,
        args.tiny_shell_volume,
    );
    apply_threshold_flag(
        &mut options.thresholds.weld_tolerance_mm,
        args.weld_tolerance,
    );
    apply_threshold_flag(&mut options.thresholds.layer_height_mm, args.layer_height);
    apply_threshold_flag(
        &mut options.thresholds.large_cross_section_mm2,
        args.large_cross_section,
    );
    apply_threshold_flag(&mut options.thresholds.tiny_edge_mm, args.tiny_edge);
    apply_threshold_flag(
        &mut options.thresholds.bad_aspect_ratio,
        args.bad_aspect_ratio,
    );
    apply_threshold_flag(
        &mut options.thresholds.suspicious_min_dimension_mm,
        args.suspicious_min_dimension,
    );
    apply_threshold_flag(
        &mut options.thresholds.suspicious_max_dimension_mm,
        args.suspicious_max_dimension,
    );
    apply_threshold_flag(
        &mut options.thresholds.max_self_intersection_tests,
        args.max_self_intersection_tests,
    );
    apply_threshold_flag(
        &mut options.thresholds.max_findings_per_rule,
        args.max_findings_per_rule,
    );

    Ok(options)
}

fn apply_threshold_flag<T: Copy>(target: &mut T, value: Option<T>) {
    if let Some(value) = value {
        *target = value;
    }
}

fn load_home_config() -> Result<Option<LintOptions>, Box<dyn std::error::Error>> {
    let Some(home) = home_dir() else {
        return Ok(None);
    };
    let path = home.join(CONFIG_FILE_NAME);
    if !path.exists() {
        return Ok(None);
    }

    let config = std::fs::read_to_string(&path)?;
    Ok(Some(parse_config(&path, &config)?))
}

fn parse_config(path: &Path, config: &str) -> Result<LintOptions, io::Error> {
    serde_json::from_str(config).map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("failed to parse {}: {error}", path.display()),
        )
    })
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
}

fn infer_format(path: &Path) -> String {
    path.extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or("stl")
        .to_ascii_lowercase()
}

fn fixed_path(path: &Path) -> PathBuf {
    let stem = path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("model");
    let extension = path
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or("stl");
    path.with_file_name(format!("{stem}.fixed.{extension}"))
}

fn print_text_report(
    path: &Path,
    fixes: &[meshlint_core::Fix],
    findings: &[meshlint_core::Finding],
) {
    println!("{}", path.display());
    if fixes.is_empty() && findings.is_empty() {
        println!("  OK: no findings");
        return;
    }
    for fix in fixes {
        println!("  FIXED: {}", fix.message);
    }
    for finding in findings {
        print_finding(finding);
    }
}

fn print_fix_report(path: &Path, report: &meshlint_core::FixReport) {
    println!("{}", path.display());
    if report.fixes.is_empty() {
        println!("  FIXED: no safe automatic fixes applied");
    } else {
        for fix in &report.fixes {
            println!("  FIXED: {}", fix.message);
        }
    }

    let initial_errors = count_severity(&report.initial_findings, Severity::Error);
    let initial_warnings = count_severity(&report.initial_findings, Severity::Warn);
    let remaining_errors = count_severity(&report.findings, Severity::Error);
    let remaining_warnings = count_severity(&report.findings, Severity::Warn);

    println!(
        "  SUMMARY: errors {initial_errors} -> {remaining_errors}, warnings {initial_warnings} -> {remaining_warnings}"
    );

    for finding in &report.findings {
        print_finding(finding);
    }
}

fn count_severity(findings: &[meshlint_core::Finding], severity: Severity) -> usize {
    findings
        .iter()
        .filter(|finding| finding.severity == severity)
        .count()
}

fn print_finding(finding: &meshlint_core::Finding) {
    let severity = match finding.severity {
        Severity::Error => "ERROR",
        Severity::Warn => "WARN",
        Severity::Info => "INFO",
    };
    let face_context = format_face_context(&finding.face_ids);
    if face_context.is_empty() {
        println!("  {severity}: {} [{}]", finding.message, finding.rule_id);
    } else {
        println!(
            "  {severity}: {} {} [{}]",
            finding.message, face_context, finding.rule_id
        );
    }
}

fn format_face_context(face_ids: &[u32]) -> String {
    match face_ids {
        [] => String::new(),
        [one] => format!("(face {one})"),
        [a, b] => format!("(faces {a}, {b})"),
        many => {
            let preview = many
                .iter()
                .take(4)
                .map(u32::to_string)
                .collect::<Vec<_>>()
                .join(", ");
            let remaining = many.len().saturating_sub(4);
            if remaining == 0 {
                format!("(faces {preview})")
            } else {
                format!("(faces {preview}, +{remaining} more)")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_partial_meshlintrc_with_defaults() {
        let options = parse_config(
            Path::new("/home/user/.meshlintrc"),
            r#"{
                "process": "fdm",
                "thresholds": {
                    "tiny_edge_mm": 0.02,
                    "max_findings_per_rule": 0
                }
            }"#,
        )
        .unwrap();

        assert_eq!(options.process, "fdm");
        assert_eq!(options.thresholds.tiny_edge_mm, 0.02);
        assert_eq!(options.thresholds.max_findings_per_rule, 0);
        assert_eq!(options.thresholds.wall_min_mm, 0.8);
    }

    #[test]
    fn rejects_invalid_meshlintrc_json() {
        let error = parse_config(Path::new("/home/user/.meshlintrc"), "{").unwrap_err();

        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert!(error.to_string().contains(".meshlintrc"));
    }
}
