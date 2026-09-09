{ pkgs ? import <nixpkgs> {} }:

pkgs.mkShell {
  buildInputs = [
    pkgs.p7zip
    pkgs.python3
    pkgs.curl
    pkgs.jq
    pkgs.wine
    pkgs.git
    pkgs.cargo
    pkgs.rustc
    pkgs.cargo-xwin
  ];

  shellHook = ''
    echo "============================================="
    echo "  DMMGamePlayer Wine Repack Dev Shell"
    echo "============================================="
    echo "Available commands:"
    echo "  ./scripts/build.sh [version]      - Download & build release package"
    echo "  cargo xwin build --release --target x86_64-pc-windows-msvc"
    echo ""
  '';
}
