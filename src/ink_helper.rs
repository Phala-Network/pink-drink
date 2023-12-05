use crate::{types::ExecMode, PinkRuntime, Result};

use ::ink::{
    env::{
        call::{
            utils::{ReturnType, Set, Unset},
            Call, CallBuilder, CreateBuilder, ExecutionInput, FromAccountId,
        },
        Environment,
    },
    primitives::Hash,
};
use drink::{
    errors::MessageResult,
    runtime::{AccountIdFor, Runtime},
    session::Session,
    ContractBundle,
};
use pink_extension::Balance;
use scale::{Decode, Encode};

type PinkSession = Session<PinkRuntime>;
type AccountId = AccountIdFor<PinkRuntime>;

const DEFAULT_GAS_LIMIT: u64 = 1_000_000_000_000_000;

pub trait SessionExt {
    fn actor(&mut self) -> AccountId;
    fn query<T>(&mut self, f: impl FnOnce() -> T) -> T;
    fn tx<T>(&mut self, f: impl FnOnce() -> T) -> T;
    fn set_driver<A: Encode>(&mut self, name: &str, contract: &A) -> Result<()>;
}

impl SessionExt for PinkSession {
    fn actor(&mut self) -> AccountId {
        let actor = self.set_actor(AccountId::new(Default::default()));
        self.set_actor(actor.clone());
        actor
    }
    fn query<T>(&mut self, f: impl FnOnce() -> T) -> T {
        PinkRuntime::execute_in_mode(ExecMode::Query, || {
            self.sandbox().dry_run(|sandbox| sandbox.execute_with(f))
        })
    }
    fn tx<T>(&mut self, f: impl FnOnce() -> T) -> T {
        PinkRuntime::execute_in_mode(ExecMode::Transaction, || self.sandbox().execute_with(f))
    }
    fn set_driver<A: Encode>(&mut self, name: &str, contract: &A) -> Result<()> {
        let caller = self.actor();
        self.tx(|| {
            let system_address =
                crate::runtime::Pink::system_contract().expect("System contract not found");
            let selector_set_driver = 0xaa1e2030u32.to_be_bytes();
            let input_data = (selector_set_driver, name, contract).encode();
            PinkRuntime::call(
                caller,
                system_address.clone(),
                0,
                u64::MAX,
                None,
                input_data,
                true,
            )
            .map(|_| ())
            .map_err(|err| format!("FailedToCallSetDriver: {err:?}").into())
        })
    }
}

pub trait DeployBundle {
    type Contract;
    fn deploy_bundle(
        self,
        bundle: &ContractBundle,
        session: &mut PinkSession,
    ) -> Result<Self::Contract>;
}
pub trait Deployable {
    type Contract;
    fn deploy(self, session: &mut PinkSession) -> Result<Self::Contract>;
}

pub trait Callable {
    type Ret;
    fn submit_tx(self, session: &mut PinkSession) -> Result<Self::Ret>;
    fn query(self, session: &mut PinkSession) -> Result<Self::Ret>;
}

impl<Env, Contract, Args, Salt> DeployBundle
    for CreateBuilder<
        Env,
        Contract,
        Unset<Hash>,
        Unset<u64>,
        Unset<Balance>,
        Set<ExecutionInput<Args>>,
        Set<Salt>,
        Set<ReturnType<Contract>>,
    >
where
    Env: Environment<Hash = Hash, Balance = Balance>,
    Contract: FromAccountId<Env>,
    Args: Encode,
    Salt: AsRef<[u8]>,
{
    type Contract = Contract;

    fn deploy_bundle(
        self,
        bundle: &ContractBundle,
        session: &mut PinkSession,
    ) -> Result<Self::Contract> {
        let caller = session.actor();
        let code_hash =
            session.tx(|| PinkRuntime::upload_code(caller.clone(), bundle.wasm.clone(), true))?;
        self.code_hash(code_hash.0.into()).deploy(session)
    }
}
impl<Env, Contract, Args> DeployBundle
    for CreateBuilder<
        Env,
        Contract,
        Unset<Hash>,
        Unset<u64>,
        Unset<Balance>,
        Set<ExecutionInput<Args>>,
        Unset<ink::env::call::state::Salt>,
        Set<ReturnType<Contract>>,
    >
where
    Env: Environment<Hash = Hash, Balance = Balance>,
    Contract: FromAccountId<Env>,
    Args: Encode,
{
    type Contract = Contract;
    fn deploy_bundle(
        self,
        bundle: &ContractBundle,
        session: &mut PinkSession,
    ) -> Result<Self::Contract> {
        self.salt_bytes(Vec::new()).deploy_bundle(bundle, session)
    }
}

impl<Env, Contract, Args, Salt> Deployable
    for CreateBuilder<
        Env,
        Contract,
        Set<Hash>,
        Unset<u64>,
        Unset<Balance>,
        Set<ExecutionInput<Args>>,
        Set<Salt>,
        Set<ReturnType<Contract>>,
    >
where
    Env: Environment<Hash = Hash, Balance = Balance>,
    Contract: FromAccountId<Env>,
    Args: Encode,
    Salt: AsRef<[u8]>,
{
    type Contract = Contract;

    fn deploy(self, session: &mut PinkSession) -> Result<Self::Contract> {
        let caller = session.actor();
        let constructor = self.endowment(0).gas_limit(DEFAULT_GAS_LIMIT);
        let params = constructor.params();
        let code_hash: &[u8] = params.code_hash().as_ref();
        let code_hash = sp_core::H256(code_hash.try_into().expect("Hash convert failed"));
        let input_data = params.exec_input().encode();

        let account_id = session.tx(|| {
            PinkRuntime::instantiate(
                caller,
                0,
                params.gas_limit(),
                None,
                code_hash.into(),
                input_data,
                params.salt_bytes().as_ref().to_vec(),
            )
        })?;
        Ok(Contract::from_account_id(
            Decode::decode(&mut &account_id.encode()[..]).expect("Failed to decode account id"),
        ))
    }
}

impl<Env, Args: Encode, Ret: Decode> Callable
    for CallBuilder<Env, Set<Call<Env>>, Set<ExecutionInput<Args>>, Set<ReturnType<Ret>>>
where
    Env: Environment,
    Ret: Decode,
    Args: Encode,
{
    type Ret = Ret;

    fn submit_tx(self, session: &mut PinkSession) -> Result<Self::Ret> {
        session.tx(|| call(self, true))
    }

    fn query(self, session: &mut PinkSession) -> Result<Self::Ret> {
        session.query(|| call(self, false))
    }
}

fn call<Env, Args, Ret>(
    call_builder: CallBuilder<Env, Set<Call<Env>>, Set<ExecutionInput<Args>>, Set<ReturnType<Ret>>>,
    deterministic: bool,
) -> Result<Ret>
where
    Env: Environment,
    Args: Encode,
    Ret: Decode,
{
    let origin = PinkRuntime::default_actor();
    let params = call_builder.params();
    let data = params.exec_input().encode();
    let callee = params.callee();
    let address: [u8; 32] = callee.as_ref().try_into().or(Err("Invalid callee"))?;

    let result = PinkRuntime::call(
        origin,
        address.into(),
        0,
        DEFAULT_GAS_LIMIT,
        None,
        data,
        deterministic,
    )?;
    let ret = MessageResult::<Ret>::decode(&mut &result[..])
        .map_err(|e| format!("Failed to decode result: {}", e))?
        .map_err(|e| format!("Failed to execute call: {}", e))?;
    Ok(ret)
}
