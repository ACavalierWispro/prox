# libs/

Local Python modules and vendored packages go here. This folder and `src/` are added to the
import path in every tool:

- `build.sh run` sets `PYTHONPATH=src:libs`
- Pyright (VS Code, Zed, Neovim) reads `pyrightconfig.json` (`extraPaths`)
- VS Code also gets `python.analysis.extraPaths`

Packages from PyPI belong in `requirements.txt`; `./build.sh` installs them into `.venv/`, which the
editors use as the interpreter, so imports resolve without errors.
