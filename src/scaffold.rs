//! Turning modulo templates into project files.
//!
//! Only *templates* are processed. Nothing inside a modulo is ever executed.

use crate::buildgen;
use crate::modulo::{FileEntry, Installed, MergeMode};
use anyhow::{anyhow, Context, Result};
use serde_json::Value;
use std::fs;
use std::path::Path;

pub struct Ctx {
    pub name: String,
    pub description: String,
    pub author: String,
    pub libs: Vec<String>,
}

#[derive(Default, Debug)]
pub struct Report {
    pub written: Vec<String>,
    pub merged: Vec<String>,
    pub skipped: Vec<String>,
    pub warnings: Vec<String>,
}

// ------------------------------------------------------------------ rendering

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Esc {
    None,
    Json,
    Xml,
}

/// Pick the escaping for user-provided values based on the destination file type.
pub fn esc_for(dest: &str) -> Esc {
    let file = dest.rsplit('/').next().unwrap_or(dest);
    let lower = file.to_lowercase();
    if [".json", ".toml", ".yml", ".yaml"]
        .iter()
        .any(|e| lower.ends_with(e))
    {
        Esc::Json
    } else if [".xml", ".html", ".htm", ".svg"]
        .iter()
        .any(|e| lower.ends_with(e))
        || lower == ".project"
        || lower == ".classpath"
    {
        Esc::Xml
    } else {
        Esc::None
    }
}

fn escape(s: &str, e: Esc) -> String {
    match e {
        Esc::None => s.to_string(),
        Esc::Json => {
            let q = serde_json::to_string(s).unwrap_or_default();
            q[1..q.len().saturating_sub(1)].to_string()
        }
        Esc::Xml => s
            .replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
            .replace('"', "&quot;")
            .replace('\'', "&apos;"),
    }
}

/// A name that is valid as a Cargo/npm style package identifier.
pub fn crate_name(name: &str) -> String {
    let mut s: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    if s.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        s.insert_str(0, "p_");
    }
    s
}

/// Replace `{{placeholders}}`.
///
/// `name`, `description`, `author`, `year`, `crate_name`, `libs`, `libs_link`, `libs_lines`,
/// `libs_cargo`, `libs_json_obj`, `libs_json_arr`.
pub fn render(tpl: &str, ctx: &Ctx, esc: Esc) -> String {
    let libs = &ctx.libs;
    let vars: Vec<(&str, String)> = vec![
        ("name", escape(&ctx.name, esc)),
        ("crate_name", escape(&crate_name(&ctx.name), esc)),
        ("description", escape(&ctx.description, esc)),
        ("author", escape(&ctx.author, esc)),
        ("year", chrono::Utc::now().format("%Y").to_string()),
        ("libs", libs.join(" ")),
        (
            "libs_link",
            libs.iter()
                .map(|l| format!("-l{l}"))
                .collect::<Vec<_>>()
                .join(" "),
        ),
        ("libs_lines", libs.join("\n")),
        (
            "libs_cargo",
            libs.iter()
                .map(|l| format!("{l} = \"*\""))
                .collect::<Vec<_>>()
                .join("\n"),
        ),
        (
            "libs_json_obj",
            libs.iter()
                .map(|l| format!("\"{l}\": \"*\""))
                .collect::<Vec<_>>()
                .join(", "),
        ),
        (
            "libs_json_arr",
            libs.iter()
                .map(|l| format!("\"{l}\""))
                .collect::<Vec<_>>()
                .join(", "),
        ),
    ];
    let mut out = tpl.to_string();
    for (k, v) in vars {
        out = out.replace(&format!("{{{{{k}}}}}"), &v);
    }
    out
}

// ------------------------------------------------------------------ JSON merge

/// Deep merge `b` into `a`. Existing scalars win; objects merge recursively; arrays are
/// unioned (objects with the same `name`/`label` are merged instead of duplicated).
pub fn deep_merge(a: &mut Value, b: &Value) {
    match (a, b) {
        (Value::Object(ao), Value::Object(bo)) => {
            for (k, bv) in bo {
                match ao.get_mut(k) {
                    Some(av) => deep_merge(av, bv),
                    None => {
                        ao.insert(k.clone(), bv.clone());
                    }
                }
            }
        }
        (Value::Array(aa), Value::Array(ba)) => {
            for item in ba {
                let key = |v: &Value| {
                    v.get("name")
                        .or_else(|| v.get("label"))
                        .and_then(Value::as_str)
                        .map(str::to_string)
                };
                if let Some(k) = key(item) {
                    if let Some(existing) = aa.iter_mut().find(|x| key(x).as_deref() == Some(&k)) {
                        deep_merge(existing, item);
                        continue;
                    }
                }
                if !aa.contains(item) {
                    aa.push(item.clone());
                }
            }
        }
        _ => {}
    }
}

fn pretty(v: &Value) -> String {
    let mut s = serde_json::to_string_pretty(v).unwrap_or_default();
    s.push('\n');
    s
}

// ------------------------------------------------------------------ writing

/// Write `text` to `dest` honouring the merge mode.
pub fn write_text(
    dest: &Path,
    rel: &str,
    text: &str,
    mode: MergeMode,
    rep: &mut Report,
) -> Result<()> {
    if let Some(p) = dest.parent() {
        fs::create_dir_all(p).with_context(|| format!("cannot create {}", p.display()))?;
    }
    if !dest.exists() {
        let out = if mode == MergeMode::Json {
            let v: Value = serde_json::from_str(text).map_err(|e| {
                anyhow!("template for {rel} is not valid JSON after rendering: {e}")
            })?;
            pretty(&v)
        } else {
            text.to_string()
        };
        fs::write(dest, out).with_context(|| format!("cannot write {}", dest.display()))?;
        rep.written.push(rel.to_string());
        return Ok(());
    }
    match mode {
        MergeMode::Skip => rep.skipped.push(rel.to_string()),
        MergeMode::Append => {
            let existing = fs::read_to_string(dest)?;
            if existing.contains(text.trim()) {
                rep.skipped.push(rel.to_string());
            } else {
                let mut out = existing;
                if !out.ends_with('\n') {
                    out.push('\n');
                }
                out.push('\n');
                out.push_str(text);
                if !out.ends_with('\n') {
                    out.push('\n');
                }
                fs::write(dest, out)?;
                rep.merged.push(rel.to_string());
            }
        }
        MergeMode::Json => {
            let existing = fs::read_to_string(dest)?;
            let parsed = serde_json::from_str::<Value>(&existing);
            let incoming = serde_json::from_str::<Value>(text);
            match (parsed, incoming) {
                (Ok(mut a), Ok(b)) => {
                    deep_merge(&mut a, &b);
                    fs::write(dest, pretty(&a))?;
                    rep.merged.push(rel.to_string());
                }
                _ => rep.warnings.push(format!(
                    "{rel} exists and is not plain JSON; left unchanged (add the settings manually)"
                )),
            }
        }
    }
    Ok(())
}

fn apply_file(
    project: &Path,
    modulo_dir: &Path,
    f: &FileEntry,
    ctx: &Ctx,
    rep: &mut Report,
) -> Result<()> {
    let src = modulo_dir.join(&f.src);
    let meta = fs::symlink_metadata(&src)
        .with_context(|| format!("template {} is missing", src.display()))?;
    if meta.file_type().is_symlink() {
        return Err(anyhow!(
            "template '{}' is a symlink; refusing to follow it",
            f.src
        ));
    }
    let bytes = fs::read(&src)?;
    let dest = project.join(&f.dest);
    match String::from_utf8(bytes) {
        Ok(text) => {
            let rendered = render(&text, ctx, esc_for(&f.dest));
            write_text(&dest, &f.dest, &rendered, f.merge, rep)
        }
        Err(e) => {
            if !dest.exists() {
                if let Some(p) = dest.parent() {
                    fs::create_dir_all(p)?;
                }
                fs::write(&dest, e.into_bytes())?;
                rep.written.push(f.dest.clone());
            } else {
                rep.skipped.push(f.dest.clone());
            }
            Ok(())
        }
    }
}

const EDITORCONFIG: &str = "root = true

[*]
charset = utf-8
end_of_line = lf
insert_final_newline = true
trim_trailing_whitespace = true

[*.{c,h,cpp,cc,hpp,java,py,rs}]
indent_style = space
indent_size = 4

[*.{html,css,js,json,yml,yaml}]
indent_style = space
indent_size = 2

[*.bat]
end_of_line = crlf

[Makefile]
indent_style = tab
";

const NVIM_BASE: &str =
    "-- prox: project-local Neovim config (loaded when 'exrc' is enabled; run :trust once)
vim.opt.makeprg = \"./build.sh\"
";

/// Scaffold a whole project: base files, each modulo's templates (already in dependency
/// order), then `build.sh`/`build.bat` and the editor task files.
pub fn scaffold(project: &Path, mods: &[Installed], ctx: &Ctx) -> Result<Report> {
    let mut rep = Report::default();

    write_text(
        &project.join(".editorconfig"),
        ".editorconfig",
        EDITORCONFIG,
        MergeMode::Skip,
        &mut rep,
    )?;
    write_text(
        &project.join(".nvim.lua"),
        ".nvim.lua",
        NVIM_BASE,
        MergeMode::Append,
        &mut rep,
    )?;
    let readme = format!(
        "# {}\n\n{}\n\n## Build\n\n```sh\n./build.sh          # build\n./build.sh run      # build and run\n./build.sh clean    # remove build output\n```\n\nOn Windows use `build.bat` instead.\n\nThird-party libraries go in `libs/` (see `libs/README.md` where present).\n",
        ctx.name,
        if ctx.description.is_empty() { "A prox project." } else { &ctx.description }
    );
    write_text(
        &project.join("README.md"),
        "README.md",
        &readme,
        MergeMode::Skip,
        &mut rep,
    )?;

    for m in mods {
        for d in &m.modulo.dirs {
            fs::create_dir_all(project.join(d))?;
        }
        for f in &m.modulo.files {
            apply_file(project, &m.dir, f, ctx, &mut rep)
                .with_context(|| format!("while applying modulo '{}'", m.modulo.name))?;
        }
    }

    buildgen::generate(project, mods, ctx, &mut rep)?;
    Ok(rep)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn ctx() -> Ctx {
        Ctx {
            name: "demo".into(),
            description: "He said \"hi\" & <left>".into(),
            author: "Ann".into(),
            libs: vec!["m".into(), "pthread".into()],
        }
    }

    #[test]
    fn renders_placeholders() {
        let c = ctx();
        assert_eq!(render("{{name}}/{{author}}", &c, Esc::None), "demo/Ann");
        assert_eq!(render("{{libs_link}}", &c, Esc::None), "-lm -lpthread");
        assert_eq!(
            render("{{libs_cargo}}", &c, Esc::None),
            "m = \"*\"\npthread = \"*\""
        );
    }

    #[test]
    fn escapes_by_file_type() {
        let c = ctx();
        let json = render("\"{{description}}\"", &c, Esc::Json);
        assert!(serde_json::from_str::<String>(&json).is_ok());
        let xml = render("{{description}}", &c, Esc::Xml);
        assert!(xml.contains("&amp;") && xml.contains("&lt;left&gt;"));
        assert_eq!(esc_for("package.json"), Esc::Json);
        assert_eq!(esc_for("src/index.html"), Esc::Xml);
        assert_eq!(esc_for(".classpath"), Esc::Xml);
        assert_eq!(esc_for("src/main.c"), Esc::None);
    }

    #[test]
    fn crate_names() {
        assert_eq!(crate_name("my.app"), "my_app");
        assert_eq!(crate_name("1st"), "p_1st");
        assert_eq!(crate_name("ok-name"), "ok-name");
    }

    #[test]
    fn merges_json() {
        let mut a = json!({"a": 1, "list": [1, 2], "cfg": [{"name": "x", "p": [1]}]});
        let b = json!({"a": 9, "b": 2, "list": [2, 3], "cfg": [{"name": "x", "p": [2], "q": 1}]});
        deep_merge(&mut a, &b);
        assert_eq!(
            a,
            json!({"a": 1, "b": 2, "list": [1, 2, 3], "cfg": [{"name": "x", "p": [1, 2], "q": 1}]})
        );
    }

    #[test]
    fn append_is_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("x.txt");
        let mut rep = Report::default();
        write_text(&f, "x.txt", "-Iinclude\n", MergeMode::Append, &mut rep).unwrap();
        write_text(&f, "x.txt", "-Iinclude\n", MergeMode::Append, &mut rep).unwrap();
        write_text(&f, "x.txt", "-Wall\n", MergeMode::Append, &mut rep).unwrap();
        let text = fs::read_to_string(&f).unwrap();
        assert_eq!(text.matches("-Iinclude").count(), 1);
        assert!(text.contains("-Wall"));
    }

    #[test]
    fn invalid_existing_json_is_not_clobbered() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("s.json");
        fs::write(&f, "// comment\n{}").unwrap();
        let mut rep = Report::default();
        write_text(&f, "s.json", "{\"a\":1}", MergeMode::Json, &mut rep).unwrap();
        assert_eq!(fs::read_to_string(&f).unwrap(), "// comment\n{}");
        assert_eq!(rep.warnings.len(), 1);
    }
}
