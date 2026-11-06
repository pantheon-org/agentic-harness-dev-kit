//! Installing the git hook a skill declares, through the repository's own hook
//! manager.
//!
//! A skill that needs a hook ships a `hook.json` next to its `SKILL.md` (see
//! [`HookSpec`]). [`install_hook`] detects which manager the target repository
//! uses (hk, pre-commit, lefthook or husky) and adds the hook to that manager's
//! configuration file. It never writes into `.git/hooks`.
//!
//! # Ownership markers
//!
//! Every line the dev kit adds sits between a pair of marker comments,
//! `pahdk:begin <id>` and `pahdk:end <id>`, written in the comment syntax of the
//! file (`#`, or `//` for `hk.pkl`). The markers make install idempotent (a
//! begin marker for the id means the hook is already there) and let
//! [`uninstall_hook`] remove exactly what was added and nothing the user wrote.
//! Anything a user places between the markers is treated as the dev kit's.
//!
//! When an `hk.pkl` pre-commit hook sets `steps = someMapping`, the line is
//! rewritten to Pkl's amend form, `steps = (someMapping) { ... }`, and the
//! begin marker is the `pahdk:wrap-begin` variant so uninstall can put the
//! original line back.
//!
//! The library never prompts. When no manager, or several, are detected it
//! returns [`HookOutcome::NoManager`] or [`HookOutcome::Ambiguous`] so the
//! calling CLI can say so and ask, then call again with
//! [`HookOptions::manager`] set.

use std::path::{Path, PathBuf};

use pahdk_core::{Error, Result};
use serde::Deserialize;

/// A git-hook manager the dev kit can install into.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Manager {
    /// `hk`, configured by `hk.pkl`.
    Hk,
    /// `pre-commit`, configured by `.pre-commit-config.yaml`.
    PreCommit,
    /// `lefthook`, configured by `lefthook.yml` (or a dotted or `.yaml` variant).
    Lefthook,
    /// `husky`, configured by the scripts in `.husky/`.
    Husky,
}

impl Manager {
    /// Every supported manager.
    pub const ALL: [Manager; 4] = [
        Manager::Hk,
        Manager::PreCommit,
        Manager::Lefthook,
        Manager::Husky,
    ];

    /// The manager's name as users write it (`hk`, `pre-commit`, `lefthook`, `husky`).
    pub fn name(self) -> &'static str {
        match self {
            Manager::Hk => "hk",
            Manager::PreCommit => "pre-commit",
            Manager::Lefthook => "lefthook",
            Manager::Husky => "husky",
        }
    }

    /// Parse a manager name, for a `--hook-manager` style flag.
    pub fn parse(name: &str) -> Option<Manager> {
        Manager::ALL.into_iter().find(|m| m.name() == name)
    }

    /// The configuration file that exists in `repo` for this manager.
    fn existing_config(self, repo: &Path) -> Option<PathBuf> {
        let names: &[&str] = match self {
            Manager::Hk => &["hk.pkl"],
            Manager::PreCommit => &[".pre-commit-config.yaml"],
            Manager::Lefthook => &[
                "lefthook.yml",
                "lefthook.yaml",
                ".lefthook.yml",
                ".lefthook.yaml",
            ],
            Manager::Husky => return repo.join(".husky").is_dir().then(|| husky_file(repo)),
        };
        names.iter().map(|n| repo.join(n)).find(|p| p.is_file())
    }

    /// The file a fresh install writes to when none exists yet.
    fn default_config(self, repo: &Path) -> PathBuf {
        match self {
            Manager::Hk => repo.join("hk.pkl"),
            Manager::PreCommit => repo.join(".pre-commit-config.yaml"),
            Manager::Lefthook => repo.join("lefthook.yml"),
            Manager::Husky => husky_file(repo),
        }
    }
}

fn husky_file(repo: &Path) -> PathBuf {
    repo.join(".husky").join("pre-commit")
}

/// What [`detect_manager`] found in a repository.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Detection {
    /// No supported manager is configured.
    None,
    /// Exactly one manager is configured.
    One(Manager),
    /// Several managers are configured; the caller must choose.
    Many(Vec<Manager>),
}

/// Detect which hook managers `repo` is configured for.
pub fn detect_manager(repo: &Path) -> Detection {
    let mut found: Vec<Manager> = Manager::ALL
        .into_iter()
        .filter(|m| m.existing_config(repo).is_some())
        .collect();
    match found.len() {
        0 => Detection::None,
        1 => Detection::One(found.remove(0)),
        _ => Detection::Many(found),
    }
}

/// A hook a skill asks to have installed, read from its `hook.json`.
///
/// ```json
/// { "id": "adr-index", "command": "adr index", "description": "Regenerate the ADR index" }
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HookSpec {
    /// Lowercase kebab-case identifier, unique per skill. Names the hook in the
    /// manager's configuration and in the ownership markers.
    pub id: String,
    /// A single-line shell command run at pre-commit time.
    pub command: String,
    /// Optional human-readable summary. Not written to the configuration.
    #[serde(default)]
    pub description: Option<String>,
}

/// The file name of the hook manifest inside a skill directory.
pub const HOOK_FILE: &str = "hook.json";

impl HookSpec {
    /// Read `hook.json` from a skill directory. `Ok(None)` when the skill
    /// declares no hook.
    pub fn from_skill_dir(skill_dir: &Path) -> Result<Option<HookSpec>> {
        let path = skill_dir.join(HOOK_FILE);
        if !path.is_file() {
            return Ok(None);
        }
        let text = pahdk_core::fs::read_to_string(&path)?;
        let spec: HookSpec =
            serde_json::from_str(&text).map_err(|e| Error::json(path.display().to_string(), e))?;
        spec.validate()?;
        Ok(Some(spec))
    }

    /// Check the id and command are safe to write into a configuration file.
    pub fn validate(&self) -> Result<()> {
        valid_id(&self.id)?;
        if self.command.trim().is_empty() || self.command.contains(['\n', '\r']) {
            return Err(Error::Config(format!(
                "hook {}: command must be one non-empty line",
                self.id
            )));
        }
        Ok(())
    }

    /// The shell line every manager runs: a banner, then the command. The
    /// banner makes the first commit show that the hook ran.
    fn script(&self) -> String {
        format!(
            "echo \"[pahdk] {}: running\" >&2; {}",
            self.id,
            self.command.trim()
        )
    }
}

fn valid_id(id: &str) -> Result<()> {
    let ok = !id.is_empty()
        && !id.starts_with('-')
        && !id.ends_with('-')
        && id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
    if ok {
        Ok(())
    } else {
        Err(Error::Config(format!(
            "hook id {id:?} must be lowercase kebab-case (a-z, 0-9, '-')"
        )))
    }
}

/// Options for [`install_hook`].
#[derive(Debug, Clone, Default)]
pub struct HookOptions {
    /// The user declined the hook. Nothing is written or read.
    pub opt_out: bool,
    /// Use this manager instead of detecting one. Required to resolve
    /// [`HookOutcome::NoManager`] and [`HookOutcome::Ambiguous`].
    pub manager: Option<Manager>,
}

/// What [`install_hook`] did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HookOutcome {
    /// The hook was added to `file`.
    Installed {
        /// The manager whose configuration was changed.
        manager: Manager,
        /// The configuration file that was written.
        file: PathBuf,
    },
    /// The hook was already in `file`; nothing changed.
    AlreadyInstalled {
        /// The manager whose configuration holds the hook.
        manager: Manager,
        /// The configuration file that holds it.
        file: PathBuf,
    },
    /// The user opted out, now or earlier; nothing was written.
    OptedOut,
    /// No manager is configured. Ask the user, then retry with a manager.
    NoManager,
    /// Several managers are configured. Ask the user, then retry with one.
    Ambiguous(Vec<Manager>),
}

/// Add `spec` to the hook manager configuration of the repository at `repo`.
///
/// Idempotent and non-destructive: existing configuration is kept, and a hook
/// already present is left alone. See the module docs for the ownership markers.
pub fn install_hook(repo: &Path, spec: &HookSpec, opts: &HookOptions) -> Result<HookOutcome> {
    spec.validate()?;
    if opts.opt_out || is_opted_out(repo, &spec.id) {
        return Ok(HookOutcome::OptedOut);
    }
    let manager = match (opts.manager, detect_manager(repo)) {
        (Some(m), _) | (None, Detection::One(m)) => m,
        (None, Detection::None) => return Ok(HookOutcome::NoManager),
        (None, Detection::Many(found)) => return Ok(HookOutcome::Ambiguous(found)),
    };
    let file = manager
        .existing_config(repo)
        .unwrap_or_else(|| manager.default_config(repo));
    let existing = if file.is_file() {
        pahdk_core::fs::read_to_string(&file)?
    } else if manager == Manager::Hk {
        return Err(Error::NotFound(file));
    } else {
        String::new()
    };
    if has_marker(&existing, &spec.id) {
        return Ok(HookOutcome::AlreadyInstalled { manager, file });
    }
    let updated = match manager {
        Manager::Hk => add_hk(&existing, spec)?,
        Manager::PreCommit => add_pre_commit(&existing, spec)?,
        Manager::Lefthook => add_lefthook(&existing, spec)?,
        Manager::Husky => add_husky(&existing, spec),
    };
    write_file(&file, &updated)?;
    #[cfg(unix)]
    if manager == Manager::Husky {
        make_executable(&file)?;
    }
    Ok(HookOutcome::Installed { manager, file })
}

/// Remove the hook `id` from every manager configuration in `repo`.
///
/// Only the lines between the dev kit's markers go; a hk `steps` line the dev
/// kit rewrote is restored. A husky script left empty is deleted. Returns
/// whether anything was removed.
pub fn uninstall_hook(repo: &Path, id: &str) -> Result<bool> {
    valid_id(id)?;
    let mut removed = false;
    for manager in Manager::ALL {
        let Some(file) = manager.existing_config(repo) else {
            continue;
        };
        let text = pahdk_core::fs::read_to_string(&file)?;
        let Some(stripped) = strip_hook(&text, id) else {
            continue;
        };
        if manager == Manager::Husky && stripped.trim().is_empty() {
            std::fs::remove_file(&file).map_err(|e| Error::io(&file, e))?;
        } else {
            write_file(&file, &stripped)?;
        }
        removed = true;
    }
    Ok(removed)
}

fn opt_out_file(repo: &Path) -> PathBuf {
    repo.join(".git").join("pahdk-hooks.json")
}

#[derive(Debug, Default, Deserialize, serde::Serialize)]
struct OptOuts {
    #[serde(default)]
    opted_out: Vec<String>,
}

fn read_opt_outs(repo: &Path) -> OptOuts {
    std::fs::read_to_string(opt_out_file(repo))
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

/// Whether the user opted out of hook `id` earlier in this repository.
pub fn is_opted_out(repo: &Path, id: &str) -> bool {
    read_opt_outs(repo).opted_out.iter().any(|o| o == id)
}

/// Remember an opt-out in `.git/pahdk-hooks.json`, so a later update does not
/// ask again. Local to the clone: it is inside `.git` and never committed.
/// [`install_hook`] with `opt_out` set does not call this; the caller decides.
pub fn remember_opt_out(repo: &Path, id: &str) -> Result<()> {
    valid_id(id)?;
    let git = repo.join(".git");
    if !git.is_dir() {
        return Err(Error::NotFound(git));
    }
    let mut outs = read_opt_outs(repo);
    if !outs.opted_out.iter().any(|o| o == id) {
        outs.opted_out.push(id.to_string());
        outs.opted_out.sort();
    }
    let json = serde_json::to_string_pretty(&outs).map_err(|e| Error::json("hook opt-outs", e))?;
    write_file(&opt_out_file(repo), &format!("{json}\n"))
}

// ---------------------------------------------------------------- text edits

fn write_file(path: &Path, text: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| Error::io(parent, e))?;
    }
    std::fs::write(path, text).map_err(|e| Error::io(path, e))
}

#[cfg(unix)]
fn make_executable(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mut perms = std::fs::metadata(path)
        .map_err(|e| Error::io(path, e))?
        .permissions();
    perms.set_mode(perms.mode() | 0o755);
    std::fs::set_permissions(path, perms).map_err(|e| Error::io(path, e))
}

fn indent_of(line: &str) -> usize {
    line.len() - line.trim_start().len()
}

fn pad(n: usize) -> String {
    " ".repeat(n)
}

fn is_blank(line: &str) -> bool {
    line.trim().is_empty()
}

fn begin_marker(comment: &str, id: &str) -> String {
    format!("{comment} pahdk:begin {id}")
}

fn end_marker(comment: &str, id: &str) -> String {
    format!("{comment} pahdk:end {id}")
}

fn has_marker(text: &str, id: &str) -> bool {
    let begin = format!("pahdk:begin {id}");
    let wrap = format!("pahdk:wrap-begin {id}");
    text.lines()
        .any(|l| l.trim_end().ends_with(&begin) || l.trim_end().ends_with(&wrap))
}

fn to_lines(text: &str) -> Vec<String> {
    text.lines().map(str::to_string).collect()
}

fn from_lines(lines: &[String]) -> String {
    if lines.is_empty() {
        String::new()
    } else {
        format!("{}\n", lines.join("\n"))
    }
}

fn splice(lines: &mut Vec<String>, at: usize, block: Vec<String>) {
    lines.splice(at..at, block);
}

/// Remove every block for `id`. `None` when there was nothing to remove.
fn strip_hook(text: &str, id: &str) -> Option<String> {
    let begin = format!("pahdk:begin {id}");
    let wrap = format!("pahdk:wrap-begin {id}");
    let end = format!("pahdk:end {id}");
    let mut lines = to_lines(text);
    let mut changed = false;
    while let Some(i) = lines.iter().position(|l| {
        let l = l.trim_end();
        l.ends_with(&begin) || l.ends_with(&wrap)
    }) {
        let Some(offset) = lines[i..].iter().position(|l| l.trim_end().ends_with(&end)) else {
            // An unterminated block: leave the file alone rather than guess.
            return None;
        };
        let j = i + offset;
        if lines[i].trim_end().ends_with(&wrap) {
            lines[i] = unwrap_steps(&lines[i]);
            lines.drain(i + 1..=j);
        } else {
            lines.drain(i..=j);
        }
        changed = true;
    }
    changed.then(|| from_lines(&lines))
}

/// Turn `    steps = (name) {  // pahdk:wrap-begin id` back into `    steps = name`.
fn unwrap_steps(line: &str) -> String {
    let code = line.split("//").next().unwrap_or(line).trim_end();
    let indent = pad(indent_of(code));
    let name = code
        .trim_start()
        .strip_prefix("steps = (")
        .and_then(|r| r.strip_suffix(") {"))
        .unwrap_or("");
    format!("{indent}steps = {name}")
}

/// Index just past the last content line of the top-level section that starts
/// at `start`. A section runs until the next unindented line that is not a
/// comment or a sequence item.
fn section_end(lines: &[String], start: usize) -> usize {
    let mut end = lines.len();
    for (k, line) in lines.iter().enumerate().skip(start + 1) {
        let c = line.chars().next();
        if matches!(c, Some(ch) if !ch.is_whitespace() && ch != '#' && ch != '-') {
            end = k;
            break;
        }
    }
    while end > start + 1 && is_blank(&lines[end - 1]) {
        end -= 1;
    }
    end
}

fn unsupported(file: &str, what: &str, snippet: &str) -> Error {
    Error::Config(format!(
        "cannot add the hook to {file} automatically ({what}); add this yourself:\n{snippet}"
    ))
}

/// Single-quote `s` for a POSIX shell.
fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

/// A YAML double-quoted scalar. A JSON string is one.
fn yaml_quote(s: &str) -> String {
    serde_json::to_string(s).unwrap_or_else(|_| format!("\"{s}\""))
}

fn pkl_quote(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}

// ----------------------------------------------------------------- pre-commit

fn add_pre_commit(text: &str, spec: &HookSpec) -> Result<String> {
    let mut lines = to_lines(text);
    let repos = lines.iter().position(|l| l.starts_with("repos:"));
    let (at, ind) = match repos {
        Some(i) => {
            if lines[i].trim_end() != "repos:" {
                return Err(unsupported(
                    ".pre-commit-config.yaml",
                    "`repos:` is not a block list",
                    &pre_commit_block(spec, 0).join("\n"),
                ));
            }
            let ind = lines[i + 1..]
                .iter()
                .find(|l| l.trim_start().starts_with("- repo:"))
                .map_or(0, |l| indent_of(l));
            (section_end(&lines, i), ind)
        }
        None => {
            lines.push("repos:".to_string());
            (lines.len(), 0)
        }
    };
    splice(&mut lines, at, pre_commit_block(spec, ind));
    Ok(from_lines(&lines))
}

fn pre_commit_block(spec: &HookSpec, ind: usize) -> Vec<String> {
    let p = pad(ind);
    let entry = yaml_quote(&format!("sh -c {} --", shell_quote(&spec.script())));
    vec![
        format!("{p}{}", begin_marker("#", &spec.id)),
        format!("{p}- repo: local"),
        format!("{p}  hooks:"),
        format!("{p}    - id: {}", spec.id),
        format!("{p}      name: {}", spec.id),
        format!("{p}      entry: {entry}"),
        format!("{p}      language: system"),
        format!("{p}      pass_filenames: false"),
        format!("{p}      always_run: true"),
        format!("{p}      verbose: true"),
        format!("{p}{}", end_marker("#", &spec.id)),
    ]
}

// ------------------------------------------------------------------- lefthook

fn add_lefthook(text: &str, spec: &HookSpec) -> Result<String> {
    let mut lines = to_lines(text);
    let snippet = lefthook_block(spec, 0, 2, true).join("\n");
    let Some(i) = lines.iter().position(|l| l.starts_with("pre-commit:")) else {
        let at = lines.len();
        splice(&mut lines, at, lefthook_block(spec, 0, 2, true));
        return Ok(from_lines(&lines));
    };
    if lines[i].trim_end() != "pre-commit:" {
        return Err(unsupported(
            "the lefthook config",
            "`pre-commit:` has an inline value",
            &snippet,
        ));
    }
    let end = section_end(&lines, i);
    let step = lines[i + 1..end]
        .iter()
        .find(|l| !is_blank(l))
        .map_or(2, |l| indent_of(l));
    let commands = (i + 1..end).find(|&k| lines[k].trim_start().starts_with("commands:"));
    match commands {
        Some(k) => {
            if lines[k].trim_end().trim_start() != "commands:" {
                return Err(unsupported(
                    "the lefthook config",
                    "`commands:` has an inline value",
                    &snippet,
                ));
            }
            let ci = indent_of(&lines[k]);
            let entry = lines
                .get(k + 1)
                .filter(|l| k + 1 < end && !is_blank(l) && indent_of(l) > ci)
                .map_or(ci + 2, |l| indent_of(l));
            splice(&mut lines, k + 1, lefthook_block(spec, entry, 2, false));
        }
        None => splice(&mut lines, i + 1, lefthook_block(spec, step, step, true)),
    }
    Ok(from_lines(&lines))
}

/// `ind` is where the block starts. With `with_hook`, the block opens the
/// `pre-commit:` key itself (at column 0) or, when `ind > 0`, a `commands:` key.
fn lefthook_block(spec: &HookSpec, ind: usize, step: usize, with_wrapper: bool) -> Vec<String> {
    let mut out = Vec::new();
    let mut at = ind;
    let top_level = with_wrapper && ind == 0;
    out.push(format!("{}{}", pad(at), begin_marker("#", &spec.id)));
    if top_level {
        out.push("pre-commit:".to_string());
        at += step;
    }
    if with_wrapper {
        out.push(format!("{}commands:", pad(at)));
        at += step;
    }
    out.push(format!("{}{}:", pad(at), spec.id));
    out.push(format!(
        "{}run: {}",
        pad(at + step),
        yaml_quote(&spec.script())
    ));
    out.push(format!("{}{}", pad(ind), end_marker("#", &spec.id)));
    out
}

// ------------------------------------------------------------------------ hk

fn hk_step(spec: &HookSpec, ind: usize) -> Vec<String> {
    let p = pad(ind);
    vec![
        format!("{p}[\"{}\"] {{", spec.id),
        format!("{p}    shell = \"sh -c\""),
        format!("{p}    check = {}", pkl_quote(&spec.script())),
        format!("{p}}}"),
    ]
}

fn hk_snippet(spec: &HookSpec) -> String {
    let mut s = vec!["hooks {".to_string(), "    [\"pre-commit\"] {".to_string()];
    s.push("        steps {".to_string());
    s.extend(hk_step(spec, 12));
    s.push("        }".to_string());
    s.push("    }".to_string());
    s.push("}".to_string());
    s.join("\n")
}

fn hk_hooks_block(spec: &HookSpec, ind: usize) -> Vec<String> {
    let p = pad(ind);
    let mut out = vec![
        format!("{p}{}", begin_marker("//", &spec.id)),
        format!("{p}[\"pre-commit\"] {{"),
        format!("{p}    steps {{"),
    ];
    out.extend(hk_step(spec, ind + 8));
    out.push(format!("{p}    }}"));
    out.push(format!("{p}}}"));
    out.push(format!("{p}{}", end_marker("//", &spec.id)));
    out
}

fn add_hk(text: &str, spec: &HookSpec) -> Result<String> {
    let mut lines = to_lines(text);
    let snippet = hk_snippet(spec);
    let Some(hooks) = lines.iter().position(|l| l.trim_end() == "hooks {") else {
        let mut block = vec!["hooks {".to_string()];
        block.extend(hk_hooks_block(spec, 4));
        block.push("}".to_string());
        let at = lines.len();
        splice(&mut lines, at, block);
        return Ok(from_lines(&lines));
    };
    let hooks_end = (hooks + 1..lines.len())
        .find(|&k| lines[k].trim_end() == "}")
        .ok_or_else(|| unsupported("hk.pkl", "the `hooks` block is not closed", &snippet))?;
    let pre_commit =
        (hooks + 1..hooks_end).find(|&k| lines[k].trim_start().starts_with("[\"pre-commit\"] {"));
    let Some(pc) = pre_commit else {
        let ind = lines[hooks + 1..hooks_end]
            .iter()
            .find(|l| !is_blank(l))
            .map_or(4, |l| indent_of(l));
        splice(&mut lines, hooks + 1, hk_hooks_block(spec, ind));
        return Ok(from_lines(&lines));
    };
    let hi = indent_of(&lines[pc]);
    let pc_end = (pc + 1..hooks_end)
        .find(|&k| indent_of(&lines[k]) == hi && lines[k].trim_end().trim_start() == "}")
        .ok_or_else(|| unsupported("hk.pkl", "the pre-commit hook is not closed", &snippet))?;
    let body = hi + 4;
    for k in pc + 1..pc_end {
        let line = &lines[k];
        if indent_of(line) != body {
            continue;
        }
        let t = line.trim();
        if t == "steps {" {
            let mut block = vec![format!("{}{}", pad(body + 4), begin_marker("//", &spec.id))];
            block.extend(hk_step(spec, body + 4));
            block.push(format!("{}{}", pad(body + 4), end_marker("//", &spec.id)));
            splice(&mut lines, k + 1, block);
            return Ok(from_lines(&lines));
        }
        if let Some(name) = t.strip_prefix("steps = ") {
            let name = name.split("//").next().unwrap_or("").trim();
            let plain = !name.is_empty() && name.chars().all(|c| c.is_alphanumeric() || c == '_');
            if plain {
                let p = pad(body);
                lines[k] = format!(
                    "{p}steps = ({name}) {{  {} pahdk:wrap-begin {}",
                    "//", spec.id
                );
                let mut block = hk_step(spec, body + 4);
                block.push(format!("{p}}}  {}", end_marker("//", &spec.id)));
                splice(&mut lines, k + 1, block);
                return Ok(from_lines(&lines));
            }
        }
    }
    Err(unsupported(
        "hk.pkl",
        "the pre-commit hook's `steps` are not a block or a plain name",
        &snippet,
    ))
}

// --------------------------------------------------------------------- husky

fn add_husky(text: &str, spec: &HookSpec) -> String {
    let mut lines = to_lines(text);
    lines.push(begin_marker("#", &spec.id));
    lines.push(format!("echo \"[pahdk] {}: running\" >&2", spec.id));
    lines.push(spec.command.trim().to_string());
    lines.push(end_marker("#", &spec.id));
    from_lines(&lines)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec() -> HookSpec {
        HookSpec {
            id: "adr-index".into(),
            command: "adr index".into(),
            description: None,
        }
    }

    fn repo_with(files: &[(&str, &str)]) -> tempfile::TempDir {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join(".git")).unwrap();
        for (name, body) in files {
            let path = tmp.path().join(name);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, body).unwrap();
        }
        tmp
    }

    const HK: &str = "amends \"package://x/Config.pkl\"\n\nlocal preCommit = new Mapping<String, Step> {\n    [\"rustfmt\"] {\n        check = \"cargo fmt --check\"\n    }\n}\n\nhooks {\n    [\"pre-commit\"] {\n        fix = true\n        steps = preCommit\n    }\n    [\"pre-push\"] {\n        steps = prePush\n    }\n}\n";
    const PRE_COMMIT: &str = "# my hooks\nrepos:\n  - repo: https://example.com/x\n    rev: v1\n    hooks:\n      - id: x\nci:\n  autofix_prs: false\n";
    const LEFTHOOK: &str = "# mine\npre-commit:\n  parallel: true\n  commands:\n    lint:\n      run: make lint\npre-push:\n  commands:\n    t:\n      run: make test\n";
    const HUSKY: &str = "npm test\n";

    fn fixtures() -> Vec<(Manager, &'static str, &'static str)> {
        vec![
            (Manager::Hk, "hk.pkl", HK),
            (Manager::PreCommit, ".pre-commit-config.yaml", PRE_COMMIT),
            (Manager::Lefthook, "lefthook.yml", LEFTHOOK),
            (Manager::Husky, ".husky/pre-commit", HUSKY),
        ]
    }

    fn read(repo: &tempfile::TempDir, name: &str) -> String {
        std::fs::read_to_string(repo.path().join(name)).unwrap()
    }

    #[test]
    fn detects_each_manager() {
        for (manager, name, body) in fixtures() {
            let repo = repo_with(&[(name, body)]);
            assert_eq!(
                detect_manager(repo.path()),
                Detection::One(manager),
                "{name}"
            );
        }
        for alt in ["lefthook.yaml", ".lefthook.yml", ".lefthook.yaml"] {
            let repo = repo_with(&[(alt, "")]);
            assert_eq!(
                detect_manager(repo.path()),
                Detection::One(Manager::Lefthook)
            );
        }
    }

    #[test]
    fn detects_none_and_many() {
        let repo = repo_with(&[]);
        assert_eq!(detect_manager(repo.path()), Detection::None);
        let repo = repo_with(&[("hk.pkl", HK), (".husky/pre-commit", HUSKY)]);
        assert_eq!(
            detect_manager(repo.path()),
            Detection::Many(vec![Manager::Hk, Manager::Husky])
        );
    }

    #[test]
    fn install_adds_hook_keeps_existing_and_runs_banner() {
        for (manager, name, body) in fixtures() {
            let repo = repo_with(&[(name, body)]);
            let out = install_hook(repo.path(), &spec(), &HookOptions::default()).unwrap();
            assert!(matches!(out, HookOutcome::Installed { manager: m, .. } if m == manager));
            let text = read(&repo, name);
            assert!(
                text.contains("[pahdk] adr-index: running"),
                "{name}: {text}"
            );
            assert!(text.contains("adr index"), "{name}");
            assert!(text.contains("pahdk:begin adr-index") || text.contains("pahdk:wrap-begin"));
            // Every original line survives (hk rewrites one `steps` line).
            for line in body.lines().filter(|l| !l.contains("steps = preCommit")) {
                assert!(text.contains(line), "{name} lost {line:?}");
            }
            assert!(!repo.path().join(".git/hooks/pre-commit").exists());
        }
    }

    #[test]
    fn installed_yaml_parses_with_the_hook_in_place() {
        for (name, body) in [
            (".pre-commit-config.yaml", PRE_COMMIT),
            ("lefthook.yml", LEFTHOOK),
        ] {
            let repo = repo_with(&[(name, body)]);
            install_hook(repo.path(), &spec(), &HookOptions::default()).unwrap();
            let doc: serde_yaml_ng::Value = serde_yaml_ng::from_str(&read(&repo, name)).unwrap();
            let flat = serde_yaml_ng::to_string(&doc).unwrap();
            assert!(flat.contains("adr-index"), "{name}");
            if name == "lefthook.yml" {
                assert!(doc["pre-commit"]["commands"]["lint"]["run"].is_string());
                assert!(doc["pre-commit"]["commands"]["adr-index"]["run"].is_string());
                assert!(doc["pre-commit"]["parallel"].as_bool().unwrap());
            } else {
                assert_eq!(doc["repos"].as_sequence().unwrap().len(), 2);
                assert!(doc["ci"]["autofix_prs"].is_bool());
            }
        }
    }

    #[test]
    fn install_twice_changes_nothing() {
        for (_, name, body) in fixtures() {
            let repo = repo_with(&[(name, body)]);
            install_hook(repo.path(), &spec(), &HookOptions::default()).unwrap();
            let first = read(&repo, name);
            let again = install_hook(repo.path(), &spec(), &HookOptions::default()).unwrap();
            assert!(
                matches!(again, HookOutcome::AlreadyInstalled { .. }),
                "{name}"
            );
            assert_eq!(read(&repo, name), first, "{name}");
        }
    }

    #[test]
    fn uninstall_restores_the_original_file() {
        for (_, name, body) in fixtures() {
            let repo = repo_with(&[(name, body)]);
            install_hook(repo.path(), &spec(), &HookOptions::default()).unwrap();
            assert!(uninstall_hook(repo.path(), "adr-index").unwrap(), "{name}");
            assert_eq!(read(&repo, name), body, "{name}");
            assert!(!uninstall_hook(repo.path(), "adr-index").unwrap(), "{name}");
        }
    }

    #[test]
    fn uninstall_leaves_the_users_own_hooks() {
        let mine = "# pahdk:begin other\nkeep me\n# pahdk:end other\n";
        let repo = repo_with(&[(".husky/pre-commit", &format!("{HUSKY}{mine}"))]);
        install_hook(repo.path(), &spec(), &HookOptions::default()).unwrap();
        uninstall_hook(repo.path(), "adr-index").unwrap();
        assert_eq!(read(&repo, ".husky/pre-commit"), format!("{HUSKY}{mine}"));
    }

    #[test]
    fn husky_script_created_and_deleted_when_absent() {
        let repo = repo_with(&[]);
        std::fs::create_dir_all(repo.path().join(".husky")).unwrap();
        install_hook(repo.path(), &spec(), &HookOptions::default()).unwrap();
        assert!(repo.path().join(".husky/pre-commit").is_file());
        uninstall_hook(repo.path(), "adr-index").unwrap();
        assert!(!repo.path().join(".husky/pre-commit").exists());
    }

    #[test]
    fn opt_out_writes_nothing() {
        for (_, name, body) in fixtures() {
            let repo = repo_with(&[(name, body)]);
            let opts = HookOptions {
                opt_out: true,
                manager: None,
            };
            assert_eq!(
                install_hook(repo.path(), &spec(), &opts).unwrap(),
                HookOutcome::OptedOut
            );
            assert_eq!(read(&repo, name), body, "{name}");
            assert!(!repo.path().join(".git/pahdk-hooks.json").exists());
        }
    }

    #[test]
    fn remembered_opt_out_skips_later_installs() {
        let repo = repo_with(&[(".husky/pre-commit", HUSKY)]);
        remember_opt_out(repo.path(), "adr-index").unwrap();
        assert!(is_opted_out(repo.path(), "adr-index"));
        let out = install_hook(repo.path(), &spec(), &HookOptions::default()).unwrap();
        assert_eq!(out, HookOutcome::OptedOut);
        assert_eq!(read(&repo, ".husky/pre-commit"), HUSKY);
    }

    #[test]
    fn no_manager_and_many_managers_ask_instead_of_guessing() {
        let repo = repo_with(&[]);
        assert_eq!(
            install_hook(repo.path(), &spec(), &HookOptions::default()).unwrap(),
            HookOutcome::NoManager
        );
        let repo = repo_with(&[("hk.pkl", HK), (".husky/pre-commit", HUSKY)]);
        let out = install_hook(repo.path(), &spec(), &HookOptions::default()).unwrap();
        assert_eq!(
            out,
            HookOutcome::Ambiguous(vec![Manager::Hk, Manager::Husky])
        );
        assert_eq!(read(&repo, "hk.pkl"), HK);
        // An explicit choice resolves it.
        let opts = HookOptions {
            opt_out: false,
            manager: Some(Manager::Husky),
        };
        let out = install_hook(repo.path(), &spec(), &opts).unwrap();
        assert!(matches!(
            out,
            HookOutcome::Installed {
                manager: Manager::Husky,
                ..
            }
        ));
        assert_eq!(read(&repo, "hk.pkl"), HK);
    }

    #[test]
    fn explicit_manager_creates_a_missing_config() {
        let repo = repo_with(&[]);
        let opts = HookOptions {
            opt_out: false,
            manager: Some(Manager::Lefthook),
        };
        install_hook(repo.path(), &spec(), &opts).unwrap();
        let doc: serde_yaml_ng::Value =
            serde_yaml_ng::from_str(&read(&repo, "lefthook.yml")).unwrap();
        assert!(doc["pre-commit"]["commands"]["adr-index"]["run"].is_string());
    }

    #[test]
    fn hk_without_a_precommit_hook_gets_one() {
        let hk = "amends \"x\"\n\nhooks {\n    [\"pre-push\"] {\n        steps = p\n    }\n}\n";
        let repo = repo_with(&[("hk.pkl", hk)]);
        install_hook(repo.path(), &spec(), &HookOptions::default()).unwrap();
        assert!(read(&repo, "hk.pkl").contains("[\"pre-commit\"] {"));
        uninstall_hook(repo.path(), "adr-index").unwrap();
        assert_eq!(read(&repo, "hk.pkl"), hk);
    }

    #[test]
    fn hk_with_a_steps_block_gets_a_step() {
        let hk = "hooks {\n    [\"pre-commit\"] {\n        steps {\n            [\"a\"] {\n                check = \"true\"\n            }\n        }\n    }\n}\n";
        let repo = repo_with(&[("hk.pkl", hk)]);
        install_hook(repo.path(), &spec(), &HookOptions::default()).unwrap();
        assert!(read(&repo, "hk.pkl").contains("[\"adr-index\"] {"));
        uninstall_hook(repo.path(), "adr-index").unwrap();
        assert_eq!(read(&repo, "hk.pkl"), hk);
    }

    #[test]
    fn hk_with_computed_steps_is_reported_not_guessed() {
        let hk = "hooks {\n    [\"pre-commit\"] {\n        steps = makeSteps(1)\n    }\n}\n";
        let repo = repo_with(&[("hk.pkl", hk)]);
        let err = install_hook(repo.path(), &spec(), &HookOptions::default()).unwrap_err();
        assert!(err.to_string().contains("add this yourself"));
        assert_eq!(read(&repo, "hk.pkl"), hk);
    }

    #[test]
    fn lefthook_without_precommit_or_commands() {
        for body in ["output:\n  - success\n", "pre-commit:\n  parallel: true\n"] {
            let repo = repo_with(&[("lefthook.yml", body)]);
            install_hook(repo.path(), &spec(), &HookOptions::default()).unwrap();
            let doc: serde_yaml_ng::Value =
                serde_yaml_ng::from_str(&read(&repo, "lefthook.yml")).unwrap();
            assert!(doc["pre-commit"]["commands"]["adr-index"]["run"].is_string());
            uninstall_hook(repo.path(), "adr-index").unwrap();
            assert_eq!(read(&repo, "lefthook.yml"), body);
        }
    }

    #[test]
    fn pre_commit_empty_file_and_four_space_indent() {
        let repo = repo_with(&[(".pre-commit-config.yaml", "")]);
        install_hook(repo.path(), &spec(), &HookOptions::default()).unwrap();
        let doc: serde_yaml_ng::Value =
            serde_yaml_ng::from_str(&read(&repo, ".pre-commit-config.yaml")).unwrap();
        assert_eq!(
            doc["repos"][0]["hooks"][0]["id"].as_str(),
            Some("adr-index")
        );

        let four =
            "repos:\n    - repo: https://e.com/x\n      rev: v1\n      hooks:\n          - id: x\n";
        let repo = repo_with(&[(".pre-commit-config.yaml", four)]);
        install_hook(repo.path(), &spec(), &HookOptions::default()).unwrap();
        let doc: serde_yaml_ng::Value =
            serde_yaml_ng::from_str(&read(&repo, ".pre-commit-config.yaml")).unwrap();
        assert_eq!(doc["repos"].as_sequence().unwrap().len(), 2);
    }

    #[test]
    fn spec_rejects_unsafe_values() {
        let mut s = spec();
        s.id = "Bad Id".into();
        assert!(s.validate().is_err());
        let mut s = spec();
        s.command = "a\nb".into();
        assert!(s.validate().is_err());
    }

    #[test]
    fn spec_is_read_from_the_skill_directory() {
        let tmp = tempfile::tempdir().unwrap();
        assert_eq!(HookSpec::from_skill_dir(tmp.path()).unwrap(), None);
        std::fs::write(
            tmp.path().join(HOOK_FILE),
            r#"{"id":"adr-index","command":"adr index","description":"d"}"#,
        )
        .unwrap();
        let got = HookSpec::from_skill_dir(tmp.path()).unwrap().unwrap();
        assert_eq!(got.id, "adr-index");
        std::fs::write(
            tmp.path().join(HOOK_FILE),
            r#"{"id":"x","command":"c","extra":1}"#,
        )
        .unwrap();
        assert!(HookSpec::from_skill_dir(tmp.path()).is_err());
    }

    #[test]
    fn quotes_survive_shell_and_yaml() {
        let mut s = spec();
        s.command = "echo 'a \"b\"' && true".into();
        let repo = repo_with(&[
            (".pre-commit-config.yaml", PRE_COMMIT),
            ("lefthook.yml", ""),
        ]);
        let opts = HookOptions {
            opt_out: false,
            manager: Some(Manager::PreCommit),
        };
        install_hook(repo.path(), &s, &opts).unwrap();
        let doc: serde_yaml_ng::Value =
            serde_yaml_ng::from_str(&read(&repo, ".pre-commit-config.yaml")).unwrap();
        let entry = doc["repos"][1]["hooks"][0]["entry"].as_str().unwrap();
        assert!(entry.starts_with("sh -c '"));
        assert!(entry.contains("a \"b\""));
    }
}
