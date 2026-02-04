# fiber-types Crate Design

## Overview

Create a lightweight `fiber-types` crate containing all store-related type definitions and deserialization utilities, enabling external applications to:

- Parse p2p gossip broadcast messages
- Parse and verify CKB Invoices
- Read and deserialize data from node's RocksDB

## Goals and Non-Goals

### Goals

- Provide all store-related type definitions with serde support
- Support both native and wasm32 compilation
- Minimal dependencies, fast compilation
- Export schema constants for direct RocksDB access

### Non-Goals

- Store read/write logic (external apps operate RocksDB directly)
- Actor communication and network layer code
- RPC service implementation

## Dependency Direction

```
fiber-types (new)  <--  fnn (fiber-lib)
     ^                       ^
External apps          fiber-bin, fiber-wasm
```

## Module Structure

```
crates/fiber-types/
├── Cargo.toml
└── src/
    ├── lib.rs
    ├── primitives.rs      # Layer 1: Basic types
    ├── protocol.rs        # Layer 2: Protocol message types
    ├── invoice.rs         # Invoice related types
    ├── channel.rs         # Channel state types
    ├── payment.rs         # Payment related types
    ├── network.rs         # Network state types
    ├── watchtower.rs      # Watchtower types (feature-gated)
    ├── schema.rs          # Store key prefixes
    ├── serde_utils.rs     # Serialization utilities
    └── gen/               # molecule generated code
        ├── mod.rs
        ├── fiber.rs
        └── gossip.rs
```

## Type Layering

| Layer | Module | Main Types |
|-------|--------|------------|
| Basic | primitives | Hash256, Pubkey, Privkey, NodeId, Cursor |
| Protocol | protocol | ChannelAnnouncement, ChannelUpdate, NodeAnnouncement, BroadcastMessage |
| Protocol | invoice | CkbInvoice, CkbInvoiceStatus, InvoiceError |
| Business | channel | ChannelActorState, ChannelState, ChannelFlags |
| Business | payment | PaymentSession, Attempt, PaymentStatus, TimedResult |
| Business | network | PersistentNetworkActorState |

## Dependencies

```toml
[dependencies]
# Serialization
serde = { version = "1.0", features = ["derive"] }
serde_with = { version = "3.7", features = ["macros"] }
bincode = "1.3"

# CKB types
ckb-types = "0.202.0"
ckb-gen-types = "0.202.0"

# Cryptography
secp256k1 = { version = "0.30", features = ["serde"] }
musig2 = { version = "0.2.4", features = ["secp256k1", "serde"] }

# Protocol encoding
molecule = { version = "0.8.0", default-features = false }
bech32 = "0.9"

# Network identity
tentacle = { version = "0.7", default-features = false }

# Other
hex = "0.4"
thiserror = "1.0"
bitflags = { version = "2.5", features = ["serde"] }

[features]
default = ["watchtower"]
watchtower = []
```

Key decisions:
- No dependency on `ractor`, `tokio`, `rocksdb`
- `tentacle` only for `PeerId` type, with `default-features = false`
- watchtower types controlled via feature flag

## Schema Export and Deserialization

```rust
// src/schema.rs
/// Store key prefixes - consistent with fnn
pub const CHANNEL_ACTOR_STATE_PREFIX: u8 = 0;
pub const PEER_ID_NETWORK_ACTOR_STATE_PREFIX: u8 = 16;
pub const CKB_INVOICE_PREFIX: u8 = 32;
pub const PREIMAGE_PREFIX: u8 = 33;
pub const CKB_INVOICE_STATUS_PREFIX: u8 = 34;
pub const PEER_ID_CHANNEL_ID_PREFIX: u8 = 64;
pub const CHANNEL_OUTPOINT_CHANNEL_ID_PREFIX: u8 = 65;
pub const BROADCAST_MESSAGE_PREFIX: u8 = 96;
pub const BROADCAST_MESSAGE_TIMESTAMP_PREFIX: u8 = 97;
pub const PAYMENT_SESSION_PREFIX: u8 = 192;
pub const PAYMENT_HISTORY_TIMED_RESULT_PREFIX: u8 = 193;
pub const PAYMENT_CUSTOM_RECORD_PREFIX: u8 = 194;
pub const ATTEMPT_PREFIX: u8 = 195;
pub const HOLD_TLC_PREFIX: u8 = 197;
pub const WATCHTOWER_TLC_SETTLED_PREFIX: u8 = 200;
#[cfg(feature = "watchtower")]
pub const WATCHTOWER_CHANNEL_PREFIX: u8 = 224;
#[cfg(feature = "watchtower")]
pub const WATCHTOWER_PREIMAGE_PREFIX: u8 = 225;
#[cfg(feature = "watchtower")]
pub const WATCHTOWER_NODE_PAYMENTHASH_PREFIX: u8 = 226;
pub const CCH_ORDER_PREFIX: u8 = 232;
```

```rust
// src/lib.rs
/// Deserialize store value
pub fn deserialize<'a, T: serde::Deserialize<'a>>(bytes: &'a [u8]) -> Result<T, bincode::Error> {
    bincode::deserialize(bytes)
}

/// Serialize to store value format
pub fn serialize<T: serde::Serialize>(value: &T) -> Result<Vec<u8>, bincode::Error> {
    bincode::serialize(value)
}
```

Usage example:
```rust
use fiber_types::{schema, ChannelActorState, deserialize};

// External app reads RocksDB
let key = [&[schema::CHANNEL_ACTOR_STATE_PREFIX], channel_id.as_ref()].concat();
let value = db.get(&key)?;
let state: ChannelActorState = deserialize(&value)?;
```

## fiber-lib Refactoring

### Cargo.toml

```toml
# crates/fiber-lib/Cargo.toml
[dependencies]
fiber-types = { path = "../fiber-types" }
```

### Re-export Strategy

```rust
// crates/fiber-lib/src/fiber/types.rs
pub use fiber_types::{
    Hash256, Pubkey, Privkey, NodeId,
    ChannelAnnouncement, ChannelUpdate, NodeAnnouncement,
    // ... other types
};

// Keep only business logic that depends on actor/network
```

```rust
// crates/fiber-lib/src/store/schema.rs
pub use fiber_types::schema::*;
```

Compatibility: Existing code using `fnn::fiber::types::Hash256` requires no changes.

## Challenges and Solutions

### Challenge 1: Circular Dependencies

Some types (e.g., `ChannelActorState`) reference types like `ProcessingChannelError` that depend on actor framework.

**Solution**:
- Move pure data structures to `fiber-types`
- Keep impl blocks with business logic in `fnn`
- Use trait abstraction where necessary, implement in `fnn`

### Challenge 2: molecule Generated Code

`gen/fiber.rs` and `gen/gossip.rs` are currently generated in `fiber-lib`'s build.rs.

**Solution**:
- Move `.mol` schema files and `build.rs` to `fiber-types`
- Or commit generated code directly to repository (reduces compile-time dependency)

### Challenge 3: Conditional Compilation

Some types have `#[cfg(not(target_arch = "wasm32"))]` or `#[cfg(feature = "watchtower")]` markers.

**Solution**:
- `fiber-types` inherits the same feature flags
- wasm32 conditional compilation remains unchanged

### Challenge 4: Time Types

wasm environment uses `web-time`, native uses `std::time`.

**Solution**:
- Define unified timestamp type (u64 milliseconds) in `fiber-types`
- Avoid direct dependency on platform-specific types like `Instant`

## Implementation Phases

### Phase 1: Create Crate Skeleton

- Create `crates/fiber-types/` directory structure
- Configure Cargo.toml and basic dependencies
- Add to workspace members

### Phase 2: Migrate Basic Types

- Migrate `Hash256`, `Pubkey`, `Privkey`, `NodeId`
- Migrate `serde_utils.rs` utility module
- Re-export in `fnn`, ensure compilation passes

### Phase 3: Migrate Protocol Types

- Migrate molecule generated code and `.mol` files
- Migrate `BroadcastMessage`, `ChannelAnnouncement`, etc.
- Migrate `CkbInvoice` related types

### Phase 4: Migrate Business State Types

- Migrate `ChannelActorState`, `ChannelState`
- Migrate `PaymentSession`, `Attempt`
- Handle circular dependency issues

### Phase 5: Export Schema and Validate

- Add `schema.rs` and utility functions
- Write integration tests to verify deserialization compatibility
- Ensure both native and wasm32 compile successfully

## Testing Strategy

### Unit Tests

- Serialization/deserialization round-trip tests for all types
- molecule encoding/decoding correctness tests

### Compatibility Tests

- Use existing node's RocksDB snapshot data
- Verify `fiber-types` can correctly deserialize all key prefix data
- Ensure full bincode format compatibility with `fnn`

### Cross-Platform Tests

- Add `cargo check --target wasm32-unknown-unknown -p fiber-types` in CI
- Ensure wasm32 compilation has no std dependency issues

### Regression Tests

- Existing `fnn` test suite should all pass
- Confirm re-export does not break existing API
