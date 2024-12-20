# Rustic Builder

## Overview

A simple mock builder implementation that serves local mempool transactions from an Ethereum node through the Builder API flow.
It works as a wrapper over Lighthouse's [mock-builder](https://github.com/sigp/lighthouse/blob/unstable/beacon_node/execution_layer/src/test_utils/mock_builder.rs) which is used for lighthouse tests. This means that as Lighthouse implements support for new forks, the builder automatically gets support for the fork by just pointing it to the right lighthouse commit.

The name references both its implementation language (Rust) and its rustic nature - serving farm-to-table payloads from your local execution client.

## Installation

### From Source
```
cargo build --release
```

### Using Docker
```bash
docker build -t rustic-builder .
```

## Usage

Needs a fully synced ethereum node (Beacon node + Execution client)

### Running from Binary
```
./target/release/rustic-builder --execution-endpoint http://localhost:8551 --beacon-node http://localhost:5052 --jwt-secret jwt.hex --port 8560
```

### Running with Docker
```bash
docker run -p 8560:8560 \
  -v /path/to/jwt.hex:/jwt.hex \
  rustic-builder \
  --execution-endpoint http://execution-client:8551 \
  --beacon-node http://beacon-node:5052
```

Note: When running with Docker, make sure to:
- Mount your JWT secret file
- Use appropriate network settings (--network host if running nodes locally)
- Adjust the execution/beacon endpoints to match your setup
