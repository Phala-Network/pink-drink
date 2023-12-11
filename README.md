# drink-pink-runtime

## Overview

`drink-pink-runtime` is a runtime implementation for drink framework for Pink contract environment. It extends Drink functionalities, offering a more convenient approach to writing strongly-typed tests for Pink contracts. This crate simplifies testing by simulating contract deployment, transaction execution, and queries in a Pink contract environment.

## Installation

To include `drink-pink-runtime` in your project, add it to your `Cargo.toml`:

```toml
[dependencies]
pink-extension = "0.5"
drink = "0.8.0"
drink-pink-runtime = "1.2"
```

## Usage

Here's an example demonstrating the basic usage of `drink-pink-runtime`. This includes setting up a test environment, deploying a contract bundle, and simulating transactions and queries.

```rust
#[cfg(test)]
mod tests {
    use drink_pink_runtime::{PinkRuntime, SessionExt, DeployBundle, Callable};
    use drink::session::Session;
    use super::YourContractRef;

    // This would compile all contracts dependended by your contract
    #[drink::contract_bundle_provider]
    enum BundleProvider {}

    #[test]
    fn example_test() -> Result<(), Box<dyn std::error::Error>> {
        let mut session = Session::<PinkRuntime>::new()?;

        // Deploy a contract bundle
        let contract_ref = YourContractRef::new().deploy_bundle(&BundleProvider::local()?, &mut session)?;

        // Set the deployed contract as a driver
        session.set_driver("YourDriverName", &contract_ref)?;

        // Simulate a transaction
        contract_ref.call_mut().your_transaction_method().submit_tx(&mut session)?;

        // Simulate a query
        let query_result = contract_ref.call().your_query_method().query(&mut session)?;

        Ok(())
    }
}
```
