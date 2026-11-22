{
  description = "Pick a browser for each link through your dmenu-style launcher";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs =
    { self, nixpkgs }:
    let
      forAllSystems = nixpkgs.lib.genAttrs [
        "x86_64-linux"
        "aarch64-linux"
      ];
    in
    {
      packages = forAllSystems (system: {
        default = nixpkgs.legacyPackages.${system}.callPackage ./packaging/package.nix { };
      });

      overlays.default = final: _: {
        urlmenu = final.callPackage ./packaging/package.nix { };
      };
    };
}
