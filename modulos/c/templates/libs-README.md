# libs/

Third-party C libraries live here. The build script, clangd, VS Code, Zed and Neovim are all
configured to look in these places:

| What | Where |
|------|-------|
| Headers | `libs/include/` (and your own in `include/`) |
| Static / shared libraries | `libs/lib/` (`-Llibs/lib`) |

System libraries chosen at creation time (e.g. `m`, `pthread`) are linked with `-l<name>`:
see the compiler line in `build.sh`. To link another one, add `-l<name>` to the `gcc` line in `build.sh`
and `build.bat`.

`./build.sh run` sets `LD_LIBRARY_PATH=libs/lib` so shared libraries placed here are found at runtime.
