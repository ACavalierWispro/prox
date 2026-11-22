# libs/

Drop `.jar` files here.

- `javac`/`java` use them through `-cp "libs/*"` (see `build.sh`).
- Every `./build.sh` run regenerates `.classpath` with one entry per jar, which is what the Java
  language server (jdtls: VS Code, Zed, Neovim) reads, so the editors stop reporting missing
  imports as soon as you build once after adding a jar.
- VS Code additionally gets `java.project.referencedLibraries` for `libs/**/*.jar`.
