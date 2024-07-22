use alloy::eips::BlockNumberOrTag;
use alloy::network::AnyNetwork;
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
        .network::<AnyNetwork>()
        .with_recommended_fillers()
        .on_http(alchemy_api_url);

    let _evm_simulator = EvmSimulator::new(provider, BlockNumberOrTag::Latest);

    Ok(())
}
