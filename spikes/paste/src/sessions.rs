//! `sessions`: read-only. Recent session transcripts in `~/.claude/projects/<project>/<id>.jsonl`
//! with their `entrypoint` (`claude-vscode` for the VS Code extension, `cli` for a terminal).
//! Never prints message content.

use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::time::SystemTime;

use crate::{Exit, Flags, usage};

/// Lines read per transcript while looking for its entrypoint.
const SCAN_LINES: usize = 200;

pub fn run(args: &[String]) -> Result<(), Exit> {
    let flags = Flags::parse(args, &["limit", "entrypoint"], &[]).map_err(usage)?;
    let limit: usize = flags.number("limit", 15).map_err(usage)?;
    let wanted = flags.value("entrypoint");
    let home = std::env::var_os("USERPROFILE").ok_or_else(|| usage("USERPROFILE not set"))?;
    let root = PathBuf::from(home).join(".claude").join("projects");

    let mut files: Vec<(SystemTime, PathBuf)> = Vec::new();
    for project in std::fs::read_dir(&root).into_iter().flatten().flatten() {
        for entry in std::fs::read_dir(project.path())
            .into_iter()
            .flatten()
            .flatten()
        {
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "jsonl")
                && let Ok(modified) = entry.metadata().and_then(|m| m.modified())
            {
                files.push((modified, path));
            }
        }
    }
    files.sort_by_key(|(modified, _)| std::cmp::Reverse(*modified));

    let now = SystemTime::now();
    let mut shown = 0;
    for (modified, path) in files {
        if shown >= limit {
            break;
        }
        let entrypoint = entrypoint(&path).unwrap_or_else(|| "?".into());
        if wanted.is_some_and(|w| w != entrypoint) {
            continue;
        }
        shown += 1;
        let minutes = now
            .duration_since(modified)
            .map(|d| d.as_secs() / 60)
            .unwrap_or(0);
        let id = path.file_stem().and_then(|s| s.to_str()).unwrap_or("?");
        let project = path
            .parent()
            .and_then(|p| p.file_name())
            .and_then(|s| s.to_str())
            .unwrap_or("?");
        println!("{id}  entrypoint={entrypoint}  modified={minutes} min ago  project={project}");
    }
    Ok(())
}

/// The first `"entrypoint":"..."` value in the transcript's first lines.
fn entrypoint(path: &PathBuf) -> Option<String> {
    const KEY: &str = "\"entrypoint\":\"";
    let reader = BufReader::new(File::open(path).ok()?);
    for line in reader.lines().take(SCAN_LINES) {
        let line = line.ok()?;
        if let Some(start) = line.find(KEY) {
            let rest = &line[start + KEY.len()..];
            return rest.find('"').map(|end| rest[..end].to_string());
        }
    }
    None
}
