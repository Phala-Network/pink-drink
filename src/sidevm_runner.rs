use anyhow::{Context, Result};
use log::error;
use pink_extension::chain_extension::JsValue;
use scale::Decode;
use sidevm_host_runtime::{
    CacheOps, DynCacheOps, OcallError, OutgoingRequest, WasmEngine, WasmInstanceConfig,
};

pub async fn run(
    vital_capacity: u64,
    max_memory_pages: u32,
    code: Vec<u8>,
    args: Vec<String>,
) -> Result<JsValue> {
    let (event_tx, mut event_rx) = tokio::sync::mpsc::channel(1);
    let config = WasmInstanceConfig {
        max_memory_pages,
        gas_per_breath: vital_capacity,
        cache_ops: no_cache(),
        scheduler: None,
        weight: 0,
        id: Default::default(),
        event_tx,
        log_handler: None,
    };
    let engine = WasmEngine::new();
    let module = engine.compile(&code)?;
    let (mut wasm_run, _env) = module
        .run(args, config)
        .context("Failed to start sidevm instance")?;
    let mut output = None;
    tokio::select! {
        rv = &mut wasm_run => {
            if let Err(err) = rv {
                error!(target: "sidevm", "Js runtime exited with error: {err:?}");
            }
        }
        _ = async {
            while let Some((_vmid, event)) = event_rx.recv().await {
                if let OutgoingRequest::Output(output_bytes) = event {
                    output = Some(output_bytes);
                    break;
                }
            }
        } => {}
    }
    if output.is_none() {
        while let Ok((_vmid, event)) = event_rx.try_recv() {
            if let OutgoingRequest::Output(output_bytes) = event {
                output = Some(output_bytes);
                break;
            }
        }
    }
    match output {
        Some(output) => Ok(JsValue::decode(&mut &output[..])?),
        None => Err(anyhow::anyhow!("No output")),
    }
}

fn no_cache() -> DynCacheOps {
    struct Ops;
    type OpResult<T> = Result<T, OcallError>;
    impl CacheOps for Ops {
        fn get(&self, _contract: &[u8], _key: &[u8]) -> OpResult<Option<Vec<u8>>> {
            Ok(None)
        }
        fn set(&self, _contract: &[u8], _key: &[u8], _value: &[u8]) -> OpResult<()> {
            Ok(())
        }
        fn set_expiration(
            &self,
            _contract: &[u8],
            _key: &[u8],
            _expire_after_secs: u64,
        ) -> OpResult<()> {
            Ok(())
        }
        fn remove(&self, _contract: &[u8], _key: &[u8]) -> OpResult<Option<Vec<u8>>> {
            Ok(None)
        }
    }
    &Ops
}
