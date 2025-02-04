extern crate alloc;

use alloc::format;
use core::marker::PhantomData;

use frame_support::dispatch::{GetDispatchInfo, Pays};

use pallet_evm::{
    ExitError, ExitSucceed, GasWeightMapping, IsPrecompileResult, Precompile, PrecompileFailure,
    PrecompileHandle, PrecompileOutput, PrecompileResult, PrecompileSet,
};
use pallet_evm_precompile_modexp::Modexp;
use pallet_evm_precompile_sha3fips::Sha3FIPS256;
use pallet_evm_precompile_simple::{ECRecover, ECRecoverPublicKey, Identity, Ripemd160, Sha256};
use sp_core::{hashing::keccak_256, H160};
use sp_runtime::{traits::Dispatchable, AccountId32};

use crate::{Runtime, RuntimeCall};

use frame_system::RawOrigin;

use sp_std::vec;

// Include custom precompiles
mod balance_transfer;
mod ed25519;
mod metagraph;
mod neuron;
mod staking;
mod subnet;

use balance_transfer::*;
use ed25519::*;
use metagraph::*;
use neuron::*;
use staking::*;
use subnet::*;
pub struct FrontierPrecompiles<R>(PhantomData<R>);
impl<R> Default for FrontierPrecompiles<R>
where
    R: pallet_evm::Config,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<R> FrontierPrecompiles<R>
where
    R: pallet_evm::Config,
{
    pub fn new() -> Self {
        Self(Default::default())
    }
    pub fn used_addresses() -> [H160; 13] {
        [
            hash(1),
            hash(2),
            hash(3),
            hash(4),
            hash(5),
            hash(1024),
            hash(1025),
            hash(EDVERIFY_PRECOMPILE_INDEX),
            hash(BALANCE_TRANSFER_INDEX),
            hash(STAKING_PRECOMPILE_INDEX),
            hash(SUBNET_PRECOMPILE_INDEX),
            hash(METAGRAPH_PRECOMPILE_INDEX),
            hash(NEURON_PRECOMPILE_INDEX),
        ]
    }
}
impl<R> PrecompileSet for FrontierPrecompiles<R>
where
    R: pallet_evm::Config,
{
    fn execute(&self, handle: &mut impl PrecompileHandle) -> Option<PrecompileResult> {
        match handle.code_address() {
            // Ethereum precompiles :
            a if a == hash(1) => Some(ECRecover::execute(handle)),
            a if a == hash(2) => Some(Sha256::execute(handle)),
            a if a == hash(3) => Some(Ripemd160::execute(handle)),
            a if a == hash(4) => Some(Identity::execute(handle)),
            a if a == hash(5) => Some(Modexp::execute(handle)),
            // Non-Frontier specific nor Ethereum precompiles :
            a if a == hash(1024) => Some(Sha3FIPS256::execute(handle)),
            a if a == hash(1025) => Some(ECRecoverPublicKey::execute(handle)),
            a if a == hash(EDVERIFY_PRECOMPILE_INDEX) => Some(Ed25519Verify::execute(handle)),
            // Subtensor specific precompiles :
            a if a == hash(BALANCE_TRANSFER_INDEX) => {
                Some(BalanceTransferPrecompile::execute(handle))
            }
            a if a == hash(STAKING_PRECOMPILE_INDEX) => Some(StakingPrecompile::execute(handle)),
            a if a == hash(SUBNET_PRECOMPILE_INDEX) => Some(SubnetPrecompile::execute(handle)),
            a if a == hash(METAGRAPH_PRECOMPILE_INDEX) => {
                Some(MetagraphPrecompile::execute(handle))
            }
            a if a == hash(NEURON_PRECOMPILE_INDEX) => Some(NeuronPrecompile::execute(handle)),

            _ => None,
        }
    }

    fn is_precompile(&self, address: H160, _gas: u64) -> IsPrecompileResult {
        IsPrecompileResult::Answer {
            is_precompile: Self::used_addresses().contains(&address),
            extra_cost: 0,
        }
    }
}

fn hash(a: u64) -> H160 {
    H160::from_low_u64_be(a)
}

/// Returns Ethereum method ID from an str method signature
///
pub fn get_method_id(method_signature: &str) -> [u8; 4] {
    // Calculate the full Keccak-256 hash of the method signature
    let hash = keccak_256(method_signature.as_bytes());

    // Extract the first 4 bytes to get the method ID
    [hash[0], hash[1], hash[2], hash[3]]
}

/// Takes a slice from bytes with PrecompileFailure as Error
///
pub fn get_slice(data: &[u8], from: usize, to: usize) -> Result<&[u8], PrecompileFailure> {
    let maybe_slice = data.get(from..to);
    if let Some(slice) = maybe_slice {
        Ok(slice)
    } else {
        log::error!(
            "fail to get slice from data, {:?}, from {}, to {}",
            &data,
            from,
            to
        );
        Err(PrecompileFailure::Error {
            exit_status: ExitError::InvalidRange,
        })
    }
}

pub fn get_pubkey(data: &[u8]) -> Result<(AccountId32, vec::Vec<u8>), PrecompileFailure> {
    let mut pubkey = [0u8; 32];
    pubkey.copy_from_slice(get_slice(data, 0, 32)?);

    Ok((
        pubkey.into(),
        data.get(4..)
            .map_or_else(vec::Vec::new, |slice| slice.to_vec()),
    ))
}

fn parse_netuid(data: &[u8], offset: usize) -> Result<u16, PrecompileFailure> {
    if data.len() < offset + 2 {
        return Err(PrecompileFailure::Error {
            exit_status: ExitError::InvalidRange,
        });
    }

    let mut netuid_bytes = [0u8; 2];
    netuid_bytes.copy_from_slice(get_slice(data, offset, offset + 2)?);
    let netuid: u16 = netuid_bytes[1] as u16 | ((netuid_bytes[0] as u16) << 8u16);

    Ok(netuid)
}

fn contract_to_origin(contract: &[u8; 32]) -> Result<RawOrigin<AccountId32>, PrecompileFailure> {
    let (account_id, _) = get_pubkey(contract)?;
    Ok(RawOrigin::Signed(account_id))
}

/// Dispatches a runtime call, but also checks and records the gas costs.
fn try_dispatch_runtime_call(
    handle: &mut impl PrecompileHandle,
    call: impl Into<RuntimeCall>,
    origin: RawOrigin<AccountId32>,
) -> PrecompileResult {
    let call = Into::<RuntimeCall>::into(call);
    let info = call.get_dispatch_info();

    let target_gas = handle.gas_limit();
    if let Some(gas) = target_gas {
        let valid_weight =
            <Runtime as pallet_evm::Config>::GasWeightMapping::gas_to_weight(gas, false).ref_time();
        if info.weight.ref_time() > valid_weight {
            return Err(PrecompileFailure::Error {
                exit_status: ExitError::OutOfGas,
            });
        }
    }

    handle.record_external_cost(
        Some(info.weight.ref_time()),
        Some(info.weight.proof_size()),
        None,
    )?;

    match call.dispatch(origin.into()) {
        Ok(post_info) => {
            if post_info.pays_fee(&info) == Pays::Yes {
                let actual_weight = post_info.actual_weight.unwrap_or(info.weight);
                let cost =
                    <Runtime as pallet_evm::Config>::GasWeightMapping::weight_to_gas(actual_weight);
                handle.record_cost(cost)?;

                handle.refund_external_cost(
                    Some(
                        info.weight
                            .ref_time()
                            .saturating_sub(actual_weight.ref_time()),
                    ),
                    Some(
                        info.weight
                            .proof_size()
                            .saturating_sub(actual_weight.proof_size()),
                    ),
                );
            }

            log::info!("Dispatch succeeded. Post info: {:?}", post_info);

            Ok(PrecompileOutput {
                exit_status: ExitSucceed::Returned,
                output: Default::default(),
            })
        }
        Err(e) => {
            log::error!("Dispatch failed. Error: {:?}", e);
            log::warn!("Returning error PrecompileFailure::Error");
            Err(PrecompileFailure::Error {
                exit_status: ExitError::Other(
                    format!("dispatch execution failed: {}", <&'static str>::from(e)).into(),
                ),
            })
        }
    }
}
