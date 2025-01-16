use std::{ops::Deref, sync::Arc};

use async_trait::async_trait;
use builder_server::{builder::Builder, FullPayloadContents};
use ethereum_apis_common::{custom_internal_err, ErrorResponse};
use execution_layer::test_utils::MockBuilder;
use types::{
    builder_bid::SignedBuilderBid, ChainSpec, EthSpec, ExecutionBlockHash, ForkName,
    PublicKeyBytes, SignedBlindedBeaconBlock, SignedValidatorRegistrationData, Slot,
};

#[derive(Clone)]
pub struct RusticBuilder<E: EthSpec> {
    builder: MockBuilder<E>,
    spec: Arc<ChainSpec>,
}

impl<E: EthSpec> RusticBuilder<E> {
    pub fn new(builder: MockBuilder<E>, spec: Arc<ChainSpec>) -> Self {
        Self { builder, spec }
    }
}

impl<E: EthSpec> Deref for RusticBuilder<E> {
    type Target = MockBuilder<E>;
    fn deref(&self) -> &Self::Target {
        &self.builder
    }
}

impl<E: EthSpec> AsRef<RusticBuilder<E>> for RusticBuilder<E> {
    fn as_ref(&self) -> &RusticBuilder<E> {
        self
    }
}

#[async_trait]
impl<E: EthSpec> Builder<E> for RusticBuilder<E> {
    fn fork_name_at_slot(&self, slot: Slot) -> ForkName {
        self.spec.fork_name_at_slot::<E>(slot)
    }

    async fn register_validators(
        &self,
        registrations: Vec<SignedValidatorRegistrationData>,
    ) -> Result<(), ErrorResponse> {
        tracing::info!("Registering validators, count: {}", registrations.len());
        self.builder
            .register_validators(registrations)
            .await
            .map_err(custom_internal_err)
    }

    async fn get_header(
        &self,
        slot: Slot,
        parent_hash: ExecutionBlockHash,
        pubkey: PublicKeyBytes,
    ) -> Result<SignedBuilderBid<E>, ErrorResponse> {
        tracing::info!(
            "Getting header for slot {}, parent_hash: {}, pubkey: {:?}",
            slot,
            parent_hash,
            pubkey
        );
        self.builder
            .get_header(slot, parent_hash, pubkey)
            .await
            .map_err(custom_internal_err)
    }

    async fn submit_blinded_block(
        &self,
        signed_block: SignedBlindedBeaconBlock<E>,
    ) -> Result<FullPayloadContents<E>, ErrorResponse> {
        tracing::info!(
            "Submitting signed blinded block to builder, slot: {}, root: {}",
            signed_block.message().slot(),
            signed_block.canonical_root(),
        );
        self.builder
            .submit_blinded_block(signed_block)
            .await
            .map_err(custom_internal_err)
    }
}
