use alloy::network::Network;
use alloy::providers::Provider;
use alloy::rpc::types::{BlockId, BlockTransactionsKind};
use alloy::transports::Transport;
use revm::db::{AlloyDB, CacheDB, StateBuilder};
use revm::inspectors::TracerEip3155;
use revm::primitives::{AccessListItem, TxKind, B256, U256};
use revm::{inspector_handle_register, Evm};
use std::io::{Result as IoResult, Write};
use std::sync::{Arc, Mutex};

struct Writer(Arc<Mutex<Box<dyn Write + Send + 'static>>>);

impl Write for Writer {
    fn write(&mut self, buf: &[u8]) -> IoResult<usize> {
        self.0.lock().unwrap().write(buf)
    }

    fn flush(&mut self) -> IoResult<()> {
        self.0.lock().unwrap().flush()
    }
}

pub struct EvmSimulator<T, N, P> {
    pub provider: P,
    writer: Arc<Mutex<Box<dyn Write + Send + 'static>>>,
    _pd: std::marker::PhantomData<(T, N)>,
}

impl<T, N, P> EvmSimulator<T, N, P>
where
    T: Transport + Clone,
    N: Network,
    P: Provider<T, N> + Clone + 'static,
{
    pub fn new(provider: P, writer: Box<dyn Write + Send + 'static>) -> Self {
        Self { provider, writer: Arc::new(Mutex::new(writer)), _pd: std::marker::PhantomData }
    }

    pub async fn block_traces(&mut self, block_id: BlockId) -> anyhow::Result<()> {
        let provider = self.provider.clone();

        // Fetch the block with full transactions
        let block = match provider.get_block(block_id, BlockTransactionsKind::Full).await {
            Ok(Some(block)) => block,
            Ok(None) => anyhow::bail!("Block not found"),
            Err(error) => anyhow::bail!("Error: {:?}", error),
        };
        let block_num = block.header.number.expect("Block number not found");
        println!("Fetched block number: {}", block_num);

        let prev_block_number = block_num - 1;
        let chain_id = provider.get_chain_id().await.expect("Failed to get chain id");

        // Use the previous block state as the db with caching
        let prev_block_id: BlockId = prev_block_number.into();
        let state_db =
            AlloyDB::new(provider.clone(), prev_block_id).expect("Failed to create AlloyDB");
        let cache_db: CacheDB<AlloyDB<T, N, P>> = CacheDB::new(state_db);
        let mut state = StateBuilder::new_with_database(cache_db).build();

        let writer = Writer(self.writer.clone());
        let mut evm = Evm::builder()
            .with_db(&mut state)
            .with_external_context(TracerEip3155::new(Box::new(writer)))
            .modify_block_env(|b| {
                if let Some(number) = block.header.number {
                    b.number = U256::from(number);
                }
                b.coinbase = block.header.miner;
                b.timestamp = U256::from(block.header.timestamp);
                b.difficulty = U256::from(block.header.difficulty);
                b.gas_limit = U256::from(block.header.gas_limit);
                if let Some(base_fee) = block.header.base_fee_per_gas {
                    b.basefee = U256::from(base_fee);
                }
            })
            .modify_cfg_env(|c| {
                c.chain_id = chain_id;
            })
            .append_handler_register(inspector_handle_register)
            .build();

        let txs = block.transactions.len();
        println!("Found {txs} transactions.");

        for tx in block.transactions.into_transactions() {
            evm = evm
                .modify()
                .modify_tx_env(|etx| {
                    etx.caller = tx.from;
                    etx.gas_limit = tx.gas as u64;
                    if let Some(gas_price) = tx.gas_price {
                        etx.gas_price = U256::from(gas_price);
                    }
                    etx.value = U256::from(tx.value);
                    etx.data = tx.input.0.into();
                    let mut gas_priority_fee = U256::ZERO;
                    if let Some(max_priority_fee_per_gas) = tx.max_priority_fee_per_gas {
                        gas_priority_fee = U256::from(max_priority_fee_per_gas);
                    }
                    etx.gas_priority_fee = Some(gas_priority_fee);
                    etx.chain_id = Some(chain_id);
                    etx.nonce = Some(tx.nonce);
                    if let Some(access_list) = tx.access_list {
                        etx.access_list = access_list
                            .0
                            .into_iter()
                            .map(|item| {
                                let storage_keys: Vec<B256> = item
                                    .storage_keys
                                    .into_iter()
                                    .map(|h256| B256::new(h256.0))
                                    .collect();

                                AccessListItem { address: item.address, storage_keys }
                            })
                            .collect();
                    } else {
                        etx.access_list = Default::default();
                    }

                    etx.transact_to = match tx.to {
                        Some(to_address) => TxKind::Call(to_address),
                        None => TxKind::Create,
                    };
                })
                .build();

            // Inspect and commit the transaction to the EVM
            if let Err(error) = evm.transact_commit() {
                println!("Got error: {:?}", error);
            }
        }

        Ok(())
    }
}
