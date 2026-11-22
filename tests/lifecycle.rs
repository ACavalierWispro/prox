//! create -> list -> edit -> delete, fully offline, in a temp HOME.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

fn copy_dir(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for e in fs::read_dir(from).unwrap() {
        let e = e.unwrap();
        let dest = to.join(e.file_name());
        if e.path().is_dir() {
            copy_dir(&e.path(), &dest);
        } else {
            fs::copy(e.path(), dest).unwrap();
        }
    }
}

struct Env {
    home: tempfile::TempDir,
}

impl Env {
    fn new() -> Env {
        let home = tempfile::tempdir().unwrap();
        let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("modulos");
        for m in ["c", "python"] {
            copy_dir(
                &repo.join(m),
                &home.path().join("data/prox/modulos").join(m),
            );
        }
        Env { home }
    }

    fn prox(&self, args: &[&str]) -> Output {
        let h = self.home.path();
        Command::new(env!("CARGO_BIN_EXE_prox"))
            .args(args)
            .env("HOME", h)
            .env("XDG_CONFIG_HOME", h.join("cfg"))
            .env("XDG_DATA_HOME", h.join("data"))
            .env("XDG_CACHE_HOME", h.join("cache"))
            .env("NO_COLOR", "1")
            .env("EDITOR", "true")
            .stdin(Stdio::null())
            .output()
            .unwrap()
    }

    fn ok(&self, args: &[&str]) -> String {
        let o = self.prox(args);
        assert!(
            o.status.success(),
            "prox {args:?} failed: {}",
            String::from_utf8_lossy(&o.stderr)
        );
        String::from_utf8_lossy(&o.stdout).into_owned()
    }

    fn project(&self, name: &str) -> PathBuf {
        self.home.path().join("prox-projects").join(name)
    }
}

#[test]
fn create_list_edit_delete() {
    let env = Env::new();

    // create (spelled the single-dash way)
    env.ok(&[
        "-create", "demo", "--lang", "c,python", "--libs", "m", "-d", "First", "-a", "Tester",
        "--no-git",
    ]);
    let dir = env.project("demo");
    for f in [
        "prox.json",
        "build.sh",
        "build.bat",
        "src/main.c",
        "src/main.py",
        "compile_flags.txt",
        ".vscode/settings.json",
        ".vscode/tasks.json",
        ".zed/settings.json",
        ".zed/tasks.json",
        ".nvim.lua",
        "pyrightconfig.json",
        "libs/include",
    ] {
        assert!(dir.join(f).exists(), "missing {f}");
    }
    let build = fs::read_to_string(dir.join("build.sh")).unwrap();
    assert!(build.contains("-lm"), "libs must be linked");

    // refuses to overwrite / bad names
    assert!(!env.prox(&["create", "demo", "--no-git"]).status.success());
    assert_eq!(env.prox(&["create", "bad name"]).status.code(), Some(2));
    assert_eq!(
        env.prox(&["create", "x", "--lang", "cobol"]).status.code(),
        Some(2)
    );

    // list
    assert!(env.ok(&["-list"]).contains("demo"));
    let json: serde_json::Value = serde_json::from_str(&env.ok(&["list", "--json"])).unwrap();
    assert_eq!(json[0]["name"], "demo");
    assert_eq!(json[0]["languages"][0], "c");
    assert!(env.ok(&["-search", "python"]).contains("demo"));
    assert_eq!(env.ok(&["-path", "demo"]).trim(), dir.to_str().unwrap());

    // edit via flags
    env.ok(&[
        "-edit",
        "demo",
        "--description",
        "Second",
        "--libs",
        "m,pthread",
    ]);
    assert!(env.ok(&["-info", "demo"]).contains("Second"));
    let meta: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(dir.join("prox.json")).unwrap()).unwrap();
    assert_eq!(meta["libs"][1], "pthread");

    // edit via editor (`true` leaves the file valid)
    env.ok(&["-edit", "demo", "--file"]);

    // dry run changes nothing
    env.ok(&["-delete", "demo", "--dry-run"]);
    assert!(dir.exists());

    // delete needs --yes when not interactive
    assert!(!env.prox(&["-delete", "demo"]).status.success());
    assert!(dir.exists());
    env.ok(&["-delete", "demo", "--yes"]);
    assert!(!dir.exists());
    assert!(env.prox(&["-path", "demo"]).status.code() == Some(1));
}

#[test]
fn add_existing_folder_and_rename() {
    let env = Env::new();
    let outside = env.home.path().join("elsewhere/legacy");
    fs::create_dir_all(&outside).unwrap();
    env.ok(&["-add", outside.to_str().unwrap()]);
    assert!(env.ok(&["-list"]).contains("legacy"));
    env.ok(&["-edit", "legacy", "--rename", "modern"]);
    assert!(env.ok(&["-list"]).contains("modern"));
    assert!(env.home.path().join("elsewhere/modern/prox.json").exists());
    // --keep-files only unregisters
    env.ok(&["-delete", "modern", "--yes", "--keep-files"]);
    assert!(env.home.path().join("elsewhere/modern").exists());
}

#[test]
fn broken_json_edit_is_rejected() {
    let env = Env::new();
    env.ok(&["create", "keep", "--no-git"]);
    // an "editor" that corrupts the file
    let h = env.home.path();
    let script = h.join("bad-editor.sh");
    fs::write(&script, "#!/bin/sh\necho '{ nope' > \"$1\"\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();
    }
    env.ok(&[
        "--settings",
        "set",
        "default_editor",
        script.to_str().unwrap(),
    ]);
    let before = fs::read_to_string(env.project("keep").join("prox.json")).unwrap();
    assert!(!env.prox(&["edit", "keep", "--file"]).status.success());
    assert_eq!(
        fs::read_to_string(env.project("keep").join("prox.json")).unwrap(),
        before
    );
}

#[test]
fn no_args_prints_help_and_version_works() {
    let env = Env::new();
    assert!(env.ok(&[]).contains("COMMAND SPELLINGS"));
    assert!(env.ok(&["--version"]).contains("prox"));
    assert!(env.ok(&["--settings"]).contains("default_editor"));
    assert!(env.ok(&["--completions", "bash"]).contains("prox"));
}
