# ālaya

personal book notes

## Setup

```sh
# Compile project (development mode)
cargo build

# Compile and serve on default port :3000
cargo run -- --serve

# Serve with auto-reload
cargo watch -x 'run -- --serve'

# Production mode
cargo run --release -- --serve
```

The server will start at `http://127.0.0.1:3000`

## License

GNU Affero General Public License version 3
