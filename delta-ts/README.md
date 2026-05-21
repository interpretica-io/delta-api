# Delta API Test Suite

Test suite for the **Delta API network server** (`delta-server`), built on the
[Test Environment](https://ts-factory.io/) framework.

The Delta API server wraps a node pool and exposes it over TCP using a
newline-delimited JSON protocol: each request is one JSON object terminated by
`\n`, and each response is one JSON object terminated by `\n`. This suite
launches the server and verifies its protocol end to end.

## Structure

```
delta-ts/
├── delta-ts/                   # Test suite source
│   ├── lib/                    # Test suite helper library
│   │   ├── delta_api.c/.h      # delta-server launcher + JSON protocol client
│   │   └── tsapi_evo.c/.h      # charts and analysis hints
│   ├── server/                 # Server test group
│   │   ├── build_delta_server.sh  # Builds delta-server from the delta-api crate
│   │   ├── ping.c              # Liveness probe and empty pool listing
│   │   ├── node_lifecycle.c    # Node registration lifecycle
│   │   ├── error_handling.c    # Malformed request handling
│   │   └── shared_state.c      # Shared pool state and request pipelining
│   ├── prologue.c              # Suite prologue
│   └── epilogue.c              # Suite epilogue
├── conf/                       # Configuration
└── scripts/                    # Helper scripts
```

## Prerequisites

- Docker
- The `delta-api` crate: the suite lives **inside** the `delta-api`
  repository, so its source is the parent directory of this suite.
- `test-environment` cloned alongside the `delta-api` repository:

```bash
git clone https://github.com/ts-factory/test-environment.git
```

## How it works

`delta-server` is a Rust binary. During the suite build, `build_delta_server.sh`
compiles it from the `delta-api` crate (`cargo build --features server`) and
installs it next to the server tests. Each test launches its own private
`delta-server` instance bound to an ephemeral loopback port, drives the JSON
protocol over a TCP socket, and stops the server on cleanup.

The crate location is taken from the `DELTA_API_SRC` environment variable
(set automatically by `scripts/run.sh`), falling back to the parent of the
suite directory.

## Running

```bash
./scripts/run.sh docker guess --cfg=localhost
```

The suite runs the following tests:

| Test             | Verifies                                                |
|------------------|---------------------------------------------------------|
| `ping`           | Liveness probe (`ping`/`pong`) and empty pool listing   |
| `node_lifecycle` | `add` / `list_nodes` / `is_connected` / `is_alive` / `remove` |
| `error_handling` | Malformed requests and unknown-node rejection           |
| `shared_state`   | Shared pool state across connections, request pipelining |

## Results

Logs are written to the build directory after the run:

```bash
cat log.txt
```

Generate HTML:

```bash
./scripts/html-log.sh
```

## License

MIT — Copyright (C) 2025-2026 Interpretica Unipessoal Lda

## Authors

- Maxim Menshikov — <maxim.menshikov@interpretica.io>
