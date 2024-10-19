# Arch Updates - COSMIC Applet

WIP - COSMIC Applet to display Arch Linux package status.
Inspired by https://github.com/savely-krasovsky/waybar-updates and https://github.com/RaphaelRochet/arch-update.

![image](https://github.com/user-attachments/assets/61d0e2af-4036-4dd9-948c-55833f0f8230)

## Features
 - Native COSMIC look and feel.
 - pacman, AUR, and devel package upgrades shown.

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
