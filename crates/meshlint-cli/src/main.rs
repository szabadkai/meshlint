use clap::{Parser, ValueEnum};
use meshlint_core::{LintOptions, Severity, Thresholds, fix_mesh_bytes, lint_mesh_bytes};
use std::path::{Path, PathBuf};

#[derive(Debug, Parser)]
#[command(name = "meshlint", about = "Like ESLint, but for 3D models.")]
struct Args {
    model: PathBuf,

    #[arg(long, default_value = "sla")]
    process: String,

    #[arg(long)]
    fix: bool,

    #[arg(long)]
    out: Option<PathBuf>,

    #[arg(long, default_value_t = 0.8)]
    wall_min: f32,

    #[arg(long, default_value_t = 1.0)]
    tiny_shell_volume: f32,

    #[arg(long, default_value_t = 0.01)]
    weld_tolerance: f32,

    #[arg(long, default_value_t = 0.05)]
    layer_height: f32,

    #[arg(long, default_value_t = 1200.0)]
    large_cross_section: f32,

    #[arg(long, default_value_t = 0.01)]
    tiny_edge: f32,

    #[arg(long, default_value_t = 100.0)]
    bad_aspect_ratio: f32,

    #[arg(long, default_value_t = 0.5)]
    suspicious_min_dimension: f32,

    #[arg(long, default_value_t = 1000.0)]
    suspicious_max_dimension: f32,

    #[arg(long, default_value_t = 50_000)]
    max_self_intersection_tests: usize,

    #[arg(long, default_value_t = 1_000)]
    max_findings_per_rule: usize,

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
    let options = LintOptions {
        process: args.process,
        thresholds: Thresholds {
            wall_min_mm: args.wall_min,
            tiny_shell_volume_mm3: args.tiny_shell_volume,
            weld_tolerance_mm: args.weld_tolerance,
            layer_height_mm: args.layer_height,
            large_cross_section_mm2: args.large_cross_section,
            tiny_edge_mm: args.tiny_edge,
            bad_aspect_ratio: args.bad_aspect_ratio,
            suspicious_min_dimension_mm: args.suspicious_min_dimension,
            suspicious_max_dimension_mm: args.suspicious_max_dimension,
            max_self_intersection_tests: args.max_self_intersection_tests,
            max_findings_per_rule: args.max_findings_per_rule,
        },
    };

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
