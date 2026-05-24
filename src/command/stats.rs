use std::{
    collections::BTreeMap,
    fmt,
    io::{self, Write},
};

use clap::Parser;
use serde::Serialize;
use walkdir::WalkDir;

use crate::utils::{
    error::{CliError, CliResult, StableErrorCode},
    output::{OutputConfig, emit_json_data},
};

#[derive(Parser, Debug)]
pub struct StatsArgs;

#[derive(Debug, Clone, Serialize)]
struct StatsEntry {
    extension: String,
    count: usize,
}

#[derive(Debug, Clone, Serialize)]
struct StatsOutput {
    total_files: usize,
    stats: Vec<StatsEntry>,
}

pub async fn execute_safe(_args: StatsArgs, output: &OutputConfig) -> CliResult<()> {
    let stats_output = collect_stats()?;

    if output.is_json() {
        emit_json_data("stats", &stats_output, output)?;
    } else if !output.quiet {
        let mut stdout = std::io::stdout();
        render_stats_output(&stats_output, &mut stdout)?;
    }

    Ok(())
}

pub async fn execute_to(_args: StatsArgs, writer: &mut impl Write) -> CliResult<()> {
    let stats_output = collect_stats()?;
    render_stats_output(&stats_output, writer)
}

pub async fn execute(args: StatsArgs) {
    if let Err(e) = execute_safe(args, &OutputConfig::default()).await {
        e.print_stderr();
    }
}

fn collect_stats() -> CliResult<StatsOutput> {
    let mut ext_map: BTreeMap<String, usize> = BTreeMap::new();

    for entry in WalkDir::new(".")
        .into_iter()
        .filter_entry(|e| {
            let name = e.file_name().to_string_lossy();
            name != ".libra" && name != "target"
        })
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if entry.file_type().is_file() {
            let ext = match entry.path().extension() {
                Some(s) => {
                    let ext_str = s.to_string_lossy();
                    if ext_str.is_empty() {
                        "no_extension".to_string()
                    } else {
                        format!(".{ext_str}")
                    }
                }
                None => "no_extension".to_string(),
            };
            *ext_map.entry(ext).or_insert(0) += 1;
        }
    }

    let total_files: usize = ext_map.values().sum();
    let stats: Vec<StatsEntry> = ext_map
        .into_iter()
        .map(|(extension, count)| StatsEntry { extension, count })
        .collect();

    Ok(StatsOutput { total_files, stats })
}

fn render_stats_output(output: &StatsOutput, writer: &mut impl Write) -> CliResult<()> {
    let max_count = output
        .stats
        .iter()
        .map(|e| e.count)
        .max()
        .unwrap_or(0);
    let width = std::cmp::max(4, max_count.to_string().len());

    for entry in &output.stats {
        if !write_stats_line(
            writer,
            format_args!("{:>width$}  {}", entry.count, entry.extension, width = width),
        )? {
            return Ok(());
        }
    }

    Ok(())
}

fn write_stats_line(writer: &mut impl Write, args: fmt::Arguments<'_>) -> CliResult<bool> {
    match writer.write_fmt(args) {
        Ok(()) => {}
        Err(err) if err.kind() == io::ErrorKind::BrokenPipe => return Ok(false),
        Err(err) => return Err(stats_output_error(err)),
    }

    match writer.write_all(b"\n") {
        Ok(()) => Ok(true),
        Err(err) if err.kind() == io::ErrorKind::BrokenPipe => Ok(false),
        Err(err) => Err(stats_output_error(err)),
    }
}

fn stats_output_error(err: io::Error) -> CliError {
    CliError::fatal(format!("stats output error: {err}"))
        .with_stable_code(StableErrorCode::IoWriteFailed)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::*;
    use crate::utils::test::ChangeDirGuard;

    #[test]
    fn test_parse_args() {
        let _args = StatsArgs::parse_from(["stats"]);
    }

    #[test]
    fn test_collect_stats_counts_extensions() {
        let dir = tempdir().unwrap();
        let _guard = ChangeDirGuard::new(dir.path());

        fs::write("foo.rs", "fn main() {}").unwrap();
        fs::write("bar.rs", "fn helper() {}").unwrap();
        fs::write("baz.md", "# Title").unwrap();
        fs::write("qux.toml", "[package]\n").unwrap();
        fs::write("readme", "no extension").unwrap();

        let output = collect_stats().unwrap();
        assert_eq!(output.total_files, 5);

        let mut map: BTreeMap<String, usize> = output
            .stats
            .iter()
            .map(|e| (e.extension.clone(), e.count))
            .collect();

        assert_eq!(map.remove(".rs"), Some(2));
        assert_eq!(map.remove(".md"), Some(1));
        assert_eq!(map.remove(".toml"), Some(1));
        assert_eq!(map.remove("no_extension"), Some(1));
    }

    #[test]
    fn test_collect_stats_ignores_libra_and_target() {
        let dir = tempdir().unwrap();
        let _guard = ChangeDirGuard::new(dir.path());

        fs::create_dir_all(".libra/objects").unwrap();
        fs::create_dir_all("target/debug").unwrap();
        fs::create_dir_all("src").unwrap();
        fs::write("src/main.rs", "fn main() {}").unwrap();
        fs::write(".libra/objects/abc123", "data").unwrap();
        fs::write("target/debug/libra", "binary").unwrap();

        let output = collect_stats().unwrap();
        assert_eq!(output.total_files, 1);

        let map: BTreeMap<String, usize> = output
            .stats
            .iter()
            .map(|e| (e.extension.clone(), e.count))
            .collect();

        assert_eq!(map.get(".rs"), Some(&1));
        assert!(map.get("no_extension").is_none());
    }

    #[test]
    fn test_collect_stats_empty_directory() {
        let dir = tempdir().unwrap();
        let _guard = ChangeDirGuard::new(dir.path());

        let output = collect_stats().unwrap();
        assert_eq!(output.total_files, 0);
        assert!(output.stats.is_empty());
    }

    #[test]
    fn test_render_stats_output() {
        let output = StatsOutput {
            total_files: 3,
            stats: vec![
                StatsEntry {
                    extension: ".rs".to_string(),
                    count: 2,
                },
                StatsEntry {
                    extension: "no_extension".to_string(),
                    count: 1,
                },
            ],
        };

        let mut buf = Vec::new();
        render_stats_output(&output, &mut buf).unwrap();
        let rendered = String::from_utf8(buf).unwrap();

        assert!(rendered.contains("   2  .rs"));
        assert!(rendered.contains("   1  no_extension"));
    }

    #[test]
    fn test_broken_pipe_writer_is_ignored() {
        struct BrokenPipeWriter;

        impl Write for BrokenPipeWriter {
            fn write(&mut self, _buf: &[u8]) -> io::Result<usize> {
                Err(io::Error::from(io::ErrorKind::BrokenPipe))
            }
            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }

        let mut writer = BrokenPipeWriter;
        assert!(
            !write_stats_line(&mut writer, format_args!("test")).unwrap(),
            "BrokenPipe should terminate output quietly"
        );
    }

    #[test]
    fn test_non_broken_pipe_writer_error_is_structured() {
        struct PermissionDeniedWriter;

        impl Write for PermissionDeniedWriter {
            fn write(&mut self, _buf: &[u8]) -> io::Result<usize> {
                Err(io::Error::from(io::ErrorKind::PermissionDenied))
            }
            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }

        let mut writer = PermissionDeniedWriter;
        let err = write_stats_line(&mut writer, format_args!("test")).unwrap_err();
        assert_eq!(err.stable_code(), StableErrorCode::IoWriteFailed);
        assert!(err.message().contains("stats output error"));
    }

    #[test]
    fn test_serializable_stats_output() {
        let output = StatsOutput {
            total_files: 1,
            stats: vec![StatsEntry {
                extension: ".rs".to_string(),
                count: 1,
            }],
        };

        let json = serde_json::to_string(&output).unwrap();
        assert!(json.contains(r#""total_files":1"#));
        assert!(json.contains(r#""extension":".rs""#));
        assert!(json.contains(r#""count":1"#));
    }
}
