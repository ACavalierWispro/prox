# libs/

Vendored front-end libraries (a downloaded `.js` or `.css` file, for example) go here.

- `./build.sh` copies `src/` and `libs/` into `dist/`, so reference them as `libs/<file>` from
  `src/index.html`.
- `jsconfig.json` includes `libs/**/*.js`, so VS Code, Zed and Neovim (tsserver) resolve them
  without errors.
