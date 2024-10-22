# Arch Updates - COSMIC Applet
WIP - COSMIC Applet to display Arch Linux package status.
Inspired by https://github.com/savely-krasovsky/waybar-updates and https://github.com/RaphaelRochet/arch-update.

![image](https://github.com/user-attachments/assets/61d0e2af-4036-4dd9-948c-55833f0f8230)

# arch_updates_rs - Arch updates API
Please refer to `arch-updates-rs/README.md` for more information.

## Features
 - Native COSMIC look and feel, supporting both light and dark mode.
 - pacman, AUR, and devel package upgrades shown.
 - Set up to support localisation - to support your language please submit your `.ftl` translations to the `./cosmic-applet-arch/i18n/` directory.
 - Modular API `arch-updates-rs` - able to be used in other similar projects.

## Development setup

To install this COSMIC applet, you will need [just](https://github.com/casey/just), if you're on Pop!\_OS, you can install it with the following command:

```sh
sudo apt install just
```

After you install it, you can run the following commands to build and install your applet:

```sh
just build-release
sudo just install
```
