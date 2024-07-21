use alloy::network::AnyNetwork;
use alloy::primitives::{Address, Bytes, TxKind, U64};
use alloy::providers::Provider;
use alloy::rpc::types::TransactionRequest;
use alloy::transports::Transport;
use anyhow::{anyhow, Result};
use foundry_fork_db::{cache::BlockchainDbMeta, BlockchainDb, SharedBackend};
use revm::{
    db::CacheDB,
    primitives::{ExecutionResult, Output, TransactTo},
    Evm,
};
use std::cell::RefCell;
use std::marker::Unpin;

pub struct EvmSimulator<'a, T, P> {
    pub provider: P,
    pub evm: RefCell<Evm<'a, (), CacheDB<SharedBackend>>>,
    pub owner: Address,
    pub block_number: U64,
    _pd: std::marker::PhantomData<T>,
}

impl<'a, T, P> EvmSimulator<'a, T, P>
where
    T: Transport + Clone + Unpin,
    P: Provider<T, AnyNetwork> + Clone + Unpin + 'static,
{
    pub fn new(provider: P, owner: Address, block_number: U64) -> Self {
        let shared_backend = SharedBackend::spawn_backend_thread(
            provider.clone(),
            BlockchainDb::new(BlockchainDbMeta::new(Default::default(), "".to_string()), None),
            Some(block_number.into()),
        );
        let db = CacheDB::new(shared_backend);
        let evm = Evm::builder().with_db(db).build();
        let evm = RefCell::new(evm);
        Self { provider, evm, owner, block_number, _pd: std::marker::PhantomData }
    }

    pub fn call(&self, tx: TransactionRequest) -> Result<TxResult> {
        self._call(tx, true)
    }

    pub fn staticcall(&self, tx: TransactionRequest) -> Result<TxResult> {
        self._call(tx, false)
    }

    fn _call(&self, tx: TransactionRequest, commit: bool) -> Result<TxResult> {
        let mut evm = self.evm.borrow_mut();
        evm.context.evm.env.tx.caller = tx.from.unwrap_or(self.owner);
        let to = match tx.to.unwrap_or_default() {
            TxKind::Call(to) => to,
            TxKind::Create => Address::default(),
        };
        evm.context.evm.env.tx.transact_to = TransactTo::Call(to);
        evm.context.evm.env.tx.data = tx.input.data.unwrap_or_default();
        evm.context.evm.env.tx.value = tx.value.unwrap_or_default();
        evm.context.evm.env.tx.gas_limit = 5000000;

        let result = if commit {
            match evm.transact_commit() {
                Ok(result) => result,
                Err(e) => return Err(anyhow!("EVM call failed: {:?}", e)),
            }
        } else {
            let ref_tx = evm.transact().map_err(|e| anyhow!("EVM staticcall failed: {:?}", e))?;
            ref_tx.result
        };

        let output = match result {
            ExecutionResult::Success { gas_used, gas_refunded, output, .. } => match output {
                Output::Call(o) => TxResult { output: o.into(), gas_used, gas_refunded },
                Output::Create(o, _) => TxResult { output: o.into(), gas_used, gas_refunded },
            },
            ExecutionResult::Revert { gas_used, output } => {
                return Err(anyhow!("EVM REVERT: {:?} / Gas used: {:?}", output, gas_used))
            }
            ExecutionResult::Halt { reason, .. } => return Err(anyhow!("EVM HALT: {:?}", reason)),
        };

        Ok(output)
    }
}

#[derive(Debug, Clone)]
pub struct TxResult {
    pub output: Bytes,
    pub gas_used: u64,
    pub gas_refunded: u64,
}
