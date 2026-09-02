{
  description = "Tiny alarm clock that only rings into a connected bluetooth headset";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs =
    { self, nixpkgs }:
    let
      system = "x86_64-linux";
      pkgs = nixpkgs.legacyPackages.${system};

      # winit/femtovg 在运行期 dlopen 这些库,只在构建期链接是不够的;
      # fontconfig/freetype 在构建期还要能被 pkg-config 找到,否则
      # yeslogic-fontconfig-sys 的 build.rs 直接 panic。
      runtimeLibs = with pkgs; [
        fontconfig
        freetype
        wayland
        libxkbcommon
        libGL
        libx11
        libxcursor
        libxrandr
        libxi
        libxcb
      ];

      # 耳机判定读 pw-dump,响铃跑 pw-play,两个都来自 pipewire,得在 PATH 上。
      runtimeTools = [ pkgs.pipewire ];
    in
    {
      packages.${system} = rec {
        default = nap-alarm;

        nap-alarm = pkgs.rustPlatform.buildRustPackage {
          pname = "nap-alarm";
          version = "0.1.0";
          src = self;
          cargoLock.lockFile = ./Cargo.lock;

          nativeBuildInputs = with pkgs; [
            pkg-config
            makeWrapper
          ];
          buildInputs = runtimeLibs;

          # 界面测试要建 Slint 窗口,构建沙箱里没有合成器,交给 `just check` 跑。
          doCheck = false;

          postInstall = ''
            wrapProgram $out/bin/nap-alarm \
              --prefix PATH : ${pkgs.lib.makeBinPath runtimeTools} \
              --prefix LD_LIBRARY_PATH : ${pkgs.lib.makeLibraryPath runtimeLibs}
          '';

          meta = {
            description = "Alarm clock that rings only into a connected bluetooth headset";
            homepage = "https://github.com/yebei199/nap-alarm";
            license = pkgs.lib.licenses.gpl3Only;
            mainProgram = "nap-alarm";
            platforms = [ system ];
          };
        };
      };

      devShells.${system}.default = pkgs.mkShell {
        packages = [ pkgs.pkg-config ] ++ runtimeTools ++ runtimeLibs;
        LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath runtimeLibs;
      };
    };
}
