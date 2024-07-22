# EVM Simulator

EVM Simulator is a Rust-based tool for simulating and tracing Ethereum Virtual Machine (EVM) transactions within a specific block. It uses the `revm` library to recreate the EVM environment and execute transactions, providing detailed traces of each operation.

## Features

- Simulates EVM transactions for a given block
- Provides detailed transaction traces
- Supports various Ethereum networks through the `alloy` library
- Configurable output writer for flexible trace storage

## Requirements

- Rust (latest stable version)
- Cargo (Rust's package manager)

## Usage

To use the EVM Simulator, you need to create an instance of `EvmSimulator` with a provider and a writer, then call the `block_traces` method with a block identifier.

Here's a basic example:

```rust
use alloy::eips::BlockId;
use alloy::network::AnyNetwork;
use alloy::providers::ProviderBuilder;
use evm_simulator::EvmSimulator;
use url::Url;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Set up your Ethereum provider (e.g., Alchemy)
    let alchemy_api_url = Url::parse("https://eth-mainnet.g.alchemy.com/v2/your-api-key")?;

    let provider = ProviderBuilder::new()
        .network::<AnyNetwork>()
        .with_recommended_fillers()
        .on_http(alchemy_api_url);

    // Create a writer (e.g., stdout)
    let writer = Box::new(std::io::stdout());

    // Initialize the EVM Simulator
    let mut evm_simulator = EvmSimulator::new(provider, writer);

    // Simulate and trace the latest block
    evm_simulator.block_traces(BlockId::latest()).await?;

    Ok(())
}
```

## Configuration

The `EvmSimulator` struct is generic over the transport type, network type, and provider type. This allows for flexibility in choosing your Ethereum provider and network.

## Output

The simulator outputs detailed traces of each transaction in the specified block. The format of the output depends on the writer you provide to the `EvmSimulator`.
