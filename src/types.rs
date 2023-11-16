use frame_support::sp_runtime;

use sp_runtime::{traits::BlakeTwo256, AccountId32};

pub use frame_support::weights::Weight;

pub type Hash = sp_core::H256;
pub type Hashing = BlakeTwo256;
pub type AccountId = AccountId32;
pub type Balance = u128;
pub type BlockNumber = u32;
pub type Nonce = u64;

#[derive(Debug, Clone, PartialEq, Eq, Copy)]
pub enum ExecMode {
    Query,
    Transaction,
}

impl ExecMode {
    pub fn is_query(&self) -> bool {
        match self {
            ExecMode::Query => true,
            _ => false,
        }
    }
}
