use crate::gate::{ColonyReputationClient, ColonyTokenClient};
use async_trait::async_trait;
use colony_rs::{get_colony_name, get_domain_count, get_reputation_in_domain};

#[derive(Debug)]
pub struct ColonyClient;

impl ColonyClient {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl ColonyReputationClient for ColonyClient {
    async fn get_reputation_in_domain(
        &self,
        colony_address: &colony_rs::H160,
        wallet_address: &colony_rs::H160,
        domain: u64,
    ) -> anyhow::Result<colony_rs::ReputationNoProof> {
        Ok(get_reputation_in_domain(colony_address, wallet_address, domain).await?)
    }

    async fn get_colony_name(&self, colony_address: &colony_rs::H160) -> anyhow::Result<String> {
        Ok(get_colony_name(*colony_address).await?)
    }

    async fn get_domain_count(&self, colony_address: &colony_rs::H160) -> anyhow::Result<u64> {
        Ok(get_domain_count(*colony_address).await?)
    }
}

#[async_trait]
impl ColonyTokenClient for ColonyClient {
    async fn balance_of(
        &self,
        token_address: &colony_rs::H160,
        wallet_address: &colony_rs::H160,
    ) -> anyhow::Result<colony_rs::U256> {
        Ok(colony_rs::balance_off(token_address, wallet_address).await?)
    }

    async fn get_token_decimals(&self, token_address: &colony_rs::H160) -> anyhow::Result<u8> {
        Ok(colony_rs::get_token_decimals(*token_address).await?)
    }

    async fn get_token_symbol(&self, token_address: &colony_rs::H160) -> anyhow::Result<String> {
        Ok(colony_rs::get_token_symbol(*token_address).await?)
    }
}
