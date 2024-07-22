use alloy::eips::BlockId;
use alloy::network::Ethereum;
use alloy::providers::ProviderBuilder;
use anyhow::Result;
use evm_simulator::EvmSimulator;
use url::Url;

#[tokio::main]
async fn main() -> Result<()> {
    dotenv::dotenv().ok();

    let alchemy_api_endpoint = std::env::var("ALCHEMY_API_ENDPOINT")?;
    let alchemy_api_key = std::env::var("ALCHEMY_API_KEY")?;
    let alchemy_api_url =
        Url::parse(format!("{}/{}", alchemy_api_endpoint, alchemy_api_key).as_str())?;

    let provider = ProviderBuilder::new()
        .network::<Ethereum>()
        .with_recommended_fillers()
        .on_http(alchemy_api_url);

    let writer = Box::new(std::io::stdout());

    let mut evm_simulator = EvmSimulator::new(provider, writer);

    evm_simulator.block_traces(BlockId::latest()).await?;

    Ok(())
}
