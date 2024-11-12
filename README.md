# 🐺 Release Dog

This simple tool automatically checks for updates to your favorite software and sends you a notification when a new release is available.

## Usage

```sh
RUST_LOG=info ./target/release/release_dog -r zen-browser/desktop,wez/wezterm
```

## Advance

You can make `release_dog` as a systemd service, check [this](./release_dog@.service)

