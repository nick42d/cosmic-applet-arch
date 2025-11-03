# Arch Updates - COSMIC Applet
COSMIC Applet to display Arch Linux package status.
Inspired by https://github.com/savely-krasovsky/waybar-updates and https://github.com/RaphaelRochet/arch-update.

![scn](https://github.com/user-attachments/assets/69c49436-226f-4349-afae-94d34694d565)

# arch_updates_rs - Arch updates API
Please refer to `arch-updates-rs/README.md` for more information.

## How to use
- The package is in the AUR under `cosmic-applet-arch`. You can install it via your favourite AUR helper, e.g `paru -Syu cosmic-applet-arch`.
- Once installed, the app can be added via the COSMIC Settings app -> Desktop -> Panel/Dock -> Configure panel/dock applets -> Add applet.

## Features
 - Native COSMIC look and feel, supporting both light and dark mode.
 - pacman, AUR, and devel package upgrades shown.
 - Latest news from Arch news feed shown (all news after last full system upgrade).
 - Package upgrades and news feed are clickable taking you to associated web page.
 - Set up to support localisation - to support your language please submit your `.ftl` translations to the `./cosmic-applet-arch/i18n/` directory.
 - Modular API `arch-updates-rs` - able to be used in other similar projects.

## Configuration
- Some basic configuration options can be provided by creating the file `~/.config/cosmic-applet-arch/config.toml`.
- An example configuration file with the default values is provided [in the repo](https://github.com/nick42d/cosmic-applet-arch/blob/main/cosmic-applet-arch/config/config.toml).
- If an incorrect configuration file has been supplied the applet will crash. To view a descriptive error, run the applet in your terminal.

## Development setup
Development dependencies are listed on the [PKGBUILD in the AUR](https://aur.archlinux.org/cgit/aur.git/tree/PKGBUILD?h=cosmic-applet-arch)
You can run the following commands to build and install:

```sh
just build-release
sudo just install
```

Logging is currently to stderr, so can be viewed by initiating the applet in a terminal.
