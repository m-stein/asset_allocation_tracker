# Tallytail Asset Manager

Tallytail is a personal asset and investment management app. It helps track
asset allocations, transactions, and portfolio positions.

## Run locally

### Dependencies

- Rust and cargo (https://rust-lang.org/, tested with rustc 1.94.1)
- Clang (https://github.com/llvm/llvm-project, tested with clang 22.1.5)
- Trunk (https://crates.io/crates/trunk, tested with trunk 0.21.14)

### Env variables

The `TALLYTAIL_DATA_DIR` environment variable must point to the folder where your SQLite and RON files for Tallytail are stored.

```powershell
setx TALLYTAIL_DATA_DIR "C:\tallytail_data"
```

### Desktop target

```powershell
cargo run -p desktop_app
```

### Web target

```powershell
cargo run -p web_back_end
```

In a second shell:
```powershell
trunk serve --config web_front_end/Trunk.toml
```

Then, open the front end URL in a browser.

## Deploy to a fly.io account

Install `flyctl` and run the deployment commands below from the repository root.
If your Fly app uses a different name than `tallytail`, update `app` in `fly.toml`
and replace `tallytail` in the commands. If you want to use another region,
replace `fra` in the commands and update `primary_region` in `fly.toml`. 

```powershell
flyctl auth login
flyctl apps create tallytail
flyctl volumes create tallytail_data --app tallytail --region fra --size 1
flyctl secrets set TALLYTAIL_UNLOCK_PATTERN=123456 --app tallytail
flyctl deploy --app tallytail
```

`TALLYTAIL_UNLOCK_PATTERN` is required for the web app. It configures the unlock
pattern the user has to enter each time she or he opens the app. Use at least 6
points without repetitions. The point numbers are:

```text
1 2 3
4 5 6
7 8 9
```

Check your deployment:

```powershell
flyctl open --app tallytail
flyctl logs --app tallytail
```

After 3 failed unlock attempts from the same client IP, the web unlock is
blocked for that IP for 3 hours. This blocked state is stored in
`TALLYTAIL_DATA_DIR/access_control_state.ron`, so it survives Fly machine
restarts.

Check the current access-control state of any client:

```powershell
flyctl ssh console --app tallytail -C "cat /app/data/access_control_state.ron"
```

To reset the access control state of all clients, delete this file from the
Fly volume:

```powershell
flyctl ssh console --app tallytail -C "rm -f /app/data/access_control_state.ron"
```

In case that Fly says that the VM is not ready for executing commands, check its state
and try starting the VM:

```powershell
fly status -a tallytail
fly machine list -a tallytail
fly machine start <MACHINE_ID> -a tallytail
```

## Data Tools

Small command-line tools for operating on Tallytail data live in `data_tools`.

### Create data backup from fly.io account

Create a compressed local backup of the Fly.io volume-mounted data files:

```powershell
cargo run -p data_tools --bin fly_data_backup -- --url https://tallytail.fly.dev --out-dir C:\Backups\Tallytail --unlock-pattern 123456789
```

The backup file is named like `tallytail-data-20260707-213000.tar.gz`.
It contains only:

- `assets.sdb`
- `transactions.sdb`
- `allocation_records/`

The tool checks the Fly Machines for the app, selects a Machine with the
`/app/data` mount, and starts it automatically if it is stopped.

The local tool unlocks the web backend and requests a server-side backup. The
backend uses SQLite's online backup API for the `.sdb` files and returns the
compressed archive to the local tool. It does not store backup archives on the
Fly volume.


## Participate in development

### Preparing changes for a commit

```powershell
./precommit.ps1
```

This script should be run from the repository root and must succeed before each
commit.
