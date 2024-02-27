use std::borrow::Cow;
use std::time::Duration;
use tokio::time::timeout;

use frame_support::sp_runtime::{AccountId32, DispatchError};
use frame_support::traits::Currency;
use log::error;
use pallet_contracts::chain_extension::{
    ChainExtension, Environment, Ext, InitState, Result as ExtResult, RetVal,
};
use pink::{
    chain_extension::{
        self as ext, HttpRequest, HttpResponse, PinkExtBackend, SigType, StorageQuotaExceeded,
    },
    dispatch_ext_call,
    types::sgx::SgxQuote,
    CacheOp, EcdhPublicKey, EcdsaPublicKey, EcdsaSignature, Hash, PinkEvent,
};
use pink_chain_extension::{DefaultPinkExtension, PinkRuntimeEnv};
use scale::Encode;

use super::{pallet_pink, PinkRuntime};
use crate::runtime::Pink as PalletPink;
use crate::types::{AccountId, ExecMode};
use pink::ConvertTo as _;

type Error = pallet_pink::Error<PinkRuntime>;

fn deposit_pink_event(contract: AccountId, event: PinkEvent) {
    let topics = [pink::PinkEvent::event_topic().into()];
    let event = super::RuntimeEvent::Contracts(pallet_contracts::Event::ContractEmitted {
        contract,
        data: event.encode(),
    });
    super::System::deposit_event_indexed(&topics[..], event);
}

environmental::environmental!(exec_mode: ExecMode);

pub(crate) fn exec_in_mode<T>(mut mode: ExecMode, f: impl FnOnce() -> T) -> T {
    exec_mode::using(&mut mode, f)
}

/// Contract extension for `pink contracts`
#[derive(Default)]
pub struct PinkExtension;

impl ChainExtension<PinkRuntime> for PinkExtension {
    fn call<E: Ext<T = PinkRuntime>>(
        &mut self,
        env: Environment<E, InitState>,
    ) -> ExtResult<RetVal> {
        let mut env = env.buf_in_buf_out();
        if env.ext_id() != 0 {
            error!(target: "pink", "Unknown extension id: {:}", env.ext_id());
            return Err(Error::UnknownChainExtensionId.into());
        }

        let address = env.ext().address().clone();
        let call_in_query = CallInQuery { address };
        let mode = exec_mode::with(|value| *value).unwrap_or(ExecMode::Query);
        let (ret, output) = if mode.is_query() {
            dispatch_ext_call!(env.func_id(), call_in_query, env)
        } else {
            let call = CallInCommand {
                as_in_query: call_in_query,
            };
            dispatch_ext_call!(env.func_id(), call, env)
        }
        .ok_or(Error::UnknownChainExtensionFunction)
        .map_err(|err| {
            error!(target: "pink", "Called an unregistered `func_id`: {:}", env.func_id());
            err
        })?;
        env.write(&output, false, None)
            .or(Err(Error::ContractIoBufferOverflow))?;
        Ok(RetVal::Converging(ret))
    }

    fn enabled() -> bool {
        true
    }
}

struct CallInQuery {
    address: AccountId,
}

impl PinkRuntimeEnv for CallInQuery {
    type AccountId = AccountId;

    fn address(&self) -> &Self::AccountId {
        &self.address
    }
}

impl CallInQuery {
    fn ensure_system(&self) -> Result<(), DispatchError> {
        let contract: AccountId32 = self.address.convert_to();
        if Some(contract) != PalletPink::system_contract() {
            return Err(DispatchError::BadOrigin);
        }
        Ok(())
    }
}

impl PinkExtBackend for CallInQuery {
    type Error = DispatchError;
    fn http_request(&self, request: HttpRequest) -> Result<HttpResponse, Self::Error> {
        DefaultPinkExtension::new(self).http_request(request)
    }

    fn batch_http_request(
        &self,
        requests: Vec<ext::HttpRequest>,
        timeout_ms: u64,
    ) -> Result<ext::BatchHttpResult, Self::Error> {
        DefaultPinkExtension::new(self).batch_http_request(requests, timeout_ms)
    }

    fn sign(
        &self,
        sigtype: SigType,
        key: Cow<[u8]>,
        message: Cow<[u8]>,
    ) -> Result<Vec<u8>, Self::Error> {
        DefaultPinkExtension::new(self).sign(sigtype, key, message)
    }

    fn verify(
        &self,
        sigtype: SigType,
        pubkey: Cow<[u8]>,
        message: Cow<[u8]>,
        signature: Cow<[u8]>,
    ) -> Result<bool, Self::Error> {
        DefaultPinkExtension::new(self).verify(sigtype, pubkey, message, signature)
    }

    fn derive_sr25519_key(&self, salt: Cow<[u8]>) -> Result<Vec<u8>, Self::Error> {
        DefaultPinkExtension::new(self).derive_sr25519_key(salt)
    }

    fn get_public_key(&self, sigtype: SigType, key: Cow<[u8]>) -> Result<Vec<u8>, Self::Error> {
        DefaultPinkExtension::new(self).get_public_key(sigtype, key)
    }

    fn cache_set(
        &self,
        key: Cow<[u8]>,
        value: Cow<[u8]>,
    ) -> Result<Result<(), StorageQuotaExceeded>, Self::Error> {
        DefaultPinkExtension::new(self).cache_set(key, value)
    }

    fn cache_set_expiration(&self, key: Cow<[u8]>, expire: u64) -> Result<(), Self::Error> {
        DefaultPinkExtension::new(self).cache_set_expiration(key, expire)
    }

    fn cache_get(&self, key: Cow<'_, [u8]>) -> Result<Option<Vec<u8>>, Self::Error> {
        DefaultPinkExtension::new(self).cache_get(key)
    }

    fn cache_remove(&self, key: Cow<'_, [u8]>) -> Result<Option<Vec<u8>>, Self::Error> {
        DefaultPinkExtension::new(self).cache_remove(key)
    }

    fn log(&self, level: u8, message: Cow<str>) -> Result<(), Self::Error> {
        DefaultPinkExtension::new(self).log(level, message)
    }

    fn getrandom(&self, length: u8) -> Result<Vec<u8>, Self::Error> {
        DefaultPinkExtension::new(self).getrandom(length)
    }

    fn is_in_transaction(&self) -> Result<bool, Self::Error> {
        Ok(false)
    }

    fn ecdsa_sign_prehashed(
        &self,
        key: Cow<[u8]>,
        message_hash: Hash,
    ) -> Result<EcdsaSignature, Self::Error> {
        DefaultPinkExtension::new(self).ecdsa_sign_prehashed(key, message_hash)
    }

    fn ecdsa_verify_prehashed(
        &self,
        signature: EcdsaSignature,
        message_hash: Hash,
        pubkey: EcdsaPublicKey,
    ) -> Result<bool, Self::Error> {
        DefaultPinkExtension::new(self).ecdsa_verify_prehashed(signature, message_hash, pubkey)
    }

    fn system_contract_id(&self) -> Result<ext::AccountId, Self::Error> {
        PalletPink::system_contract()
            .map(|address| address.convert_to())
            .ok_or(Error::SystemContractMissing.into())
    }

    fn balance_of(
        &self,
        account: ext::AccountId,
    ) -> Result<(pink::Balance, pink::Balance), Self::Error> {
        self.ensure_system()?;
        let account: AccountId32 = account.convert_to();
        let total = crate::runtime::Balances::total_balance(&account);
        let free = crate::runtime::Balances::free_balance(&account);
        Ok((total, free))
    }

    fn untrusted_millis_since_unix_epoch(&self) -> Result<u64, Self::Error> {
        DefaultPinkExtension::new(self).untrusted_millis_since_unix_epoch()
    }

    fn worker_pubkey(&self) -> Result<EcdhPublicKey, Self::Error> {
        Ok(Default::default())
    }

    fn code_exists(&self, code_hash: Hash, sidevm: bool) -> Result<bool, Self::Error> {
        if sidevm {
            Ok(PalletPink::sidevm_code_exists(&code_hash.into()))
        } else {
            Ok(helper::code_exists(&code_hash.into()))
        }
    }

    fn import_latest_system_code(
        &self,
        _payer: ext::AccountId,
    ) -> Result<Option<Hash>, Self::Error> {
        self.ensure_system()?;
        return Ok(None);
    }

    fn runtime_version(&self) -> Result<(u32, u32), Self::Error> {
        Ok(crate::version())
    }

    fn current_event_chain_head(&self) -> Result<(u64, Hash), Self::Error> {
        Ok((
            PalletPink::next_event_block_number(),
            PalletPink::last_event_block_hash().into(),
        ))
    }

    fn js_eval(
        &self,
        codes: Vec<ext::JsCode>,
        script_args: Vec<String>,
    ) -> Result<ext::JsValue, Self::Error> {
        let runtime_code = PalletPink::js_runtime();
        let mut args = vec!["phatjs".to_string()];
        for code in codes {
            match code {
                ext::JsCode::Bytecode(bytes) => {
                    args.push("-b".to_string());
                    args.push(hex::encode(bytes));
                }
                ext::JsCode::Source(src) => {
                    args.push("-c".to_string());
                    args.push(src);
                }
            }
        }
        args.push("--".to_string());
        args.extend(script_args);
        let vital_capacity = 100_000_000_000_u64;
        let max_memory_pages = 256;
        let run = crate::sidevm_runner::run(vital_capacity, max_memory_pages, runtime_code, args);
        let run = async { timeout(Duration::from_secs(10), run).await };
        match crate::blocking::block_on(run) {
            Ok(Ok(value)) => Ok(value),
            Ok(Err(err)) => Ok(ext::JsValue::Exception(format!("{:?}", err))),
            Err(_) => Ok(ext::JsValue::Exception(
                "Sidevm execution timeout".to_string(),
            )),
        }
    }

    fn worker_sgx_quote(&self) -> Result<Option<SgxQuote>, Self::Error> {
        pink_chain_extension::mock_ext::MockExtension
            .worker_sgx_quote()
            .map_err(|_| "No SGX quote".into())
    }
}

struct CallInCommand {
    as_in_query: CallInQuery,
}

/// This implementation is used when calling the extension in a command.
/// # NOTE FOR IMPLEMENTORS
/// Make sure the return values are deterministic.
impl PinkExtBackend for CallInCommand {
    type Error = DispatchError;

    fn http_request(&self, _request: HttpRequest) -> Result<HttpResponse, Self::Error> {
        Ok(HttpResponse {
            status_code: 523,
            reason_phrase: "API Unavailable".into(),
            headers: vec![],
            body: vec![],
        })
    }
    fn batch_http_request(
        &self,
        _requests: Vec<ext::HttpRequest>,
        _timeout_ms: u64,
    ) -> Result<ext::BatchHttpResult, Self::Error> {
        Ok(Err(ext::HttpRequestError::NotAllowed))
    }
    fn sign(
        &self,
        sigtype: SigType,
        key: Cow<[u8]>,
        message: Cow<[u8]>,
    ) -> Result<Vec<u8>, Self::Error> {
        if matches!(sigtype, SigType::Sr25519) {
            return Ok(vec![]);
        }
        self.as_in_query.sign(sigtype, key, message)
    }

    fn verify(
        &self,
        sigtype: SigType,
        pubkey: Cow<[u8]>,
        message: Cow<[u8]>,
        signature: Cow<[u8]>,
    ) -> Result<bool, Self::Error> {
        self.as_in_query.verify(sigtype, pubkey, message, signature)
    }

    fn derive_sr25519_key(&self, salt: Cow<[u8]>) -> Result<Vec<u8>, Self::Error> {
        self.as_in_query.derive_sr25519_key(salt)
    }

    fn get_public_key(&self, sigtype: SigType, key: Cow<[u8]>) -> Result<Vec<u8>, Self::Error> {
        self.as_in_query.get_public_key(sigtype, key)
    }

    fn cache_set(
        &self,
        key: Cow<[u8]>,
        value: Cow<[u8]>,
    ) -> Result<Result<(), StorageQuotaExceeded>, Self::Error> {
        deposit_pink_event(
            self.as_in_query.address.clone(),
            PinkEvent::CacheOp(CacheOp::Set {
                key: key.into_owned(),
                value: value.into_owned(),
            }),
        );
        Ok(Ok(()))
    }

    fn cache_set_expiration(&self, key: Cow<[u8]>, expiration: u64) -> Result<(), Self::Error> {
        deposit_pink_event(
            self.as_in_query.address.clone(),
            PinkEvent::CacheOp(CacheOp::SetExpiration {
                key: key.into_owned(),
                expiration,
            }),
        );
        Ok(())
    }

    fn cache_get(&self, _key: Cow<[u8]>) -> Result<Option<Vec<u8>>, Self::Error> {
        Ok(None)
    }

    fn cache_remove(&self, key: Cow<[u8]>) -> Result<Option<Vec<u8>>, Self::Error> {
        deposit_pink_event(
            self.as_in_query.address.clone(),
            PinkEvent::CacheOp(CacheOp::Remove {
                key: key.into_owned(),
            }),
        );
        Ok(None)
    }

    fn log(&self, level: u8, message: Cow<str>) -> Result<(), Self::Error> {
        self.as_in_query.log(level, message)
    }

    fn getrandom(&self, _length: u8) -> Result<Vec<u8>, Self::Error> {
        Ok(vec![])
    }

    fn is_in_transaction(&self) -> Result<bool, Self::Error> {
        Ok(true)
    }

    fn ecdsa_sign_prehashed(
        &self,
        key: Cow<[u8]>,
        message_hash: Hash,
    ) -> Result<EcdsaSignature, Self::Error> {
        self.as_in_query.ecdsa_sign_prehashed(key, message_hash)
    }

    fn ecdsa_verify_prehashed(
        &self,
        signature: EcdsaSignature,
        message_hash: Hash,
        pubkey: EcdsaPublicKey,
    ) -> Result<bool, Self::Error> {
        self.as_in_query
            .ecdsa_verify_prehashed(signature, message_hash, pubkey)
    }

    fn system_contract_id(&self) -> Result<ext::AccountId, Self::Error> {
        self.as_in_query.system_contract_id()
    }

    fn balance_of(
        &self,
        account: ext::AccountId,
    ) -> Result<(pink::Balance, pink::Balance), Self::Error> {
        self.as_in_query.balance_of(account)
    }

    fn untrusted_millis_since_unix_epoch(&self) -> Result<u64, Self::Error> {
        Ok(0)
    }

    fn worker_pubkey(&self) -> Result<EcdhPublicKey, Self::Error> {
        Ok(Default::default())
    }

    fn code_exists(&self, code_hash: Hash, sidevm: bool) -> Result<bool, Self::Error> {
        self.as_in_query.code_exists(code_hash, sidevm)
    }

    fn import_latest_system_code(
        &self,
        payer: ext::AccountId,
    ) -> Result<Option<Hash>, Self::Error> {
        self.as_in_query.import_latest_system_code(payer)
    }

    fn runtime_version(&self) -> Result<(u32, u32), Self::Error> {
        self.as_in_query.runtime_version()
    }

    fn current_event_chain_head(&self) -> Result<(u64, Hash), Self::Error> {
        self.as_in_query.current_event_chain_head()
    }

    fn js_eval(
        &self,
        _codes: Vec<ext::JsCode>,
        _args: Vec<String>,
    ) -> Result<ext::JsValue, Self::Error> {
        return Ok(ext::JsValue::Exception(
            "js_eval is not supported".to_string(),
        ));
    }

    fn worker_sgx_quote(&self) -> Result<Option<SgxQuote>, Self::Error> {
        Ok(None)
    }
}

pub mod helper {
    use crate::types::Hash;
    use scale::Encode;
    use sp_core::hashing::twox_128;

    pub fn code_exists(code_hash: &Hash) -> bool {
        let key = code_owner_key(code_hash);
        frame_support::storage::unhashed::exists(&key)
    }

    fn code_owner_key(code_hash: &Hash) -> Vec<u8> {
        let mut key = Vec::new();
        key.extend(twox_128("Contracts".as_bytes()));
        key.extend(twox_128("CodeInfoOf".as_bytes()));
        key.extend(&code_hash.encode());
        key
    }
}
