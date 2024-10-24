# Arch Updates - COSMIC Applet
WIP - COSMIC Applet to display Arch Linux package status.
Inspired by https://github.com/savely-krasovsky/waybar-updates and https://github.com/RaphaelRochet/arch-update.

![image](https://github.com/user-attachments/assets/61d0e2af-4036-4dd9-948c-55833f0f8230)

# arch_updates_rs - Arch updates API
Please refer to `arch-updates-rs/README.md` for more information.

## How to use
The package is in the AUR under `cosmic-applet-arch`. You can install it via your favourite AUR helper, e.g `paru -Syu cosmic-applet-arch`.

## Features
 - Native COSMIC look and feel, supporting both light and dark mode.
 - pacman, AUR, and devel package upgrades shown.
 - Set up to support localisation - to support your language please submit your `.ftl` translations to the `./cosmic-applet-arch/i18n/` directory.
 - Modular API `arch-updates-rs` - able to be used in other similar projects.

## Development setup

Development dependencies are listed on the [PKGBUILD in the AUR](https://aur.archlinux.org/cgit/aur.git/tree/PKGBUILD?h=cosmic-applet-arch)
You can run the following commands to build and install:

```sh
just build-release
sudo just install
```
