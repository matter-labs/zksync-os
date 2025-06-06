{ pkgs
, zkos
}:
pkgs.mkShell {
  inputsFrom = [ zkos ];
  packages = with pkgs; [
    clippy
    rustfmt
    rust-analyzer
  ];
}

