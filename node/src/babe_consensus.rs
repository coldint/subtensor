use crate::{
    client::{FullBackend, FullClient},
    conditional_evm_block_import::ConditionalEVMBlockImport,
    ethereum::EthConfiguration,
    service::{BIQ, ConsensusMechanism, GrandpaBlockImport, StartAuthoringParams},
};
use fc_consensus::FrontierBlockImport;
use node_subtensor_runtime::opaque::Block;
use sc_client_api::{AuxStore, BlockOf};
use sc_consensus::{BlockImport, BoxBlockImport};
use sc_consensus_babe::{BabeLink, BabeWorkerHandle};
use sc_consensus_grandpa::BlockNumberOps;
use sc_consensus_slots::{BackoffAuthoringBlocksStrategy, InherentDataProviderExt};
use sc_service::{Configuration, TaskManager};
use sc_telemetry::TelemetryHandle;
use sc_transaction_pool::TransactionPoolHandle;
use sc_transaction_pool_api::OffchainTransactionPoolFactory;
use sp_api::ProvideRuntimeApi;
use sp_blockchain::{HeaderBackend, HeaderMetadata};
use sp_consensus::{Environment, Proposer, SelectChain, SyncOracle};
use sp_consensus_aura::AuraApi;
use sp_consensus_aura::sr25519::AuthorityId;
use sp_consensus_babe::BabeApi;
use sp_consensus_slots::SlotDuration;
use sp_inherents::CreateInherentDataProviders;
use sp_runtime::traits::NumberFor;
use std::{error::Error, sync::Arc};

pub struct BabeConsensus {
    babe_link: Option<BabeLink<Block>>,
    babe_worker_handle: Option<BabeWorkerHandle<Block>>,
}

impl ConsensusMechanism for BabeConsensus {
    type InherentDataProviders = (
        sp_consensus_babe::inherents::InherentDataProvider,
        sp_timestamp::InherentDataProvider,
    );

    fn start_authoring<C, SC, I, PF, SO, L, CIDP, BS, Error>(
        self,
        task_manager: &mut TaskManager,
        StartAuthoringParams {
            slot_duration: _,
            client,
            select_chain,
            block_import,
            proposer_factory,
            sync_oracle,
            justification_sync_link,
            create_inherent_data_providers,
            force_authoring,
            backoff_authoring_blocks,
            keystore,
            telemetry,
            block_proposal_slot_portion,
            max_block_proposal_slot_portion,
        }: StartAuthoringParams<C, SC, I, PF, SO, L, CIDP, BS>,
    ) -> Result<(), sp_consensus::Error>
    where
        C: ProvideRuntimeApi<Block>
            + BlockOf
            + AuxStore
            + HeaderBackend<Block>
            + HeaderMetadata<Block, Error = sp_blockchain::Error>
            + Send
            + Sync
            + 'static,
        C::Api: AuraApi<Block, AuthorityId> + BabeApi<Block>,
        SC: SelectChain<Block> + 'static,
        I: BlockImport<Block, Error = sp_consensus::Error> + Send + Sync + 'static,
        PF: Environment<Block, Error = Error> + Send + Sync + 'static,
        PF::Proposer: Proposer<Block, Error = Error>,
        SO: SyncOracle + Send + Sync + Clone + 'static,
        L: sc_consensus::JustificationSyncLink<Block> + 'static,
        CIDP: CreateInherentDataProviders<Block, ()> + Send + Sync + 'static,
        CIDP::InherentDataProviders: InherentDataProviderExt + Send,
        BS: BackoffAuthoringBlocksStrategy<NumberFor<Block>> + Send + Sync + 'static,
        Error: std::error::Error + Send + From<sp_consensus::Error> + From<I::Error> + 'static,
    {
        let babe = sc_consensus_babe::start_babe::<Block, C, SC, PF, I, SO, CIDP, BS, L, Error>(
            sc_consensus_babe::BabeParams {
                keystore,
                client,
                select_chain,
                env: proposer_factory,
                block_import,
                sync_oracle,
                justification_sync_link,
                create_inherent_data_providers,
                force_authoring,
                backoff_authoring_blocks,
                babe_link: self
                    .babe_link
                    .expect("Must build the import queue before starting authoring."),
                block_proposal_slot_portion,
                max_block_proposal_slot_portion,
                telemetry,
            },
        )?;

        // the BABE authoring task is considered essential, i.e. if it
        // fails we take down the service with it.
        task_manager.spawn_essential_handle().spawn_blocking(
            "babe-proposer",
            Some("block-authoring"),
            babe,
        );

        Ok(())
    }

    fn frontier_consensus_data_provider(
        client: Arc<FullClient>,
    ) -> Box<dyn fc_rpc::pending::ConsensusDataProvider<Block>> {
        // TODO: When frontier is merged, update this to fc_babe::BabeConsensusDataProvider
        // Box::new(fc_babe::BabeConsensusDataProvider::new(client))
        Box::new(fc_aura::AuraConsensusDataProvider::new(client))
    }

    fn create_inherent_data_providers(
        slot_duration: SlotDuration,
    ) -> Result<Self::InherentDataProviders, Box<dyn Error + Send + Sync>> {
        let timestamp = sp_timestamp::InherentDataProvider::from_system_time();
        let slot =
            sp_consensus_babe::inherents::InherentDataProvider::from_timestamp_and_slot_duration(
                *timestamp,
                slot_duration,
            );
        Ok((slot, timestamp))
    }

    fn new() -> Self {
        Self {
            babe_link: None,
            babe_worker_handle: None,
        }
    }

    fn build_biq(&mut self) -> Result<BIQ, sc_service::Error>
    where
        NumberFor<Block>: BlockNumberOps,
    {
        let build_import_queue = Box::new(
            move |client: Arc<FullClient>,
                  backend: Arc<FullBackend>,
                  config: &Configuration,
                  _eth_config: &EthConfiguration,
                  task_manager: &TaskManager,
                  telemetry: Option<TelemetryHandle>,
                  grandpa_block_import: GrandpaBlockImport,
                  transaction_pool: Arc<TransactionPoolHandle<Block, FullClient>>| {
                let (babe_import, babe_link) = sc_consensus_babe::block_import(
                    sc_consensus_babe::configuration(&*client)?,
                    grandpa_block_import.clone(),
                    client.clone(),
                )?;

                let conditional_block_import = ConditionalEVMBlockImport::new(
                    babe_import.clone(),
                    FrontierBlockImport::new(babe_import.clone(), client.clone()),
                    client.clone(),
                );

                let slot_duration = babe_link.config().slot_duration();
                let create_inherent_data_providers = move |_, ()| async move {
                    let timestamp = sp_timestamp::InherentDataProvider::from_system_time();
                    let slot =
						sp_consensus_babe::inherents::InherentDataProvider::from_timestamp_and_slot_duration(
							*timestamp,
							slot_duration,
						);
                    Ok((slot, timestamp))
                };

                let (import_queue, babe_worker_handle) =
                    sc_consensus_babe::import_queue(sc_consensus_babe::ImportQueueParams {
                        link: babe_link.clone(),
                        block_import: conditional_block_import.clone(),
                        justification_import: Some(Box::new(grandpa_block_import)),
                        client,
                        select_chain: sc_consensus::LongestChain::new(backend.clone()),
                        create_inherent_data_providers,
                        spawner: &task_manager.spawn_essential_handle(),
                        registry: config.prometheus_registry(),
                        telemetry,
                        offchain_tx_pool_factory: OffchainTransactionPoolFactory::new(
                            transaction_pool,
                        ),
                    })?;

                self.babe_link = Some(babe_link);
                self.babe_worker_handle = Some(babe_worker_handle);
                Ok((import_queue, Box::new(babe_import) as BoxBlockImport<Block>))
            },
        );

        Ok(build_import_queue)
    }

    fn slot_duration(client: &FullClient) -> Result<SlotDuration, sc_service::Error> {
        sc_consensus_aura::slot_duration(&*client).map_err(Into::into)
    }

    fn spawn_essential_handles(
        _task_manager: &mut TaskManager,
        _client: Arc<FullClient>,
        _triggered: Option<Arc<std::sync::atomic::AtomicBool>>,
    ) -> Result<(), sc_service::Error> {
        // No additional Babe handles required.
        Ok(())
    }

    fn rpc_methods() -> Vec<jsonrpsee::Methods> {
        // TODO: Add Babe RPC.
        Default::default()
    }
}
