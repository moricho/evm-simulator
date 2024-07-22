use alloy::eips::BlockNumberOrTag;
use alloy::network::AnyNetwork;
use alloy::primitives::{Address, Bytes, TxKind};
use alloy::providers::{ext::TraceApi, Provider};
use alloy::rpc::types::{
    trace::parity::{TraceResults, TraceType},
    TransactionRequest,
};
use alloy::serde::WithOtherFields;
use alloy::transports::Transport;
use anyhow::{anyhow, Result};
use foundry_fork_db::{cache::BlockchainDbMeta, BlockchainDb, SharedBackend};
use revm::{
    db::CacheDB,
    primitives::{ExecutionResult, Output, TransactTo},
    Evm,
};
use std::marker::Unpin;

#[derive(Debug, Clone)]
pub struct TxResult {
    pub output: Bytes,
    pub gas_used: u64,
    pub gas_refunded: u64,
}

#[derive(Debug, Clone)]
pub struct TxResultWithTrace {
    pub result: TxResult,
    pub trace: TraceResults,
}

pub struct EvmSimulator<'a, T, P> {
    pub provider: P,
    pub evm: Evm<'a, (), CacheDB<SharedBackend>>,
    pub block_number: BlockNumberOrTag,
    _pd: std::marker::PhantomData<T>,
}

impl<'a, T, P> EvmSimulator<'a, T, P>
where
    T: Transport + Clone + Unpin,
    P: Provider<T, AnyNetwork> + Clone + Unpin + 'static,
{
    pub fn new(provider: P, block_number: BlockNumberOrTag) -> Self {
        let shared_backend = SharedBackend::spawn_backend_thread(
            provider.clone(),
            BlockchainDb::new(BlockchainDbMeta::new(Default::default(), "".to_string()), None),
            Some(block_number.into()),
        );
        let db = CacheDB::new(shared_backend);
        let evm = Evm::builder().with_db(db).build();
        Self { provider, evm, block_number, _pd: std::marker::PhantomData }
    }

    pub fn call(&mut self, tx: WithOtherFields<TransactionRequest>) -> Result<TxResult> {
        self.call_inner(tx, true)
    }

    pub fn staticcall(&mut self, tx: WithOtherFields<TransactionRequest>) -> Result<TxResult> {
        self.call_inner(tx, false)
    }

    pub async fn call_with_trace(
        &mut self,
        tx: WithOtherFields<TransactionRequest>,
        trace_types: Vec<TraceType>,
    ) -> Result<TxResultWithTrace> {
        let trace = self
            .provider
            .trace_call(&tx, &trace_types)
            .await
            .map_err(|e| anyhow!("Failed to trace call: {:?}", e))?;
        let result = self.call(tx)?;
        Ok(TxResultWithTrace { result, trace })
    }

    fn call_inner(
        &mut self,
        tx: WithOtherFields<TransactionRequest>,
        commit: bool,
    ) -> Result<TxResult> {
        self.evm.context.evm.env.tx.caller = tx.from.unwrap_or_default();
        let to = match tx.to.unwrap_or_default() {
            TxKind::Call(to) => to,
            TxKind::Create => Address::default(),
        };
        self.evm.context.evm.env.tx.transact_to = TransactTo::Call(to);
        self.evm.context.evm.env.tx.data = tx.input.data.clone().unwrap_or_default();
        self.evm.context.evm.env.tx.value = tx.value.unwrap_or_default();
        self.evm.context.evm.env.tx.gas_limit = 5000000;

        let result = if commit {
            match self.evm.transact_commit() {
                Ok(result) => result,
                Err(e) => return Err(anyhow!("EVM call failed: {:?}", e)),
            }
        } else {
            let ref_tx =
                self.evm.transact().map_err(|e| anyhow!("EVM staticcall failed: {:?}", e))?;
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
