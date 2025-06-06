{ pkgs
, cargo-binutils
, rust-bin
}:
let
  toolchain = (rust-bin.fromRustupToolchainFile ../rust-toolchain.toml).override {
    extensions = [ "llvm-tools-preview" ];
    targets = [ "riscv32i-unknown-none-elf" ];
  };

  rustPlatform = pkgs.makeRustPlatform {
    cargo = toolchain;
    rustc = toolchain;
  };
  cargoFile = (builtins.fromTOML (builtins.readFile ../zksync_os/Cargo.toml)).package;

  src = ../.;
  cargoRoot = "zksync_os";

in
rustPlatform.buildRustPackage {
  inherit src;
  pname = cargoFile.name;
  version = cargoFile.version;

  cargoLock = {
    lockFile = ../zksync_os/Cargo.lock;
    allowBuiltinFetchGit = true;
  };

  inherit cargoRoot;

  auditable = false;
  buildPhase = ''
    pushd ${cargoRoot}
    sed -i -e "s#@NIX_BUILD_TOP@#$NIX_BUILD_TOP#" .cargo/config.toml
    sed -i -e "s#@BUILD_DIR@#$(pwd)#" .cargo/config.toml
    sed -i -e "s#@CARGO_DEPS_DIR@#''${cargoDepsLockfile%/Cargo.lock}#" .cargo/config.toml
    runHook preBuild
    cargo build --features proving --release --target=riscv32i-unknown-none-elf
    popd
    runHook postBuild
  '';

  installPhase = ''
    pushd ${cargoRoot}
    runHook preInstall
    mkdir -p $out
    cargo objcopy --features proving --release -- -O binary $out/app.bin
    cargo objcopy --features proving --release -- -R .text $out/app.elf
    cargo objcopy --features proving --release -- -O binary --only-section=.text $out/app.text
    popd
    runHook postInstall
  '';

  nativeBuildInputs = [
    cargo-binutils
    toolchain
    rustPlatform.bindgenHook
    rustPlatform.cargoSetupHook
  ];

  doCheck = false;
}
