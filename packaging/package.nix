{
  lib,
  rustPlatform,
  installShellFiles,
}:

let
  manifest = lib.importTOML ../Cargo.toml;
in
rustPlatform.buildRustPackage {
  pname = "urlmenu";
  inherit (manifest.package) version;

  src = lib.fileset.toSource {
    root = ../.;
    fileset = lib.fileset.unions [
      ../Cargo.toml
      ../Cargo.lock
      ../src
      ../packaging
      ../config.sample.toml
    ];
  };

  cargoLock.lockFile = ../Cargo.lock;

  nativeBuildInputs = [ installShellFiles ];

  postInstall = ''
    installManPage packaging/man/*.1
    installShellCompletion --cmd urlmenu \
      --bash packaging/completions/urlmenu.bash \
      --fish packaging/completions/urlmenu.fish \
      --zsh packaging/completions/_urlmenu
    install -Dm644 packaging/urlmenu.desktop -t $out/share/applications
    install -Dm644 packaging/urlmenu.svg -t $out/share/icons/hicolor/scalable/apps
    install -Dm644 config.sample.toml -t $out/share/doc/urlmenu
  '';

  meta = {
    description = "Pick a browser for each link through your dmenu-style launcher";
    homepage = "https://github.com/bitxikeria/urlmenu";
    license = lib.licenses.unlicense;
    mainProgram = "urlmenu";
    platforms = lib.platforms.linux;
  };
}
