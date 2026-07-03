# Tallytail Asset Manager

Tallytail is a personal asset and investment management app. It helps track
asset allocations, transactions, and portfolio positions.

## Initial setup

### Dependencioes

- Rust and cargo (https://rust-lang.org/, tested with rustc 1.94.1)
- Clang (https://github.com/llvm/llvm-project, tested with clang 22.1.5)
- Trunk (https://crates.io/crates/trunk, tested with trunk 0.21.14)

### Env variables

The `TALLYTAIL_DATA_DIR` environment variable must point to the folder where your SQLite and RON files for Tallytail are stored.

Temporary:

```powershell
$Env:TALLYTAIL_DATA_DIR = "C:\tallytail_data"
```

Persistently:

```powershell
setx TALLYTAIL_DATA_DIR "C:\tallytail_data"
```

For Fly.io, the Dockerfile sets `TALLYTAIL_DATA_DIR=/app/data`, and the persistent volume is mounted to `/app/data`.

## Running the app from source

### Desktop target

```powershell
cargo run -p desktop_app
```

### Web target

```powershell
cargo run -p web_back_end
```

```powershell
trunk serve --config web_front_end/Trunk.toml
```

Then, open the front end URL in a browser.

## Preparing changes for a commit

```powershell
./precommit.ps1
```

This script should be run from the repository root and must succeed before each
commit.
