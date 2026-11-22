# prox

**prox** is a fast, single-binary project manager for the Linux terminal. It creates, opens, lists,
edits and deletes projects, and it learns new languages through small installable packages called
**modulos**, downloaded from this GitHub repository.

Every project it creates comes with a ready-made `build.sh` / `build.bat` and with configuration for
**VS Code, Zed and Neovim**, so your editor knows where your headers, libraries and virtualenv are
and does not fill the file with red squiggles.

```console
$ prox -create game --lang c --libs m,pthread -d "My first game"
✓ created /home/you/prox-projects/game
  languages: c
  libs:      m, pthread
  build:     ./build.sh  (Windows: build.bat)
  editors:   VS Code, Zed and Neovim are configured to find include/ and libs/

$ cd "$(prox -path game)" && ./build.sh run
==> building c
Hello from game!
```

## Contents

- [Features](#features)
- [Installation](#installation)
- [Quick start](#quick-start)
- [Commands](#commands)
- [What a new project contains](#what-a-new-project-contains)
- [Editor integration (VS Code, Zed, Neovim)](#editor-integration-vs-code-zed-neovim)
- [Settings](#settings)
- [Modulos](#modulos)
- [Writing and publishing your own modulo](#writing-and-publishing-your-own-modulo)
- [Self-update](#self-update)
- [Repository layout](#repository-layout)
- [`proxcd`: jump into a project](#proxcd-jump-into-a-project)
- [Shell completions](#shell-completions)
- [Troubleshooting](#troubleshooting)
- [Contributing](#contributing)
- [License](#license)

## Features

- Create / open / list / edit / delete / search / info projects, plus `-add` for existing folders.
- Language support comes from **modulos**: `c`, `cpp`, `rust`, `python`, `web` (HTML/CSS/JS) and
  `java` ship in this repo; add your own.
- Generated **`build.sh` and `build.bat`** (`build`, `run`, `clean`) composed from the modulos you pick.
- Generated **editor configuration** for VS Code (`.vscode/`), Zed (`.zed/`), Neovim (`.nvim.lua`),
  clangd, Pyright, `jsconfig.json` and the Java language server.
- Modulos are **checksum-verified** and installed **atomically**; a failed update never corrupts an
  installed modulo.
- `prox --update` updates the binary (verified, atomically replaced) and all modulos.
- Offline mode: the modulo index is cached and used with a clear warning when GitHub is unreachable.
- Nice errors that say what failed *and* how to fix it, real exit codes, `NO_COLOR` support,
  `--dry-run` and `--yes`.
- **Modulos never run code.** They ship templates only. prox writes `build.sh`, but *you* run it.

## Installation

### 1. Install Rust

```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"
rustc --version        # needs 1.82 or newer
```

### 2. Build from source

```sh
git clone https://github.com/ACavalierWispro/prox.git
cd prox
cargo build --release
```

The binary is `target/release/prox`. It uses `rustls`, so there is no OpenSSL dependency.

### 3. Install the binary to `~/.local/bin`

```sh
mkdir -p ~/.local/bin
install -m 755 target/release/prox ~/.local/bin/prox
```

Make sure `~/.local/bin` is on your `PATH` (add `export PATH="$HOME/.local/bin:$PATH"` to your shell
profile if `prox --version` is not found).

### Or: download a release binary

Each GitHub Release contains `prox-<tag>-x86_64-linux` and `prox-<tag>-aarch64-linux`
(plus `.sha256` files). Download the right one, verify it, and put it in `~/.local/bin`:

```sh
sha256sum -c prox-v0.1.0-x86_64-linux.sha256
install -m 755 prox-v0.1.0-x86_64-linux ~/.local/bin/prox
```

### First run

On first run prox asks for your editor, author name and project folder (it only asks when attached to
a terminal), then offers to download the starter modulos. You can do it later with:

```sh
prox --modulos download all
```

## Quick start

```sh
prox --modulos download all          # get c, cpp, rust, python, web, java
prox -create hello                   # interactive: description, author, languages, libs, git
prox -list
prox -open hello                     # opens your editor and records last_opened
prox -path hello                     # prints the folder (see proxcd)
prox -info hello
prox -delete hello                   # asks you to type the name
```

## Commands

Every command can be written three ways: `prox -create x`, `prox --create x`, or `prox create x`.

| Command | What it does |
|---------|--------------|
| `prox -create <name>` | Create a project (interactive; flags pre-fill answers) |
| `prox -open <name>` | Open in your editor; updates `last_opened` |
| `prox -path <name>` | Print the project folder |
| `prox -list` | List project names |
| `prox -edit <name>` | Edit metadata |
| `prox -delete <name>` | Delete a project |
| `prox -info <name>` | Show all metadata |
| `prox -search <text>` | Search name, description, languages and libs |
| `prox -add <path>` | Register an existing folder |
| `prox --modulos list\|update\|download\|checksum` | Manage modulos |
| `prox --update` | Update prox and all modulos |
| `prox --settings [set <k> <v> \| edit]` | Show / change settings |
| `prox --doctor` | Check your setup |
| `prox --completions bash\|zsh\|fish` | Shell completions |
| `prox --help`, `prox -h`, `prox --version` | Help / version |

Running `prox` with no arguments prints the help. Global flags: `--yes` (skip confirmations) and
`--dry-run` (show what would happen).

### `-create`

```sh
prox -create site                      # fully interactive
prox -create game --lang c,python \
     --libs m,SDL2,requests \
     -d "A game" -a "Ann" --git        # fully scripted
prox -create tool --lang rust --no-git --dir ~/work
prox -create demo --lang cpp --dry-run # preview the files, write nothing
```

Questions asked (each can be pre-filled with a flag): description (`-d`), author (`-a`, default from
settings), languages (`--lang`, multi-select from installed language modulos), libraries
(`--libs`, multi-select of what the chosen modulos suggest plus free text) and `git init`
(`--git` / `--no-git`).

`--lang` accepts a modulo name, an alias or a provided language: `c++`, `cpp`, `html`, `js` and
`javascript` all work.

prox refuses to overwrite an existing project or folder, validates the name (letters, digits, `-`,
`_`, `.`; no spaces or slashes), and removes the half-created folder if scaffolding fails.

### `-open`

Opens the project in the editor from settings (`default_editor`, then `$EDITOR`, then `$VISUAL`).
A child process cannot change your shell's directory, so use [`proxcd`](#proxcd-jump-into-a-project)
to `cd` into a project.

### `-list`

```sh
prox -list            # names, one per line
prox -list --long     # + description, languages, last opened, path
prox -list --recent   # most recently opened first
prox -list --json     # for scripts
```

### `-edit`

```sh
prox -edit game                          # interactive prompts
prox -edit game --file                   # open prox.json in your editor; JSON is validated afterwards
prox -edit game -d "New text" --libs m   # flags
prox -edit game --rename arcade          # renames metadata, registry and (if it matches) the folder
```

If you save broken JSON with `--file`, prox restores the previous `prox.json` and tells you why.
Changing languages updates the metadata only; existing files are not regenerated.

### `-delete`

Asks you to **type the project name**. `--yes` skips the prompt, `--keep-files` only unregisters the
project, `--dry-run` shows what would be deleted. prox only deletes folders that contain a `prox.json`.

### Modulo commands

```sh
prox --modulos list                 # NAME | INSTALLED | LATEST | STATUS
prox --modulos update all           # update installed modulos that have a newer version
prox --modulos update rust
prox --modulos download all         # download modulos that are NOT installed yet
prox --modulos download java        # (dependencies are downloaded too)
prox --modulos checksum ./my-modulo --write   # for authors, see below
```

`update` only touches installed modulos; one failure does not abort the rest. `download` only fetches
modulos that are not installed and tells you when something already is.

## What a new project contains

For `prox -create demo --lang c,python`:

```
demo/
├── prox.json              # metadata (name, description, author, languages, libs, modulos, ...)
├── README.md
├── build.sh               # ./build.sh [build|run|clean]
├── build.bat              # build.bat [build|run|clean]
├── .editorconfig
├── .nvim.lua              # Neovim (project-local)
├── .vscode/               # settings.json, c_cpp_properties.json, tasks.json, extensions.json
├── .zed/                  # settings.json, tasks.json
├── compile_flags.txt      # clangd
├── pyrightconfig.json     # Pyright
├── src/                   # main.c, main.py
├── include/               # your own headers
└── libs/                  # third-party libraries (README inside explains the layout)
```

### `build.sh` and `build.bat`

Generated from the `build` section of every selected modulo:

```sh
./build.sh            # build everything
./build.sh run        # build, then run the first language that can run
./build.sh clean      # remove build output
```

Each modulo's step checks for its tools first (`gcc`, `cargo`, `javac`, ...) and tells you what is
missing. The scripts are plain text: edit them freely. Libraries chosen at creation time are wired in
(for example `-lm -lpthread` for C, `[dependencies]` for Rust, `requirements.txt` for Python).

| Modulo | Build | Run | Output |
|--------|-------|-----|--------|
| `c` | `gcc -std=c11 ...` | `./build/<name>` | `build/` |
| `cpp` | `g++ -std=c++17 ...` | `./build/<name>` | `build/` |
| `rust` | `cargo build --release` | `cargo run --release` | `target/` |
| `python` | creates `.venv`, `pip install -r requirements.txt`, byte-compiles | `python src/main.py` | `.venv/` |
| `web` | copies `src/` and `libs/` to `dist/` | serves `dist/` on <http://localhost:8000> | `dist/` |
| `java` | `javac` with `libs/*` on the classpath | `java -cp build/classes:libs/* Main` | `build/` |

## Editor integration (VS Code, Zed, Neovim)

The goal: open the folder, and imports/includes resolve with no errors.

| Language | What is configured |
|----------|--------------------|
| C / C++ | `compile_flags.txt` (clangd: `-Iinclude -Ilibs/include`); `.vscode/c_cpp_properties.json` and settings (`includePath`); Zed clangd arguments; Neovim `path` |
| Rust | `rust-analyzer` reads `Cargo.toml`; clippy as the check command in VS Code and Zed |
| Python | `pyrightconfig.json` (`extraPaths: src, libs`, `venv: .venv`); VS Code interpreter and `python.analysis.extraPaths`; Zed Pyright settings; Neovim `PYTHONPATH` |
| Web | `jsconfig.json` including `src/` and `libs/` (used by tsserver in all three editors) |
| Java | `.project` + `.classpath` (read by jdtls); **every build regenerates `.classpath` with one entry per jar in `libs/`**; VS Code `java.project.referencedLibraries` |

Also generated: `.vscode/tasks.json` (build / run / clean tasks, default build task = `build`),
`.zed/tasks.json` (same tasks in Zed's task picker), and `.vscode/extensions.json` recommendations.
When a project mixes several languages, the files are **merged** (for example one
`c_cpp_properties.json` with both C and C++ standards).

**Neovim note:** `.nvim.lua` is project-local config. Neovim loads it only when `exrc` is on:

```lua
vim.o.exrc = true   -- in your init.lua; then run :trust once inside the project
```

It also sets `makeprg` to `./build.sh`, so `:make` builds the project.

## Settings

`~/.config/prox/settings.json`, created on first run:

```json
{
  "default_editor": "nano",
  "default_author": "",
  "default_project_dir": "~/prox-projects",
  "github_repo": "ACavalierWispro/prox",
  "github_token": ""
}
```

```sh
prox --settings                              # show (token is hidden)
prox --settings set default_editor nvim
prox --settings set default_project_dir ~/code
prox --settings edit                         # opens the file; invalid JSON is rejected
```

`~` is expanded in paths. `GITHUB_TOKEN` in the environment overrides `github_token`. Use a token to
avoid API rate limits or to use a private repository.

Other files: `~/.local/share/prox/registry.json` (name → folder, so projects can live anywhere),
`~/.local/share/prox/modulos/` (installed modulos) and `~/.cache/prox/index.json` (offline index).

## Modulos

A modulo is a folder in this repo under `modulos/<name>/` containing a `modulo.json` and its template
files. Installed modulos live in `~/.local/share/prox/modulos/`.

- Remote information comes from the GitHub API (one call to list the tree) and
  `raw.githubusercontent.com` (manifests and templates).
- Downloads are staged in a temp folder, **checksum-verified**, then renamed into place.
- If GitHub cannot be reached, prox uses the cached index and warns you.
- Modulos can depend on other modulos; dependencies are resolved first and cycles are reported.

## Writing and publishing your own modulo

### 1. Create the folder

```
modulos/go/
├── modulo.json
└── templates/
    ├── main.go
    ├── go.mod
    ├── gitignore
    └── vscode-settings.json
```

(Name template files without a leading dot, such as `gitignore`; the `dest` field gives the final name.)

### 2. Write `modulo.json`

```json
{
  "name": "go",
  "version": "1.0.0",
  "description": "Go projects with gopls configured",
  "type": "language",
  "aliases": ["golang"],
  "provides": {
    "languages": ["go"],
    "libs": ["github.com/spf13/cobra", "github.com/stretchr/testify"]
  },
  "dependencies": [
    { "name": "some-other-modulo", "version": ">=1.0.0" }
  ],
  "dirs": ["cmd", "libs"],
  "files": [
    { "src": "templates/main.go",   "dest": "cmd/main.go" },
    { "src": "templates/go.mod",    "dest": "go.mod" },
    { "src": "templates/gitignore", "dest": ".gitignore", "merge": "append" },
    { "src": "templates/vscode-settings.json", "dest": ".vscode/settings.json", "merge": "json" }
  ],
  "build": {
    "requires": ["go"],
    "problem_matcher": "$go",
    "build_sh":  ["mkdir -p build", "go build -o build/{{name}} ./cmd"],
    "build_bat": ["if not exist build mkdir build", "go build -o build\\{{name}}.exe .\\cmd"],
    "run_sh":    ["./build/{{name}}"],
    "run_bat":   ["build\\{{name}}.exe"],
    "clean_sh":  ["rm -rf build"],
    "clean_bat": ["if exist build rmdir /s /q build"]
  },
  "checksum": "sha256:filled-in-by-prox",
  "min_prox_version": "0.1.0"
}
```

| Field | Meaning |
|-------|---------|
| `name` | Lowercase letters, digits, `-`, `_`. Must equal the folder name. |
| `version` | Semantic version. A higher version is what makes `--modulos update` fire. |
| `type` | `language`, `library` or `tool`. Only `language` modulos appear in the language picker. |
| `aliases` | Extra names accepted by `--lang`. |
| `provides.languages` | Languages shown and stored in `prox.json`. |
| `provides.libs` | Suggestions for the libs prompt. |
| `dependencies` | Other modulos, with an optional semver requirement. |
| `dirs` | Folders to create in the project. |
| `files` | Templates to copy (see below). |
| `build` | Command lists for `build.sh` / `build.bat` (all optional). |
| `checksum` | Integrity hash, written by `prox --modulos checksum <dir> --write`. |
| `min_prox_version` | Refuse to install on older prox versions. |

**`files`.** `src` is relative to the modulo folder, `dest` to the project. Absolute paths and `..` are
rejected, so a modulo cannot write outside the project. `merge` decides what happens when the file
already exists (several modulos often write `.gitignore` or `.vscode/settings.json`):

- `skip` (default): keep the existing file.
- `append`: append the text unless it is already there.
- `json`: deep-merge JSON (objects merge, arrays are unioned, arrays of objects with the same
  `name`/`label` are merged).

**Placeholders** in templates: `{{name}}`, `{{crate_name}}`, `{{description}}`, `{{author}}`,
`{{year}}`, `{{libs}}`, `{{libs_link}}` (`-lm -lpthread`), `{{libs_lines}}` (one per line),
`{{libs_cargo}}` (`foo = "*"` lines), `{{libs_json_obj}}` (`"a": "*", "b": "*"`) and `{{libs_json_arr}}`.
Values are escaped for the destination type (JSON/TOML/YAML, HTML/XML). The same placeholders work
inside `build` commands.

**Security rules.** Modulos are templates, never programs: prox does not execute anything from a
modulo. The `build` commands are only *written* into `build.sh`/`build.bat`, which you run yourself.
Review a modulo's `build` section before trusting it, like any script.

### 3. Compute the checksum

```sh
prox --modulos checksum modulos/go --write
```

The checksum covers every template listed in `files` (by `dest` and content) **and** the manifest
itself (canonicalised, without its own `checksum` field), so tampering with either is detected. Re-run
this command every time you change anything in the folder. CI in this repo fails when a checksum is stale.

### 4. Test locally

Copy the folder to `~/.local/share/prox/modulos/go/` and run `prox -create t --lang go --dry-run`,
then without `--dry-run`, then `./build.sh` in the new project. `prox --doctor` validates installed
modulos.

### 5. Publish

Commit `modulos/go/` to the default branch of the repo configured in `github_repo` and push. Anyone
can now run `prox --modulos download go`. To release a new version, bump `version`, re-run the
checksum command, and push; users get it with `prox --modulos update go`.

## Self-update

`prox --update` checks the latest GitHub Release, compares it with the running version (semver),
downloads the right Linux binary (`x86_64` or `aarch64`), verifies its SHA-256 against the published
`.sha256` file, test-runs the new binary, and then atomically replaces the running executable (a
rename, so there is never a half-written binary). Then it updates all installed modulos. If everything
is current it says so. `--dry-run` shows what would happen.

## Repository layout

```
prox/
├── Cargo.toml
├── README.md
├── LICENSE
├── .github/workflows/
│   ├── ci.yml                # fmt + clippy + build + test on push/PR
│   └── release.yml           # on a v* tag: build x86_64 + aarch64, attach to a Release
├── src/
│   ├── main.rs               # entry, dispatch, exit codes
│   ├── cli.rs                # clap definitions + single-dash rewriting
│   ├── settings.rs           # settings.json + first-run wizard
│   ├── project.rs            # prox.json + name validation
│   ├── registry.rs           # name -> folder registry
│   ├── modulo.rs             # modulo.json, checksum, dependencies, atomic install
│   ├── scaffold.rs           # templates, placeholders, merge modes
│   ├── buildgen.rs           # build.sh / build.bat / editor tasks
│   ├── github.rs             # API + raw client, index cache, offline fallback
│   ├── updater.rs            # self-update
│   ├── util.rs               # paths, errors, helpers
│   └── commands/             # one file per command
├── tests/lifecycle.rs        # create -> list -> edit -> delete in a temp HOME
└── modulos/                  # c, cpp, rust, python, web, java
```

## `proxcd`: jump into a project

Add to `~/.bashrc` or `~/.zshrc`:

```sh
proxcd() { cd "$(prox -path "$1")"; }
```

Then `proxcd game` changes into the project folder. (fish: `function proxcd; cd (prox -path $argv[1]); end`)

## Shell completions

```sh
prox --completions bash > ~/.local/share/bash-completion/completions/prox
prox --completions zsh  > ~/.zfunc/_prox          # with ~/.zfunc in your fpath
prox --completions fish > ~/.config/fish/completions/prox.fish
```

Completions use the plain command names (`create`, `list`, ...); the dashed spellings behave the same.

## Troubleshooting

Run **`prox --doctor`** first: it checks your editor, project folder, GitHub access and installed modulos.

| Problem | Fix |
|---------|-----|
| `editor 'nano' was not found` | `prox --settings set default_editor <cmd>` or export `EDITOR` |
| `no language modulos installed` | `prox --modulos download all` |
| `GitHub API rate limit reached` | `export GITHUB_TOKEN=...` or `prox --settings set github_token ...` |
| `not found on GitHub` / 404 | Check `github_repo`; private repos need a token; push `modulos/` to the default branch |
| `has no published release yet` | Create the first release by pushing a `v*` tag |
| `checksum mismatch` | Corrupted or tampered download; nothing was installed. Retry; authors: re-run `prox --modulos checksum <dir> --write` |
| `cannot write next to ...` on `--update` | Install prox somewhere you own, such as `~/.local/bin` |
| Editor shows missing includes/imports | Build once (Java regenerates `.classpath`, Python creates `.venv`), and for Neovim enable `exrc` and run `:trust` |
| Colors in logs | Set `NO_COLOR=1` |

Exit codes: `0` ok, `1` error, `2` usage error, `3` network/offline, `4` verification failed.

## Contributing

1. Fork and create a branch.
2. `cargo fmt --all`, `cargo clippy --all-targets -- -D warnings`, `cargo test`.
3. If you touch a modulo, run `prox --modulos checksum modulos/<name> --write`.
4. Open a pull request.

New modulos are welcome; see [the guide above](#writing-and-publishing-your-own-modulo).

## License

MIT, see [LICENSE](LICENSE).
