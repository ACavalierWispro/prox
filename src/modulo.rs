//! Modulo manifests (`modulo.json`): parsing, validation, checksums, installed set,
//! and dependency ordering.

use crate::util::{self, fail, EXIT_GENERAL, EXIT_VERIFY};
use anyhow::{anyhow, Context, Result};
use colored::Colorize;
use semver::{Version, VersionReq};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::fs;
use std::path::{Component, Path, PathBuf};

pub const MANIFEST: &str = "modulo.json";

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ModuloKind {
    Language,
    Library,
    Tool,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Provides {
    #[serde(default)]
    pub languages: Vec<String>,
    #[serde(default)]
    pub libs: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Dependency {
    pub name: String,
    #[serde(default)]
    pub version: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum MergeMode {
    /// Keep the existing file untouched (default).
    #[default]
    Skip,
    /// Append the template text unless it is already present.
    Append,
    /// Deep-merge JSON documents.
    Json,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct FileEntry {
    pub src: String,
    pub dest: String,
    #[serde(default)]
    pub merge: MergeMode,
}

/// Command lists that prox turns into `build.sh` / `build.bat`.
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct BuildSpec {
    #[serde(default)]
    pub requires: Vec<String>,
    #[serde(default)]
    pub requires_bat: Option<Vec<String>>,
    #[serde(default)]
    pub problem_matcher: Option<String>,
    #[serde(default)]
    pub build_sh: Vec<String>,
    #[serde(default)]
    pub build_bat: Vec<String>,
    #[serde(default)]
    pub run_sh: Vec<String>,
    #[serde(default)]
    pub run_bat: Vec<String>,
    #[serde(default)]
    pub clean_sh: Vec<String>,
    #[serde(default)]
    pub clean_bat: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Modulo {
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub description: String,
    #[serde(rename = "type")]
    pub kind: ModuloKind,
    #[serde(default)]
    pub aliases: Vec<String>,
    #[serde(default)]
    pub provides: Provides,
    #[serde(default)]
    pub dependencies: Vec<Dependency>,
    #[serde(default)]
    pub dirs: Vec<String>,
    #[serde(default)]
    pub files: Vec<FileEntry>,
    #[serde(default)]
    pub build: Option<BuildSpec>,
    #[serde(default)]
    pub checksum: Option<String>,
    #[serde(default)]
    pub min_prox_version: Option<String>,
}

/// A modulo available in the GitHub repo (manifest + the raw text it came from).
#[derive(Clone, Debug)]
pub struct Remote {
    pub modulo: Modulo,
    pub raw: String,
}

/// A modulo installed on disk.
#[derive(Clone, Debug)]
pub struct Installed {
    pub modulo: Modulo,
    pub raw: String,
    pub dir: PathBuf,
}

// ------------------------------------------------------------------ validation

/// Relative, forward-slash path that stays inside its root.
pub fn safe_rel(p: &str) -> bool {
    if p.is_empty() || p.contains('\\') || p.contains('\0') {
        return false;
    }
    Path::new(p)
        .components()
        .all(|c| matches!(c, Component::Normal(_)))
}

pub fn valid_modulo_name(n: &str) -> bool {
    !n.is_empty()
        && n.len() <= 40
        && n.chars()
            .next()
            .is_some_and(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        && n.chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
}

pub fn parse_version(s: &str) -> Result<Version> {
    Version::parse(s.trim().trim_start_matches('v'))
        .with_context(|| format!("'{s}' is not a valid semantic version"))
}

impl Modulo {
    pub fn parse(raw: &str) -> Result<Modulo> {
        let m: Modulo =
            serde_json::from_str(raw).map_err(|e| anyhow!("invalid {MANIFEST}: {e}"))?;
        m.validate()?;
        Ok(m)
    }

    pub fn validate(&self) -> Result<()> {
        if !valid_modulo_name(&self.name) {
            return Err(anyhow!(
                "invalid modulo name '{}' (use lowercase letters, digits, '-' and '_')",
                self.name
            ));
        }
        parse_version(&self.version)?;
        if let Some(v) = &self.min_prox_version {
            parse_version(v)?;
        }
        for d in &self.dependencies {
            if !valid_modulo_name(&d.name) {
                return Err(anyhow!("invalid dependency name '{}'", d.name));
            }
            if let Some(req) = &d.version {
                VersionReq::parse(req)
                    .with_context(|| format!("bad version requirement '{req}' for '{}'", d.name))?;
            }
        }
        for d in &self.dirs {
            if !safe_rel(d) {
                return Err(anyhow!("unsafe directory path '{d}'"));
            }
        }
        for f in &self.files {
            if !safe_rel(&f.src) {
                return Err(anyhow!("unsafe template path '{}'", f.src));
            }
            if !safe_rel(&f.dest) {
                return Err(anyhow!(
                    "unsafe destination path '{}' (absolute paths and '..' are not allowed)",
                    f.dest
                ));
            }
        }
        Ok(())
    }

    pub fn semver(&self) -> Version {
        parse_version(&self.version).expect("validated")
    }

    /// Does this modulo answer to `token` (its name, an alias, or a language it provides)?
    pub fn matches(&self, token: &str) -> bool {
        let t = token.to_lowercase();
        self.name == t
            || self.aliases.iter().any(|a| a.to_lowercase() == t)
            || self
                .provides
                .languages
                .iter()
                .any(|l| l.to_lowercase() == t)
    }
}

// ------------------------------------------------------------------ checksum

fn canonical(v: &Value, out: &mut String) {
    match v {
        Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            out.push('{');
            for (i, k) in keys.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                out.push_str(&serde_json::to_string(k).unwrap());
                out.push(':');
                canonical(&map[*k], out);
            }
            out.push('}');
        }
        Value::Array(a) => {
            out.push('[');
            for (i, x) in a.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                canonical(x, out);
            }
            out.push(']');
        }
        other => out.push_str(&serde_json::to_string(other).unwrap()),
    }
}

/// The manifest with its own `checksum` field removed and keys sorted, so the hash
/// does not depend on formatting or key order.
pub fn canonical_manifest(raw: &str) -> Result<String> {
    let mut v: Value = serde_json::from_str(raw)?;
    if let Value::Object(map) = &mut v {
        map.remove("checksum");
    }
    let mut out = String::new();
    canonical(&v, &mut out);
    Ok(out)
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

/// `sha256:<hex>` over the sorted lines `"<dest>\0<sha256(file)>\n"` for every template
/// file, plus one `modulo.json` line holding the hash of the canonical manifest.
pub fn compute_checksum(raw: &str, dir: &Path, m: &Modulo) -> Result<String> {
    let mut lines: Vec<(String, String)> = Vec::new();
    for f in &m.files {
        let path = dir.join(&f.src);
        let bytes = fs::read(&path).with_context(|| format!("cannot read {}", path.display()))?;
        lines.push((f.dest.clone(), sha256_hex(&bytes)));
    }
    lines.push((
        MANIFEST.to_string(),
        sha256_hex(canonical_manifest(raw)?.as_bytes()),
    ));
    lines.sort();
    let mut h = Sha256::new();
    for (dest, hash) in &lines {
        h.update(dest.as_bytes());
        h.update([0u8]);
        h.update(hash.as_bytes());
        h.update(b"\n");
    }
    Ok(format!("sha256:{}", hex::encode(h.finalize())))
}

#[derive(Debug, PartialEq, Eq)]
pub enum Verify {
    Ok,
    NoChecksum,
    Mismatch { expected: String, actual: String },
}

pub fn verify(raw: &str, dir: &Path, m: &Modulo) -> Result<Verify> {
    let Some(expected) = &m.checksum else {
        return Ok(Verify::NoChecksum);
    };
    let actual = compute_checksum(raw, dir, m)?;
    Ok(if expected.eq_ignore_ascii_case(&actual) {
        Verify::Ok
    } else {
        Verify::Mismatch {
            expected: expected.clone(),
            actual,
        }
    })
}

// ------------------------------------------------------------------ installed set

pub fn load_from_dir(dir: &Path) -> Result<Installed> {
    let path = dir.join(MANIFEST);
    let raw =
        fs::read_to_string(&path).with_context(|| format!("cannot read {}", path.display()))?;
    let modulo = Modulo::parse(&raw)?;
    Ok(Installed {
        modulo,
        raw,
        dir: dir.to_path_buf(),
    })
}

/// Every installed modulo directory with its load result (for `--doctor`).
pub fn scan_installed() -> Result<Vec<(String, Result<Installed>)>> {
    let root = util::modulos_dir()?;
    let mut out = Vec::new();
    if !root.is_dir() {
        return Ok(out);
    }
    let mut names: Vec<String> = fs::read_dir(&root)?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| !n.starts_with('.'))
        .collect();
    names.sort();
    for n in names {
        let res = load_from_dir(&root.join(&n));
        out.push((n, res));
    }
    Ok(out)
}

/// All valid installed modulos (invalid ones are reported as warnings and skipped).
pub fn list_installed() -> Result<Vec<Installed>> {
    let mut v = Vec::new();
    for (name, res) in scan_installed()? {
        match res {
            Ok(i) => v.push(i),
            Err(e) => eprintln!(
                "{} skipping invalid modulo '{name}': {e:#}",
                "warning:".yellow().bold()
            ),
        }
    }
    Ok(v)
}

// ------------------------------------------------------------------ dependency ordering

/// Order `names` (plus their dependencies) so dependencies come first,
/// using only what is installed locally.
pub fn order_with_deps(names: &[String], installed: &[Installed]) -> Result<Vec<Installed>> {
    fn visit(
        name: &str,
        installed: &[Installed],
        done: &mut HashSet<String>,
        stack: &mut Vec<String>,
        out: &mut Vec<Installed>,
        needed_by: Option<&str>,
    ) -> Result<()> {
        if done.contains(name) {
            return Ok(());
        }
        if stack.iter().any(|s| s == name) {
            return Err(fail(
                EXIT_GENERAL,
                format!("dependency cycle: {} -> {name}", stack.join(" -> ")),
                "break the cycle in the modulos' dependencies",
            ));
        }
        let Some(inst) = installed.iter().find(|i| i.modulo.name == name) else {
            let msg = match needed_by {
                Some(b) => format!("modulo '{b}' needs '{name}', which is not installed"),
                None => format!("modulo '{name}' is not installed"),
            };
            return Err(fail(
                EXIT_GENERAL,
                msg,
                format!("install it with `prox --modulos download {name}`"),
            ));
        };
        stack.push(name.to_string());
        for d in &inst.modulo.dependencies {
            visit(&d.name, installed, done, stack, out, Some(name))?;
            if let Some(req) = &d.version {
                let dep = installed.iter().find(|i| i.modulo.name == d.name).unwrap();
                if !VersionReq::parse(req)?.matches(&dep.modulo.semver()) {
                    return Err(fail(
                        EXIT_GENERAL,
                        format!(
                            "modulo '{name}' needs '{}' {req}, but {} is installed",
                            d.name, dep.modulo.version
                        ),
                        format!("run `prox --modulos update {}`", d.name),
                    ));
                }
            }
        }
        stack.pop();
        done.insert(name.to_string());
        out.push(inst.clone());
        Ok(())
    }

    let mut done = HashSet::new();
    let mut out = Vec::new();
    for n in names {
        visit(n, installed, &mut done, &mut Vec::new(), &mut out, None)?;
    }
    Ok(out)
}

/// Work out what to install (targets + missing/outdated dependencies), dependencies first.
pub fn plan_install(
    targets: &[String],
    remote: &[Remote],
    installed: &[Installed],
) -> Result<Vec<Remote>> {
    fn visit(
        name: &str,
        remote: &[Remote],
        installed: &[Installed],
        done: &mut HashSet<String>,
        stack: &mut Vec<String>,
        out: &mut Vec<Remote>,
    ) -> Result<()> {
        if done.contains(name) {
            return Ok(());
        }
        if stack.iter().any(|s| s == name) {
            return Err(fail(
                EXIT_GENERAL,
                format!("dependency cycle: {} -> {name}", stack.join(" -> ")),
                "break the cycle in the modulos' dependencies",
            ));
        }
        let Some(r) = remote.iter().find(|r| r.modulo.name == name) else {
            let known: Vec<&str> = remote.iter().map(|r| r.modulo.name.as_str()).collect();
            return Err(fail(
                EXIT_GENERAL,
                format!("modulo '{name}' was not found in the repository"),
                format!("available: {}", known.join(", ")),
            ));
        };
        stack.push(name.to_string());
        for d in &r.modulo.dependencies {
            let req = d.version.as_deref().map(VersionReq::parse).transpose()?;
            let have = installed.iter().find(|i| i.modulo.name == d.name);
            let satisfied =
                have.is_some_and(|i| req.as_ref().is_none_or(|q| q.matches(&i.modulo.semver())));
            if !satisfied {
                if let (Some(q), Some(rd)) = (&req, remote.iter().find(|x| x.modulo.name == d.name))
                {
                    if !q.matches(&rd.modulo.semver()) {
                        return Err(fail(
                            EXIT_GENERAL,
                            format!(
                                "'{name}' needs '{}' {}, but the repository only has {}",
                                d.name,
                                d.version.as_deref().unwrap_or("*"),
                                rd.modulo.version
                            ),
                            "ask the modulo author to update their dependencies",
                        ));
                    }
                }
                visit(&d.name, remote, installed, done, stack, out)?;
            }
        }
        stack.pop();
        done.insert(name.to_string());
        out.push(r.clone());
        Ok(())
    }

    let mut done = HashSet::new();
    let mut out = Vec::new();
    for t in targets {
        visit(t, remote, installed, &mut done, &mut Vec::new(), &mut out)?;
    }
    Ok(out)
}

// ------------------------------------------------------------------ install

/// Install a modulo's files from `fetch` into the modulos dir, atomically:
/// stage in a temp dir, verify the checksum, then rename into place.
pub fn install_with<F>(r: &Remote, mut fetch: F) -> Result<()>
where
    F: FnMut(&str) -> Result<Vec<u8>>,
{
    if let Some(min) = &r.modulo.min_prox_version {
        if parse_version(min)? > parse_version(env!("CARGO_PKG_VERSION"))? {
            return Err(fail(
                EXIT_GENERAL,
                format!(
                    "modulo '{}' needs prox >= {min} (you have {})",
                    r.modulo.name,
                    env!("CARGO_PKG_VERSION")
                ),
                "run `prox --update` first",
            ));
        }
    }
    let root = util::modulos_dir()?;
    fs::create_dir_all(&root)?;
    let name = &r.modulo.name;
    let stage = root.join(format!(".tmp-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&stage);
    fs::create_dir_all(&stage)?;

    let result = (|| -> Result<()> {
        fs::write(stage.join(MANIFEST), &r.raw)?;
        for f in &r.modulo.files {
            let bytes = fetch(&f.src)?;
            if bytes.len() > 5 * 1024 * 1024 {
                return Err(anyhow!("template '{}' is larger than 5 MB", f.src));
            }
            let target = stage.join(&f.src);
            if let Some(p) = target.parent() {
                fs::create_dir_all(p)?;
            }
            fs::write(&target, bytes)?;
        }
        match verify(&r.raw, &stage, &r.modulo)? {
            Verify::Ok => {}
            Verify::NoChecksum => eprintln!(
                "{} modulo '{name}' has no checksum; contents are unverified",
                "warning:".yellow().bold()
            ),
            Verify::Mismatch { expected, actual } => {
                return Err(fail(
                    EXIT_VERIFY,
                    format!("checksum mismatch for modulo '{name}'"),
                    format!("expected {expected}, got {actual}. The download may be corrupted or tampered with; nothing was installed"),
                ));
            }
        }
        Ok(())
    })();

    if let Err(e) = result {
        let _ = fs::remove_dir_all(&stage);
        return Err(e);
    }

    // Swap in. Keep the old version until the new one is in place.
    let live = root.join(name);
    let backup = root.join(format!(".old-{name}-{}", std::process::id()));
    let had_old = live.exists();
    if had_old {
        fs::rename(&live, &backup).map_err(|e| {
            let _ = fs::remove_dir_all(&stage);
            anyhow!("cannot move the old version of '{name}' aside: {e}")
        })?;
    }
    if let Err(e) = fs::rename(&stage, &live) {
        if had_old {
            let _ = fs::rename(&backup, &live);
        }
        let _ = fs::remove_dir_all(&stage);
        return Err(anyhow!("cannot install '{name}': {e}"));
    }
    if had_old {
        let _ = fs::remove_dir_all(&backup);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"{
        "name": "demo", "version": "1.2.3", "type": "language",
        "provides": {"languages": ["demo"], "libs": ["x"]},
        "files": [{"src": "templates/a.txt", "dest": "a.txt"}]
    }"#;

    #[test]
    fn parses_sample() {
        let m = Modulo::parse(SAMPLE).unwrap();
        assert_eq!(m.name, "demo");
        assert_eq!(m.kind, ModuloKind::Language);
        assert_eq!(m.files[0].merge, MergeMode::Skip);
    }

    #[test]
    fn rejects_bad_manifests() {
        let bad_version = SAMPLE.replace("1.2.3", "one");
        assert!(Modulo::parse(&bad_version).is_err());
        let bad_dest = SAMPLE.replace("\"a.txt\"", "\"../escape\"");
        assert!(Modulo::parse(&bad_dest).is_err());
        let abs = SAMPLE.replace("\"a.txt\"", "\"/etc/passwd\"");
        assert!(Modulo::parse(&abs).is_err());
        let bad_name = SAMPLE.replace("\"demo\",", "\"Bad Name\",");
        assert!(Modulo::parse(&bad_name).is_err());
    }

    #[test]
    fn safe_paths() {
        assert!(safe_rel("src/main.rs"));
        assert!(safe_rel(".vscode/settings.json"));
        assert!(!safe_rel("../x"));
        assert!(!safe_rel("a/../b"));
        assert!(!safe_rel("/abs"));
        assert!(!safe_rel(""));
        assert!(!safe_rel("a\\b"));
    }

    #[test]
    fn alias_matching() {
        let mut m = Modulo::parse(SAMPLE).unwrap();
        m.aliases = vec!["Dm".into()];
        assert!(m.matches("demo") && m.matches("DM") && !m.matches("other"));
    }

    #[test]
    fn checksum_ignores_formatting_and_own_field() {
        let a = canonical_manifest(SAMPLE).unwrap();
        let reordered = r#"{"type":"language","files":[{"dest":"a.txt","src":"templates/a.txt"}],
            "provides":{"libs":["x"],"languages":["demo"]},"version":"1.2.3","name":"demo",
            "checksum":"sha256:abc"}"#;
        assert_eq!(a, canonical_manifest(reordered).unwrap());
    }

    #[test]
    fn checksum_detects_tampering() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join("templates")).unwrap();
        fs::write(dir.path().join("templates/a.txt"), "hello").unwrap();
        let m = Modulo::parse(SAMPLE).unwrap();
        let sum = compute_checksum(SAMPLE, dir.path(), &m).unwrap();
        let signed = SAMPLE.replacen('{', &format!("{{\"checksum\":\"{sum}\","), 1);
        let m2 = Modulo::parse(&signed).unwrap();
        assert_eq!(verify(&signed, dir.path(), &m2).unwrap(), Verify::Ok);
        fs::write(dir.path().join("templates/a.txt"), "evil").unwrap();
        assert!(matches!(
            verify(&signed, dir.path(), &m2).unwrap(),
            Verify::Mismatch { .. }
        ));
    }

    fn remote(name: &str, version: &str, deps: &[(&str, &str)]) -> Remote {
        let deps_json: Vec<String> = deps
            .iter()
            .map(|(n, v)| format!(r#"{{"name":"{n}","version":"{v}"}}"#))
            .collect();
        let raw = format!(
            r#"{{"name":"{name}","version":"{version}","type":"tool","dependencies":[{}]}}"#,
            deps_json.join(",")
        );
        Remote {
            modulo: Modulo::parse(&raw).unwrap(),
            raw,
        }
    }

    #[test]
    fn plan_orders_dependencies_first() {
        let remote_set = vec![
            remote("app", "1.0.0", &[("base", ">=1.0.0")]),
            remote("base", "1.0.0", &[]),
        ];
        let plan = plan_install(&["app".into()], &remote_set, &[]).unwrap();
        let names: Vec<&str> = plan.iter().map(|r| r.modulo.name.as_str()).collect();
        assert_eq!(names, vec!["base", "app"]);
    }

    #[test]
    fn plan_detects_cycles() {
        let remote_set = vec![
            remote("a", "1.0.0", &[("b", "*")]),
            remote("b", "1.0.0", &[("a", "*")]),
        ];
        assert!(plan_install(&["a".into()], &remote_set, &[]).is_err());
    }

    #[test]
    fn plan_unknown_modulo_errors() {
        assert!(plan_install(&["nope".into()], &[], &[]).is_err());
    }
}
