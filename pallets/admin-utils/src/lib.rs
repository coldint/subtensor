#![cfg_attr(not(feature = "std"), no_std)]

// extern crate alloc;

use frame_system::pallet_prelude::BlockNumberFor;
pub use pallet::*;
// - we could replace it with Vec<(AuthorityId, u64)>, but we would need
//   `sp_consensus_grandpa` for `AuthorityId` anyway
// - we could use a type parameter for `AuthorityId`, but there is
//   no sense for this as GRANDPA's `AuthorityId` is not a parameter -- it's always the same
use sp_consensus_grandpa::AuthorityList;
use sp_runtime::{DispatchResult, RuntimeAppPublic, traits::Member};

mod benchmarking;

#[cfg(test)]
mod tests;

#[deny(missing_docs)]
#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use frame_support::pallet_prelude::*;
    use frame_support::traits::tokens::Balance;
    use frame_support::{
        dispatch::{DispatchResult, RawOrigin},
        pallet_prelude::StorageMap,
    };
    use frame_system::pallet_prelude::*;
    use pallet_evm_chain_id::{self, ChainId};
    use pallet_subtensor::utils::rate_limiting::TransactionType;
    use scale_info::TypeInfo;
    use scale_info::prelude::fmt;
    use sp_runtime::BoundedVec;
    use substrate_fixed::types::I96F32;
    use subtensor_runtime_common::NetUid;

    /// The main data structure of the module.
    #[pallet::pallet]
    #[pallet::without_storage_info]
    pub struct Pallet<T>(_);

    /// Configure the pallet by specifying the parameters and types on which it depends.
    #[pallet::config]
    pub trait Config:
        frame_system::Config
        + pallet_subtensor::pallet::Config
        + pallet_evm_chain_id::pallet::Config
    {
        /// Because this pallet emits events, it depends on the runtime's definition of an event.
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        /// Implementation of the AuraInterface
        type Aura: crate::AuraInterface<<Self as Config>::AuthorityId, Self::MaxAuthorities>;

        /// Implementation of [`GrandpaInterface`]
        type Grandpa: crate::GrandpaInterface<Self>;

        /// The identifier type for an authority.
        type AuthorityId: Member
            + Parameter
            + RuntimeAppPublic
            + MaybeSerializeDeserialize
            + MaxEncodedLen;

        /// The maximum number of authorities that the pallet can hold.
        type MaxAuthorities: Get<u32>;

        /// Unit of assets
        type Balance: Balance;
    }

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// Event emitted when a precompile operation is updated.
        PrecompileUpdated {
            /// The type of precompile operation being updated.
            precompile_id: PrecompileEnum,
            /// Indicates if the precompile operation is enabled or not.
            enabled: bool,
        },
        /// Event emitted when the Yuma3 enable is toggled.
        Yuma3EnableToggled {
            /// The network identifier.
            netuid: NetUid,
            /// Indicates if the Yuma3 enable was enabled or disabled.
            enabled: bool,
        },
        /// Event emitted when Bonds Reset is toggled.
        BondsResetToggled {
            /// The network identifier.
            netuid: NetUid,
            /// Indicates if the Bonds Reset was enabled or disabled.
            enabled: bool,
        },
    }

    // Errors inform users that something went wrong.
    #[pallet::error]
    pub enum Error<T> {
        /// The subnet does not exist, check the netuid parameter
        SubnetDoesNotExist,
        /// The maximum number of subnet validators must be less than the maximum number of allowed UIDs in the subnet.
        MaxValidatorsLargerThanMaxUIds,
        /// The maximum number of subnet validators must be more than the current number of UIDs already in the subnet.
        MaxAllowedUIdsLessThanCurrentUIds,
        /// The maximum value for bonds moving average is reached
        BondsMovingAverageMaxReached,
    }
    /// Enum for specifying the type of precompile operation.
    #[derive(Encode, Decode, TypeInfo, Clone, PartialEq, Eq, Debug, Copy)]
    pub enum PrecompileEnum {
        /// Enum for balance transfer precompile
        BalanceTransfer,
        /// Enum for staking precompile
        Staking,
        /// Enum for subnet precompile
        Subnet,
        /// Enum for metagraph precompile
        Metagraph,
        /// Enum for neuron precompile
        Neuron,
        /// Enum for UID lookup precompile
        UidLookup,
        /// Enum for alpha precompile
        Alpha,
    }

    #[pallet::type_value]
    /// Default value for precompile enable
    pub fn DefaultPrecompileEnabled<T: Config>() -> bool {
        true
    }

    #[pallet::storage]
    /// Map PrecompileEnum --> enabled
    pub type PrecompileEnable<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        PrecompileEnum,
        bool,
        ValueQuery,
        DefaultPrecompileEnabled<T>,
    >;

    /// Dispatchable functions allows users to interact with the pallet and invoke state changes.
    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// The extrinsic sets the new authorities for Aura consensus.
        /// It is only callable by the root account.
        /// The extrinsic will call the Aura pallet to change the authorities.
        #[pallet::call_index(0)]
        #[pallet::weight(Weight::from_parts(6_265_000, 0)
        .saturating_add(T::DbWeight::get().reads(1_u64))
        .saturating_add(T::DbWeight::get().writes(2_u64)))]
        pub fn swap_authorities(
            origin: OriginFor<T>,
            new_authorities: BoundedVec<<T as Config>::AuthorityId, T::MaxAuthorities>,
        ) -> DispatchResult {
            ensure_root(origin)?;

            T::Aura::change_authorities(new_authorities.clone());

            log::debug!("Aura authorities changed: {:?}", new_authorities);

            // Return a successful DispatchResultWithPostInfo
            Ok(())
        }

        /// The extrinsic sets the default take for the network.
        /// It is only callable by the root account.
        /// The extrinsic will call the Subtensor pallet to set the default take.
        #[pallet::call_index(1)]
        #[pallet::weight(Weight::from_parts(6_942_000, 0)
        .saturating_add(T::DbWeight::get().reads(0_u64))
        .saturating_add(T::DbWeight::get().writes(1_u64)))]
        pub fn sudo_set_default_take(origin: OriginFor<T>, default_take: u16) -> DispatchResult {
            ensure_root(origin)?;
            pallet_subtensor::Pallet::<T>::set_max_delegate_take(default_take);
            log::debug!("DefaultTakeSet( default_take: {:?} ) ", default_take);
            Ok(())
        }

        /// The extrinsic sets the transaction rate limit for the network.
        /// It is only callable by the root account.
        /// The extrinsic will call the Subtensor pallet to set the transaction rate limit.
        #[pallet::call_index(2)]
        #[pallet::weight((0, DispatchClass::Operational, Pays::No))]
        pub fn sudo_set_tx_rate_limit(origin: OriginFor<T>, tx_rate_limit: u64) -> DispatchResult {
            ensure_root(origin)?;
            pallet_subtensor::Pallet::<T>::set_tx_rate_limit(tx_rate_limit);
            log::debug!("TxRateLimitSet( tx_rate_limit: {:?} ) ", tx_rate_limit);
            Ok(())
        }

        /// The extrinsic sets the serving rate limit for a subnet.
        /// It is only callable by the root account or subnet owner.
        /// The extrinsic will call the Subtensor pallet to set the serving rate limit.
        #[pallet::call_index(3)]
        #[pallet::weight(Weight::from_parts(7_815_000, 0)
        .saturating_add(T::DbWeight::get().reads(0_u64))
        .saturating_add(T::DbWeight::get().writes(1_u64)))]
        pub fn sudo_set_serving_rate_limit(
            origin: OriginFor<T>,
            netuid: NetUid,
            serving_rate_limit: u64,
        ) -> DispatchResult {
            pallet_subtensor::Pallet::<T>::ensure_subnet_owner_or_root(origin, netuid)?;

            pallet_subtensor::Pallet::<T>::set_serving_rate_limit(netuid, serving_rate_limit);
            log::debug!(
                "ServingRateLimitSet( serving_rate_limit: {:?} ) ",
                serving_rate_limit
            );
            Ok(())
        }

        /// The extrinsic sets the minimum difficulty for a subnet.
        /// It is only callable by the root account or subnet owner.
        /// The extrinsic will call the Subtensor pallet to set the minimum difficulty.
        #[pallet::call_index(4)]
        #[pallet::weight(Weight::from_parts(19_780_000, 0)
        .saturating_add(T::DbWeight::get().reads(1_u64))
        .saturating_add(T::DbWeight::get().writes(1_u64)))]
        pub fn sudo_set_min_difficulty(
            origin: OriginFor<T>,
            netuid: NetUid,
            min_difficulty: u64,
        ) -> DispatchResult {
            ensure_root(origin)?;

            ensure!(
                pallet_subtensor::Pallet::<T>::if_subnet_exist(netuid),
                Error::<T>::SubnetDoesNotExist
            );
            pallet_subtensor::Pallet::<T>::set_min_difficulty(netuid, min_difficulty);
            log::debug!(
                "MinDifficultySet( netuid: {:?} min_difficulty: {:?} ) ",
                netuid,
                min_difficulty
            );
            Ok(())
        }

        /// The extrinsic sets the maximum difficulty for a subnet.
        /// It is only callable by the root account or subnet owner.
        /// The extrinsic will call the Subtensor pallet to set the maximum difficulty.
        #[pallet::call_index(5)]
        #[pallet::weight(Weight::from_parts(20_050_000, 0)
        .saturating_add(T::DbWeight::get().reads(1_u64))
        .saturating_add(T::DbWeight::get().writes(1_u64)))]
        pub fn sudo_set_max_difficulty(
            origin: OriginFor<T>,
            netuid: NetUid,
            max_difficulty: u64,
        ) -> DispatchResult {
            pallet_subtensor::Pallet::<T>::ensure_subnet_owner_or_root(origin, netuid)?;

            ensure!(
                pallet_subtensor::Pallet::<T>::if_subnet_exist(netuid),
                Error::<T>::SubnetDoesNotExist
            );
            pallet_subtensor::Pallet::<T>::set_max_difficulty(netuid, max_difficulty);
            log::debug!(
                "MaxDifficultySet( netuid: {:?} max_difficulty: {:?} ) ",
                netuid,
                max_difficulty
            );
            Ok(())
        }

        /// The extrinsic sets the weights version key for a subnet.
        /// It is only callable by the root account or subnet owner.
        /// The extrinsic will call the Subtensor pallet to set the weights version key.
        #[pallet::call_index(6)]
        #[pallet::weight(Weight::from_parts(19_990_000, 0)
        .saturating_add(T::DbWeight::get().reads(1_u64))
        .saturating_add(T::DbWeight::get().writes(1_u64)))]
        pub fn sudo_set_weights_version_key(
            origin: OriginFor<T>,
            netuid: NetUid,
            weights_version_key: u64,
        ) -> DispatchResult {
            pallet_subtensor::Pallet::<T>::ensure_subnet_owner_or_root(origin.clone(), netuid)?;

            ensure!(
                pallet_subtensor::Pallet::<T>::if_subnet_exist(netuid),
                Error::<T>::SubnetDoesNotExist
            );

            if let Ok(RawOrigin::Signed(who)) = origin.into() {
                // SN Owner
                // Ensure the origin passes the rate limit.
                ensure!(
                    pallet_subtensor::Pallet::<T>::passes_rate_limit_on_subnet(
                        &TransactionType::SetWeightsVersionKey,
                        &who,
                        netuid,
                    ),
                    pallet_subtensor::Error::<T>::TxRateLimitExceeded
                );

                // Set last transaction block
                let current_block = pallet_subtensor::Pallet::<T>::get_current_block_as_u64();
                pallet_subtensor::Pallet::<T>::set_last_transaction_block_on_subnet(
                    &who,
                    netuid,
                    &TransactionType::SetWeightsVersionKey,
                    current_block,
                );
            }

            pallet_subtensor::Pallet::<T>::set_weights_version_key(netuid, weights_version_key);
            log::debug!(
                "WeightsVersionKeySet( netuid: {:?} weights_version_key: {:?} ) ",
                netuid,
                weights_version_key
            );
            Ok(())
        }

        /// The extrinsic sets the weights set rate limit for a subnet.
        /// It is only callable by the root account.
        /// The extrinsic will call the Subtensor pallet to set the weights set rate limit.
        #[pallet::call_index(7)]
        #[pallet::weight(Weight::from_parts(20_050_000, 0)
        .saturating_add(T::DbWeight::get().reads(1_u64))
        .saturating_add(T::DbWeight::get().writes(1_u64)))]
        pub fn sudo_set_weights_set_rate_limit(
            origin: OriginFor<T>,
            netuid: NetUid,
            weights_set_rate_limit: u64,
        ) -> DispatchResult {
            ensure_root(origin)?;

            ensure!(
                pallet_subtensor::Pallet::<T>::if_subnet_exist(netuid),
                Error::<T>::SubnetDoesNotExist
            );
            pallet_subtensor::Pallet::<T>::set_weights_set_rate_limit(
                netuid,
                weights_set_rate_limit,
            );
            log::debug!(
                "WeightsSetRateLimitSet( netuid: {:?} weights_set_rate_limit: {:?} ) ",
                netuid,
                weights_set_rate_limit
            );
            Ok(())
        }

        /// The extrinsic sets the adjustment interval for a subnet.
        /// It is only callable by the root account, not changeable by the subnet owner.
        /// The extrinsic will call the Subtensor pallet to set the adjustment interval.
        #[pallet::call_index(8)]
        #[pallet::weight(Weight::from_parts(20_010_000, 0)
        .saturating_add(T::DbWeight::get().reads(1_u64))
        .saturating_add(T::DbWeight::get().writes(1_u64)))]
        pub fn sudo_set_adjustment_interval(
            origin: OriginFor<T>,
            netuid: NetUid,
            adjustment_interval: u16,
        ) -> DispatchResult {
            ensure_root(origin)?;

            ensure!(
                pallet_subtensor::Pallet::<T>::if_subnet_exist(netuid),
                Error::<T>::SubnetDoesNotExist
            );
            pallet_subtensor::Pallet::<T>::set_adjustment_interval(netuid, adjustment_interval);
            log::debug!(
                "AdjustmentIntervalSet( netuid: {:?} adjustment_interval: {:?} ) ",
                netuid,
                adjustment_interval
            );
            Ok(())
        }

        /// The extrinsic sets the adjustment alpha for a subnet.
        /// It is only callable by the root account or subnet owner.
        /// The extrinsic will call the Subtensor pallet to set the adjustment alpha.
        #[pallet::call_index(9)]
        #[pallet::weight((
            Weight::from_parts(14_000_000, 0)
                .saturating_add(T::DbWeight::get().writes(1))
                .saturating_add(T::DbWeight::get().reads(1)),
            DispatchClass::Operational,
            Pays::No
        ))]
        pub fn sudo_set_adjustment_alpha(
            origin: OriginFor<T>,
            netuid: NetUid,
            adjustment_alpha: u64,
        ) -> DispatchResult {
            pallet_subtensor::Pallet::<T>::ensure_subnet_owner_or_root(origin, netuid)?;

            ensure!(
                pallet_subtensor::Pallet::<T>::if_subnet_exist(netuid),
                Error::<T>::SubnetDoesNotExist
            );
            pallet_subtensor::Pallet::<T>::set_adjustment_alpha(netuid, adjustment_alpha);
            log::debug!(
                "AdjustmentAlphaSet( adjustment_alpha: {:?} ) ",
                adjustment_alpha
            );
            Ok(())
        }

        /// The extrinsic sets the adjustment beta for a subnet.
        /// It is only callable by the root account or subnet owner.
        /// The extrinsic will call the Subtensor pallet to set the adjustment beta.
        #[pallet::call_index(12)]
        #[pallet::weight(Weight::from_parts(19_240_000, 0)
        .saturating_add(T::DbWeight::get().reads(1_u64))
        .saturating_add(T::DbWeight::get().writes(1_u64)))]
        pub fn sudo_set_max_weight_limit(
            origin: OriginFor<T>,
            netuid: NetUid,
            max_weight_limit: u16,
        ) -> DispatchResult {
            pallet_subtensor::Pallet::<T>::ensure_subnet_owner_or_root(origin, netuid)?;

            ensure!(
                pallet_subtensor::Pallet::<T>::if_subnet_exist(netuid),
                Error::<T>::SubnetDoesNotExist
            );
            pallet_subtensor::Pallet::<T>::set_max_weight_limit(netuid, max_weight_limit);
            log::debug!(
                "MaxWeightLimitSet( netuid: {:?} max_weight_limit: {:?} ) ",
                netuid,
                max_weight_limit
            );
            Ok(())
        }

        /// The extrinsic sets the immunity period for a subnet.
        /// It is only callable by the root account or subnet owner.
        /// The extrinsic will call the Subtensor pallet to set the immunity period.
        #[pallet::call_index(13)]
        #[pallet::weight(Weight::from_parts(19_380_000, 0)
        .saturating_add(T::DbWeight::get().reads(1_u64))
        .saturating_add(T::DbWeight::get().writes(1_u64)))]
        pub fn sudo_set_immunity_period(
            origin: OriginFor<T>,
            netuid: NetUid,
            immunity_period: u16,
        ) -> DispatchResult {
            pallet_subtensor::Pallet::<T>::ensure_subnet_owner_or_root(origin, netuid)?;
            ensure!(
                pallet_subtensor::Pallet::<T>::if_subnet_exist(netuid),
                Error::<T>::SubnetDoesNotExist
            );

            pallet_subtensor::Pallet::<T>::set_immunity_period(netuid, immunity_period);
            log::debug!(
                "ImmunityPeriodSet( netuid: {:?} immunity_period: {:?} ) ",
                netuid,
                immunity_period
            );
            Ok(())
        }

        /// The extrinsic sets the minimum allowed weights for a subnet.
        /// It is only callable by the root account or subnet owner.
        /// The extrinsic will call the Subtensor pallet to set the minimum allowed weights.
        #[pallet::call_index(14)]
        #[pallet::weight(Weight::from_parts(19_770_000, 0)
        .saturating_add(T::DbWeight::get().reads(1_u64))
        .saturating_add(T::DbWeight::get().writes(1_u64)))]
        pub fn sudo_set_min_allowed_weights(
            origin: OriginFor<T>,
            netuid: NetUid,
            min_allowed_weights: u16,
        ) -> DispatchResult {
            pallet_subtensor::Pallet::<T>::ensure_subnet_owner_or_root(origin, netuid)?;

            ensure!(
                pallet_subtensor::Pallet::<T>::if_subnet_exist(netuid),
                Error::<T>::SubnetDoesNotExist
            );
            pallet_subtensor::Pallet::<T>::set_min_allowed_weights(netuid, min_allowed_weights);
            log::debug!(
                "MinAllowedWeightSet( netuid: {:?} min_allowed_weights: {:?} ) ",
                netuid,
                min_allowed_weights
            );
            Ok(())
        }

        /// The extrinsic sets the maximum allowed UIDs for a subnet.
        /// It is only callable by the root account.
        /// The extrinsic will call the Subtensor pallet to set the maximum allowed UIDs for a subnet.
        #[pallet::call_index(15)]
        #[pallet::weight(Weight::from_parts(23_820_000, 0)
        .saturating_add(T::DbWeight::get().reads(2_u64))
        .saturating_add(T::DbWeight::get().writes(1_u64)))]
        pub fn sudo_set_max_allowed_uids(
            origin: OriginFor<T>,
            netuid: NetUid,
            max_allowed_uids: u16,
        ) -> DispatchResult {
            ensure_root(origin)?;
            ensure!(
                pallet_subtensor::Pallet::<T>::if_subnet_exist(netuid),
                Error::<T>::SubnetDoesNotExist
            );
            ensure!(
                pallet_subtensor::Pallet::<T>::get_subnetwork_n(netuid) < max_allowed_uids,
                Error::<T>::MaxAllowedUIdsLessThanCurrentUIds
            );
            pallet_subtensor::Pallet::<T>::set_max_allowed_uids(netuid, max_allowed_uids);
            log::debug!(
                "MaxAllowedUidsSet( netuid: {:?} max_allowed_uids: {:?} ) ",
                netuid,
                max_allowed_uids
            );
            Ok(())
        }

        /// The extrinsic sets the kappa for a subnet.
        /// It is only callable by the root account or subnet owner.
        /// The extrinsic will call the Subtensor pallet to set the kappa.
        #[pallet::call_index(16)]
        #[pallet::weight(Weight::from_parts(19_590_000, 0)
        .saturating_add(T::DbWeight::get().reads(1_u64))
        .saturating_add(T::DbWeight::get().writes(1_u64)))]
        pub fn sudo_set_kappa(origin: OriginFor<T>, netuid: NetUid, kappa: u16) -> DispatchResult {
            pallet_subtensor::Pallet::<T>::ensure_subnet_owner_or_root(origin, netuid)?;

            ensure!(
                pallet_subtensor::Pallet::<T>::if_subnet_exist(netuid),
                Error::<T>::SubnetDoesNotExist
            );
            pallet_subtensor::Pallet::<T>::set_kappa(netuid, kappa);
            log::debug!("KappaSet( netuid: {:?} kappa: {:?} ) ", netuid, kappa);
            Ok(())
        }

        /// The extrinsic sets the rho for a subnet.
        /// It is only callable by the root account or subnet owner.
        /// The extrinsic will call the Subtensor pallet to set the rho.
        #[pallet::call_index(17)]
        #[pallet::weight(Weight::from_parts(16_420_000, 0)
        .saturating_add(T::DbWeight::get().reads(1_u64))
        .saturating_add(T::DbWeight::get().writes(1_u64)))]
        pub fn sudo_set_rho(origin: OriginFor<T>, netuid: NetUid, rho: u16) -> DispatchResult {
            pallet_subtensor::Pallet::<T>::ensure_subnet_owner_or_root(origin, netuid)?;

            ensure!(
                pallet_subtensor::Pallet::<T>::if_subnet_exist(netuid),
                Error::<T>::SubnetDoesNotExist
            );
            pallet_subtensor::Pallet::<T>::set_rho(netuid, rho);
            log::debug!("RhoSet( netuid: {:?} rho: {:?} ) ", netuid, rho);
            Ok(())
        }

        /// The extrinsic sets the activity cutoff for a subnet.
        /// It is only callable by the root account or subnet owner.
        /// The extrinsic will call the Subtensor pallet to set the activity cutoff.
        #[pallet::call_index(18)]
        #[pallet::weight(Weight::from_parts(22_600_000, 0)
        .saturating_add(T::DbWeight::get().reads(2_u64))
        .saturating_add(T::DbWeight::get().writes(1_u64)))]
        pub fn sudo_set_activity_cutoff(
            origin: OriginFor<T>,
            netuid: NetUid,
            activity_cutoff: u16,
        ) -> DispatchResult {
            pallet_subtensor::Pallet::<T>::ensure_subnet_owner_or_root(origin, netuid)?;

            ensure!(
                pallet_subtensor::Pallet::<T>::if_subnet_exist(netuid),
                Error::<T>::SubnetDoesNotExist
            );

            ensure!(
                activity_cutoff >= pallet_subtensor::MinActivityCutoff::<T>::get(),
                pallet_subtensor::Error::<T>::ActivityCutoffTooLow
            );

            pallet_subtensor::Pallet::<T>::set_activity_cutoff(netuid, activity_cutoff);
            log::debug!(
                "ActivityCutoffSet( netuid: {:?} activity_cutoff: {:?} ) ",
                netuid,
                activity_cutoff
            );
            Ok(())
        }

        /// The extrinsic sets the network registration allowed for a subnet.
        /// It is only callable by the root account or subnet owner.
        /// The extrinsic will call the Subtensor pallet to set the network registration allowed.
        #[pallet::call_index(19)]
        #[pallet::weight((
			Weight::from_parts(8_696_000, 0)
                .saturating_add(T::DbWeight::get().reads(0))
				.saturating_add(T::DbWeight::get().writes(1)),
			DispatchClass::Operational,
			Pays::No
		))]
        pub fn sudo_set_network_registration_allowed(
            origin: OriginFor<T>,
            netuid: NetUid,
            registration_allowed: bool,
        ) -> DispatchResult {
            pallet_subtensor::Pallet::<T>::ensure_subnet_owner_or_root(origin, netuid)?;

            pallet_subtensor::Pallet::<T>::set_network_registration_allowed(
                netuid,
                registration_allowed,
            );
            log::debug!(
                "NetworkRegistrationAllowed( registration_allowed: {:?} ) ",
                registration_allowed
            );
            Ok(())
        }

        /// The extrinsic sets the network PoW registration allowed for a subnet.
        /// It is only callable by the root account or subnet owner.
        /// The extrinsic will call the Subtensor pallet to set the network PoW registration allowed.
        #[pallet::call_index(20)]
        #[pallet::weight((
			Weight::from_parts(14_000_000, 0)
				.saturating_add(T::DbWeight::get().writes(1)),
			DispatchClass::Operational,
			Pays::No
		))]
        pub fn sudo_set_network_pow_registration_allowed(
            origin: OriginFor<T>,
            netuid: NetUid,
            registration_allowed: bool,
        ) -> DispatchResult {
            pallet_subtensor::Pallet::<T>::ensure_subnet_owner_or_root(origin, netuid)?;

            pallet_subtensor::Pallet::<T>::set_network_pow_registration_allowed(
                netuid,
                registration_allowed,
            );
            log::debug!(
                "NetworkPowRegistrationAllowed( registration_allowed: {:?} ) ",
                registration_allowed
            );
            Ok(())
        }

        /// The extrinsic sets the target registrations per interval for a subnet.
        /// It is only callable by the root account.
        /// The extrinsic will call the Subtensor pallet to set the target registrations per interval.
        #[pallet::call_index(21)]
        #[pallet::weight(Weight::from_parts(19_830_000, 0)
        .saturating_add(T::DbWeight::get().reads(1_u64))
        .saturating_add(T::DbWeight::get().writes(1_u64)))]
        pub fn sudo_set_target_registrations_per_interval(
            origin: OriginFor<T>,
            netuid: NetUid,
            target_registrations_per_interval: u16,
        ) -> DispatchResult {
            ensure_root(origin)?;

            ensure!(
                pallet_subtensor::Pallet::<T>::if_subnet_exist(netuid),
                Error::<T>::SubnetDoesNotExist
            );
            pallet_subtensor::Pallet::<T>::set_target_registrations_per_interval(
                netuid,
                target_registrations_per_interval,
            );
            log::debug!(
                "RegistrationPerIntervalSet( netuid: {:?} target_registrations_per_interval: {:?} ) ",
                netuid,
                target_registrations_per_interval
            );
            Ok(())
        }

        /// The extrinsic sets the minimum burn for a subnet.
        /// It is only callable by the root account.
        /// The extrinsic will call the Subtensor pallet to set the minimum burn.
        #[pallet::call_index(22)]
        #[pallet::weight(Weight::from_parts(19_840_000, 0)
        .saturating_add(T::DbWeight::get().reads(1_u64))
        .saturating_add(T::DbWeight::get().writes(1_u64)))]
        pub fn sudo_set_min_burn(
            origin: OriginFor<T>,
            netuid: NetUid,
            min_burn: u64,
        ) -> DispatchResult {
            ensure_root(origin)?;

            ensure!(
                pallet_subtensor::Pallet::<T>::if_subnet_exist(netuid),
                Error::<T>::SubnetDoesNotExist
            );
            pallet_subtensor::Pallet::<T>::set_min_burn(netuid, min_burn);
            log::debug!(
                "MinBurnSet( netuid: {:?} min_burn: {:?} ) ",
                netuid,
                min_burn
            );
            Ok(())
        }

        /// The extrinsic sets the maximum burn for a subnet.
        /// It is only callable by the root account or subnet owner.
        /// The extrinsic will call the Subtensor pallet to set the maximum burn.
        #[pallet::call_index(23)]
        #[pallet::weight(Weight::from_parts(19_740_000, 0)
        .saturating_add(T::DbWeight::get().reads(1_u64))
        .saturating_add(T::DbWeight::get().writes(1_u64)))]
        pub fn sudo_set_max_burn(
            origin: OriginFor<T>,
            netuid: NetUid,
            max_burn: u64,
        ) -> DispatchResult {
            ensure_root(origin)?;

            ensure!(
                pallet_subtensor::Pallet::<T>::if_subnet_exist(netuid),
                Error::<T>::SubnetDoesNotExist
            );
            pallet_subtensor::Pallet::<T>::set_max_burn(netuid, max_burn);
            log::debug!(
                "MaxBurnSet( netuid: {:?} max_burn: {:?} ) ",
                netuid,
                max_burn
            );
            Ok(())
        }

        /// The extrinsic sets the difficulty for a subnet.
        /// It is only callable by the root account or subnet owner.
        /// The extrinsic will call the Subtensor pallet to set the difficulty.
        #[pallet::call_index(24)]
        #[pallet::weight(Weight::from_parts(20_280_000, 0)
        .saturating_add(T::DbWeight::get().reads(1_u64))
        .saturating_add(T::DbWeight::get().writes(1_u64)))]
        pub fn sudo_set_difficulty(
            origin: OriginFor<T>,
            netuid: NetUid,
            difficulty: u64,
        ) -> DispatchResult {
            ensure_root(origin)?;
            ensure!(
                pallet_subtensor::Pallet::<T>::if_subnet_exist(netuid),
                Error::<T>::SubnetDoesNotExist
            );
            pallet_subtensor::Pallet::<T>::set_difficulty(netuid, difficulty);
            log::debug!(
                "DifficultySet( netuid: {:?} difficulty: {:?} ) ",
                netuid,
                difficulty
            );
            Ok(())
        }

        /// The extrinsic sets the maximum allowed validators for a subnet.
        /// It is only callable by the root account.
        /// The extrinsic will call the Subtensor pallet to set the maximum allowed validators.
        #[pallet::call_index(25)]
        #[pallet::weight(Weight::from_parts(25_210_000, 0)
        .saturating_add(T::DbWeight::get().reads(2_u64))
        .saturating_add(T::DbWeight::get().writes(1_u64)))]
        pub fn sudo_set_max_allowed_validators(
            origin: OriginFor<T>,
            netuid: NetUid,
            max_allowed_validators: u16,
        ) -> DispatchResult {
            ensure_root(origin)?;
            ensure!(
                pallet_subtensor::Pallet::<T>::if_subnet_exist(netuid),
                Error::<T>::SubnetDoesNotExist
            );
            ensure!(
                max_allowed_validators
                    <= pallet_subtensor::Pallet::<T>::get_max_allowed_uids(netuid),
                Error::<T>::MaxValidatorsLargerThanMaxUIds
            );

            pallet_subtensor::Pallet::<T>::set_max_allowed_validators(
                netuid,
                max_allowed_validators,
            );
            log::debug!(
                "MaxAllowedValidatorsSet( netuid: {:?} max_allowed_validators: {:?} ) ",
                netuid,
                max_allowed_validators
            );
            Ok(())
        }

        /// The extrinsic sets the bonds moving average for a subnet.
        /// It is only callable by the root account or subnet owner.
        /// The extrinsic will call the Subtensor pallet to set the bonds moving average.
        #[pallet::call_index(26)]
        #[pallet::weight(Weight::from_parts(20_270_000, 0)
        .saturating_add(T::DbWeight::get().reads(1_u64))
        .saturating_add(T::DbWeight::get().writes(1_u64)))]
        pub fn sudo_set_bonds_moving_average(
            origin: OriginFor<T>,
            netuid: NetUid,
            bonds_moving_average: u64,
        ) -> DispatchResult {
            pallet_subtensor::Pallet::<T>::ensure_subnet_owner_or_root(origin.clone(), netuid)?;

            if pallet_subtensor::Pallet::<T>::ensure_subnet_owner(origin, netuid).is_ok() {
                ensure!(
                    bonds_moving_average <= 975000,
                    Error::<T>::BondsMovingAverageMaxReached
                )
            }

            ensure!(
                pallet_subtensor::Pallet::<T>::if_subnet_exist(netuid),
                Error::<T>::SubnetDoesNotExist
            );
            pallet_subtensor::Pallet::<T>::set_bonds_moving_average(netuid, bonds_moving_average);
            log::debug!(
                "BondsMovingAverageSet( netuid: {:?} bonds_moving_average: {:?} ) ",
                netuid,
                bonds_moving_average
            );
            Ok(())
        }

        /// The extrinsic sets the bonds penalty for a subnet.
        /// It is only callable by the root account or subnet owner.
        /// The extrinsic will call the Subtensor pallet to set the bonds penalty.
        #[pallet::call_index(60)]
        #[pallet::weight(Weight::from_parts(20_030_000, 0)
        .saturating_add(T::DbWeight::get().reads(1_u64))
        .saturating_add(T::DbWeight::get().writes(1_u64)))]
        pub fn sudo_set_bonds_penalty(
            origin: OriginFor<T>,
            netuid: NetUid,
            bonds_penalty: u16,
        ) -> DispatchResult {
            pallet_subtensor::Pallet::<T>::ensure_subnet_owner_or_root(origin, netuid)?;

            ensure!(
                pallet_subtensor::Pallet::<T>::if_subnet_exist(netuid),
                Error::<T>::SubnetDoesNotExist
            );
            pallet_subtensor::Pallet::<T>::set_bonds_penalty(netuid, bonds_penalty);
            log::debug!(
                "BondsPenalty( netuid: {:?} bonds_penalty: {:?} ) ",
                netuid,
                bonds_penalty
            );
            Ok(())
        }

        /// The extrinsic sets the maximum registrations per block for a subnet.
        /// It is only callable by the root account.
        /// The extrinsic will call the Subtensor pallet to set the maximum registrations per block.
        #[pallet::call_index(27)]
        #[pallet::weight(Weight::from_parts(19_680_000, 0)
        .saturating_add(T::DbWeight::get().reads(1_u64))
        .saturating_add(T::DbWeight::get().writes(1_u64)))]
        pub fn sudo_set_max_registrations_per_block(
            origin: OriginFor<T>,
            netuid: NetUid,
            max_registrations_per_block: u16,
        ) -> DispatchResult {
            ensure_root(origin)?;

            ensure!(
                pallet_subtensor::Pallet::<T>::if_subnet_exist(netuid),
                Error::<T>::SubnetDoesNotExist
            );
            pallet_subtensor::Pallet::<T>::set_max_registrations_per_block(
                netuid,
                max_registrations_per_block,
            );
            log::debug!(
                "MaxRegistrationsPerBlock( netuid: {:?} max_registrations_per_block: {:?} ) ",
                netuid,
                max_registrations_per_block
            );
            Ok(())
        }

        /// The extrinsic sets the subnet owner cut for a subnet.
        /// It is only callable by the root account.
        /// The extrinsic will call the Subtensor pallet to set the subnet owner cut.
        #[pallet::call_index(28)]
        #[pallet::weight((
			Weight::from_parts(14_000_000, 0)
				.saturating_add(T::DbWeight::get().writes(1)),
			DispatchClass::Operational,
			Pays::No
		))]
        pub fn sudo_set_subnet_owner_cut(
            origin: OriginFor<T>,
            subnet_owner_cut: u16,
        ) -> DispatchResult {
            ensure_root(origin)?;
            pallet_subtensor::Pallet::<T>::set_subnet_owner_cut(subnet_owner_cut);
            log::debug!(
                "SubnetOwnerCut( subnet_owner_cut: {:?} ) ",
                subnet_owner_cut
            );
            Ok(())
        }

        /// The extrinsic sets the network rate limit for the network.
        /// It is only callable by the root account.
        /// The extrinsic will call the Subtensor pallet to set the network rate limit.
        #[pallet::call_index(29)]
        #[pallet::weight((
			Weight::from_parts(14_000_000, 0)
				.saturating_add(T::DbWeight::get().writes(1)),
			DispatchClass::Operational,
			Pays::No
		))]
        pub fn sudo_set_network_rate_limit(
            origin: OriginFor<T>,
            rate_limit: u64,
        ) -> DispatchResult {
            ensure_root(origin)?;
            pallet_subtensor::Pallet::<T>::set_network_rate_limit(rate_limit);
            log::debug!("NetworkRateLimit( rate_limit: {:?} ) ", rate_limit);
            Ok(())
        }

        /// The extrinsic sets the tempo for a subnet.
        /// It is only callable by the root account.
        /// The extrinsic will call the Subtensor pallet to set the tempo.
        #[pallet::call_index(30)]
        #[pallet::weight(Weight::from_parts(19_900_000, 0)
        .saturating_add(T::DbWeight::get().reads(1_u64))
        .saturating_add(T::DbWeight::get().writes(1_u64)))]
        pub fn sudo_set_tempo(origin: OriginFor<T>, netuid: NetUid, tempo: u16) -> DispatchResult {
            ensure_root(origin)?;
            ensure!(
                pallet_subtensor::Pallet::<T>::if_subnet_exist(netuid),
                Error::<T>::SubnetDoesNotExist
            );
            pallet_subtensor::Pallet::<T>::set_tempo(netuid, tempo);
            log::debug!("TempoSet( netuid: {:?} tempo: {:?} ) ", netuid, tempo);
            Ok(())
        }

        /// The extrinsic sets the total issuance for the network.
        /// It is only callable by the root account.
        /// The extrinsic will call the Subtensor pallet to set the issuance for the network.
        #[pallet::call_index(33)]
        #[pallet::weight((0, DispatchClass::Operational, Pays::No))]
        pub fn sudo_set_total_issuance(
            origin: OriginFor<T>,
            total_issuance: u64,
        ) -> DispatchResult {
            ensure_root(origin)?;

            pallet_subtensor::Pallet::<T>::set_total_issuance(total_issuance);

            Ok(())
        }

        /// The extrinsic sets the immunity period for the network.
        /// It is only callable by the root account.
        /// The extrinsic will call the Subtensor pallet to set the immunity period for the network.
        #[pallet::call_index(35)]
        #[pallet::weight((
			Weight::from_parts(14_000_000, 0)
				.saturating_add(T::DbWeight::get().writes(1)),
			DispatchClass::Operational,
			Pays::No
		))]
        pub fn sudo_set_network_immunity_period(
            origin: OriginFor<T>,
            immunity_period: u64,
        ) -> DispatchResult {
            ensure_root(origin)?;

            pallet_subtensor::Pallet::<T>::set_network_immunity_period(immunity_period);

            log::debug!("NetworkImmunityPeriod( period: {:?} ) ", immunity_period);

            Ok(())
        }

        /// The extrinsic sets the min lock cost for the network.
        /// It is only callable by the root account.
        /// The extrinsic will call the Subtensor pallet to set the min lock cost for the network.
        #[pallet::call_index(36)]
        #[pallet::weight((
			Weight::from_parts(14_000_000, 0)
				.saturating_add(T::DbWeight::get().writes(1)),
			DispatchClass::Operational,
			Pays::No
		))]
        pub fn sudo_set_network_min_lock_cost(
            origin: OriginFor<T>,
            lock_cost: u64,
        ) -> DispatchResult {
            ensure_root(origin)?;

            pallet_subtensor::Pallet::<T>::set_network_min_lock(lock_cost);

            log::debug!("NetworkMinLockCost( lock_cost: {:?} ) ", lock_cost);

            Ok(())
        }

        /// The extrinsic sets the subnet limit for the network.
        /// It is only callable by the root account.
        /// The extrinsic will call the Subtensor pallet to set the subnet limit.
        #[pallet::call_index(37)]
        #[pallet::weight((
			Weight::from_parts(14_000_000, 0)
				.saturating_add(T::DbWeight::get().writes(1)),
			DispatchClass::Operational,
			Pays::No
		))]
        pub fn sudo_set_subnet_limit(origin: OriginFor<T>, _max_subnets: u16) -> DispatchResult {
            ensure_root(origin)?;
            Ok(())
        }

        /// The extrinsic sets the lock reduction interval for the network.
        /// It is only callable by the root account.
        /// The extrinsic will call the Subtensor pallet to set the lock reduction interval.
        #[pallet::call_index(38)]
        #[pallet::weight((
			Weight::from_parts(14_000_000, 0)
				.saturating_add(T::DbWeight::get().writes(1)),
			DispatchClass::Operational,
			Pays::No
		))]
        pub fn sudo_set_lock_reduction_interval(
            origin: OriginFor<T>,
            interval: u64,
        ) -> DispatchResult {
            ensure_root(origin)?;

            pallet_subtensor::Pallet::<T>::set_lock_reduction_interval(interval);

            log::debug!("NetworkLockReductionInterval( interval: {:?} ) ", interval);

            Ok(())
        }

        /// The extrinsic sets the recycled RAO for a subnet.
        /// It is only callable by the root account.
        /// The extrinsic will call the Subtensor pallet to set the recycled RAO.
        #[pallet::call_index(39)]
        #[pallet::weight((0, DispatchClass::Operational, Pays::No))]
        pub fn sudo_set_rao_recycled(
            origin: OriginFor<T>,
            netuid: NetUid,
            rao_recycled: u64,
        ) -> DispatchResult {
            ensure_root(origin)?;
            ensure!(
                pallet_subtensor::Pallet::<T>::if_subnet_exist(netuid),
                Error::<T>::SubnetDoesNotExist
            );
            pallet_subtensor::Pallet::<T>::set_rao_recycled(netuid, rao_recycled);
            Ok(())
        }

        /// The extrinsic sets the weights min stake.
        /// It is only callable by the root account.
        /// The extrinsic will call the Subtensor pallet to set the weights min stake.
        #[pallet::call_index(42)]
        #[pallet::weight((0, DispatchClass::Operational, Pays::No))]
        pub fn sudo_set_stake_threshold(origin: OriginFor<T>, min_stake: u64) -> DispatchResult {
            ensure_root(origin)?;
            pallet_subtensor::Pallet::<T>::set_stake_threshold(min_stake);
            Ok(())
        }

        /// The extrinsic sets the minimum stake required for nominators.
        /// It is only callable by the root account.
        /// The extrinsic will call the Subtensor pallet to set the minimum stake required for nominators.
        #[pallet::call_index(43)]
        #[pallet::weight((0, DispatchClass::Operational, Pays::No))]
        pub fn sudo_set_nominator_min_required_stake(
            origin: OriginFor<T>,
            // The minimum stake required for nominators.
            min_stake: u64,
        ) -> DispatchResult {
            ensure_root(origin)?;
            let prev_min_stake = pallet_subtensor::Pallet::<T>::get_nominator_min_required_stake();
            log::trace!("Setting minimum stake to: {}", min_stake);
            pallet_subtensor::Pallet::<T>::set_nominator_min_required_stake(min_stake);
            if min_stake > prev_min_stake {
                log::trace!("Clearing small nominations");
                pallet_subtensor::Pallet::<T>::clear_small_nominations();
                log::trace!("Small nominations cleared");
            }
            Ok(())
        }

        /// The extrinsic sets the rate limit for delegate take transactions.
        /// It is only callable by the root account.
        /// The extrinsic will call the Subtensor pallet to set the rate limit for delegate take transactions.
        #[pallet::call_index(45)]
        #[pallet::weight((0, DispatchClass::Operational, Pays::No))]
        pub fn sudo_set_tx_delegate_take_rate_limit(
            origin: OriginFor<T>,
            tx_rate_limit: u64,
        ) -> DispatchResult {
            ensure_root(origin)?;
            pallet_subtensor::Pallet::<T>::set_tx_delegate_take_rate_limit(tx_rate_limit);
            log::debug!(
                "TxRateLimitDelegateTakeSet( tx_delegate_take_rate_limit: {:?} ) ",
                tx_rate_limit
            );
            Ok(())
        }

        /// The extrinsic sets the minimum delegate take.
        /// It is only callable by the root account.
        /// The extrinsic will call the Subtensor pallet to set the minimum delegate take.
        #[pallet::call_index(46)]
        #[pallet::weight((0, DispatchClass::Operational, Pays::No))]
        pub fn sudo_set_min_delegate_take(origin: OriginFor<T>, take: u16) -> DispatchResult {
            ensure_root(origin)?;
            pallet_subtensor::Pallet::<T>::set_min_delegate_take(take);
            log::debug!("TxMinDelegateTakeSet( tx_min_delegate_take: {:?} ) ", take);
            Ok(())
        }

        // The extrinsic sets the target stake per interval.
        // It is only callable by the root account.
        // The extrinsic will call the Subtensor pallet to set target stake per interval.
        // #[pallet::call_index(47)]
        // #[pallet::weight((0, DispatchClass::Operational, Pays::No))]
        // pub fn sudo_set_target_stakes_per_interval(
        //     origin: OriginFor<T>,
        //     target_stakes_per_interval: u64,
        // ) -> DispatchResult {
        //     ensure_root(origin)?;
        //     pallet_subtensor::Pallet::<T>::set_target_stakes_per_interval(
        //         target_stakes_per_interval,
        //     );
        //     log::debug!(
        //         "TxTargetStakesPerIntervalSet( set_target_stakes_per_interval: {:?} ) ",
        //         target_stakes_per_interval
        //     ); (DEPRECATED)
        //     Ok(())
        // } (DEPRECATED)

        /// The extrinsic enabled/disables commit/reaveal for a given subnet.
        /// It is only callable by the root account or subnet owner.
        /// The extrinsic will call the Subtensor pallet to set the value.
        #[pallet::call_index(49)]
        #[pallet::weight(Weight::from_parts(19_480_000, 0)
        .saturating_add(T::DbWeight::get().reads(1_u64))
        .saturating_add(T::DbWeight::get().writes(1_u64)))]
        pub fn sudo_set_commit_reveal_weights_enabled(
            origin: OriginFor<T>,
            netuid: NetUid,
            enabled: bool,
        ) -> DispatchResult {
            pallet_subtensor::Pallet::<T>::ensure_subnet_owner_or_root(origin, netuid)?;

            ensure!(
                pallet_subtensor::Pallet::<T>::if_subnet_exist(netuid),
                Error::<T>::SubnetDoesNotExist
            );

            pallet_subtensor::Pallet::<T>::set_commit_reveal_weights_enabled(netuid, enabled);
            log::debug!("ToggleSetWeightsCommitReveal( netuid: {:?} ) ", netuid);
            Ok(())
        }

        /// Enables or disables Liquid Alpha for a given subnet.
        ///
        /// # Parameters
        /// - `origin`: The origin of the call, which must be the root account or subnet owner.
        /// - `netuid`: The unique identifier for the subnet.
        /// - `enabled`: A boolean flag to enable or disable Liquid Alpha.
        ///
        /// # Weight
        /// This function has a fixed weight of 0 and is classified as an operational transaction that does not incur any fees.
        #[pallet::call_index(50)]
        #[pallet::weight((0, DispatchClass::Operational, Pays::No))]
        pub fn sudo_set_liquid_alpha_enabled(
            origin: OriginFor<T>,
            netuid: NetUid,
            enabled: bool,
        ) -> DispatchResult {
            pallet_subtensor::Pallet::<T>::ensure_subnet_owner_or_root(origin, netuid)?;
            pallet_subtensor::Pallet::<T>::set_liquid_alpha_enabled(netuid, enabled);
            log::debug!(
                "LiquidAlphaEnableToggled( netuid: {:?}, Enabled: {:?} ) ",
                netuid,
                enabled
            );
            Ok(())
        }

        /// Sets values for liquid alpha
        #[pallet::call_index(51)]
        #[pallet::weight((0, DispatchClass::Operational, Pays::No))]
        pub fn sudo_set_alpha_values(
            origin: OriginFor<T>,
            netuid: NetUid,
            alpha_low: u16,
            alpha_high: u16,
        ) -> DispatchResult {
            pallet_subtensor::Pallet::<T>::ensure_subnet_owner_or_root(origin.clone(), netuid)?;
            pallet_subtensor::Pallet::<T>::do_set_alpha_values(
                origin, netuid, alpha_low, alpha_high,
            )
        }

        // DEPRECATED
        // #[pallet::call_index(52)]
        // #[pallet::weight((0, DispatchClass::Operational, Pays::No))]
        // pub fn sudo_set_hotkey_emission_tempo(
        //     origin: OriginFor<T>,
        //     emission_tempo: u64,
        // ) -> DispatchResult {
        //     ensure_root(origin)?;
        //     pallet_subtensor::Pallet::<T>::set_hotkey_emission_tempo(emission_tempo);
        //     log::debug!(
        //         "HotkeyEmissionTempoSet( emission_tempo: {:?} )",
        //         emission_tempo
        //     );
        //     Ok(())
        // }

        /// Sets the maximum stake allowed for a specific network.
        ///
        /// This function allows the root account to set the maximum stake for a given network.
        /// It updates the network's maximum stake value and logs the change.
        ///
        /// # Arguments
        ///
        /// * `origin` - The origin of the call, which must be the root account.
        /// * `netuid` - The unique identifier of the network.
        /// * `max_stake` - The new maximum stake value to set.
        ///
        /// # Returns
        ///
        /// Returns `Ok(())` if the operation is successful, or an error if it fails.
        ///
        /// # Example
        ///
        ///
        /// # Notes
        ///
        /// - This function can only be called by the root account.
        /// - The `netuid` should correspond to an existing network.
        ///
        /// # TODO
        ///
        // - Consider adding a check to ensure the `netuid` corresponds to an existing network.
        // - Implement a mechanism to gradually adjust the max stake to prevent sudden changes.
        // #[pallet::weight(<T as Config>::WeightInfo::sudo_set_network_max_stake())]
        #[pallet::call_index(53)]
        #[pallet::weight((0, DispatchClass::Operational, Pays::No))]
        pub fn sudo_set_network_max_stake(
            origin: OriginFor<T>,
            _netuid: NetUid,
            _max_stake: u64,
        ) -> DispatchResult {
            // Ensure the call is made by the root account
            ensure_root(origin)?;
            Ok(())
        }

        /// Sets the duration of the coldkey swap schedule.
        ///
        /// This extrinsic allows the root account to set the duration for the coldkey swap schedule.
        /// The coldkey swap schedule determines how long it takes for a coldkey swap operation to complete.
        ///
        /// # Arguments
        /// * `origin` - The origin of the call, which must be the root account.
        /// * `duration` - The new duration for the coldkey swap schedule, in number of blocks.
        ///
        /// # Errors
        /// * `BadOrigin` - If the caller is not the root account.
        ///
        /// # Weight
        /// Weight is handled by the `#[pallet::weight]` attribute.
        #[pallet::call_index(54)]
        #[pallet::weight((0, DispatchClass::Operational, Pays::No))]
        pub fn sudo_set_coldkey_swap_schedule_duration(
            origin: OriginFor<T>,
            duration: BlockNumberFor<T>,
        ) -> DispatchResult {
            // Ensure the call is made by the root account
            ensure_root(origin)?;

            // Set the new duration of schedule coldkey swap
            pallet_subtensor::Pallet::<T>::set_coldkey_swap_schedule_duration(duration);

            // Log the change
            log::trace!("ColdkeySwapScheduleDurationSet( duration: {:?} )", duration);

            Ok(())
        }

        /// Sets the duration of the dissolve network schedule.
        ///
        /// This extrinsic allows the root account to set the duration for the dissolve network schedule.
        /// The dissolve network schedule determines how long it takes for a network dissolution operation to complete.
        ///
        /// # Arguments
        /// * `origin` - The origin of the call, which must be the root account.
        /// * `duration` - The new duration for the dissolve network schedule, in number of blocks.
        ///
        /// # Errors
        /// * `BadOrigin` - If the caller is not the root account.
        ///
        /// # Weight
        /// Weight is handled by the `#[pallet::weight]` attribute.
        #[pallet::call_index(55)]
        #[pallet::weight((0, DispatchClass::Operational, Pays::No))]
        pub fn sudo_set_dissolve_network_schedule_duration(
            origin: OriginFor<T>,
            duration: BlockNumberFor<T>,
        ) -> DispatchResult {
            // Ensure the call is made by the root account
            ensure_root(origin)?;

            // Set the duration of schedule dissolve network
            pallet_subtensor::Pallet::<T>::set_dissolve_network_schedule_duration(duration);

            // Log the change
            log::trace!(
                "DissolveNetworkScheduleDurationSet( duration: {:?} )",
                duration
            );

            Ok(())
        }

        /// Sets the commit-reveal weights periods for a specific subnet.
        ///
        /// This extrinsic allows the subnet owner or root account to set the duration (in epochs) during which committed weights must be revealed.
        /// The commit-reveal mechanism ensures that users commit weights in advance and reveal them only within a specified period.
        ///
        /// # Arguments
        /// * `origin` - The origin of the call, which must be the subnet owner or the root account.
        /// * `netuid` - The unique identifier of the subnet for which the periods are being set.
        /// * `periods` - The number of epochs that define the commit-reveal period.
        ///
        /// # Errors
        /// * `BadOrigin` - If the caller is neither the subnet owner nor the root account.
        /// * `SubnetDoesNotExist` - If the specified subnet does not exist.
        ///
        /// # Weight
        /// Weight is handled by the `#[pallet::weight]` attribute.
        #[pallet::call_index(57)]
        #[pallet::weight(Weight::from_parts(20_490_000, 0)
        .saturating_add(T::DbWeight::get().reads(1_u64))
        .saturating_add(T::DbWeight::get().writes(1_u64)))]
        pub fn sudo_set_commit_reveal_weights_interval(
            origin: OriginFor<T>,
            netuid: NetUid,
            interval: u64,
        ) -> DispatchResult {
            pallet_subtensor::Pallet::<T>::ensure_subnet_owner_or_root(origin, netuid)?;

            ensure!(
                pallet_subtensor::Pallet::<T>::if_subnet_exist(netuid),
                Error::<T>::SubnetDoesNotExist
            );

            pallet_subtensor::Pallet::<T>::set_reveal_period(netuid, interval);
            log::debug!(
                "SetWeightCommitInterval( netuid: {:?}, interval: {:?} ) ",
                netuid,
                interval
            );
            Ok(())
        }

        /// Sets the EVM ChainID.
        ///
        /// # Arguments
        /// * `origin` - The origin of the call, which must be the subnet owner or the root account.
        /// * `chainId` - The u64 chain ID
        ///
        /// # Errors
        /// * `BadOrigin` - If the caller is neither the subnet owner nor the root account.
        ///
        /// # Weight
        /// Weight is handled by the `#[pallet::weight]` attribute.
        #[pallet::call_index(58)]
        #[pallet::weight(Weight::from_parts(27_199_000, 0)
        .saturating_add(T::DbWeight::get().reads(1_u64))
        .saturating_add(T::DbWeight::get().writes(1_u64)))]
        pub fn sudo_set_evm_chain_id(origin: OriginFor<T>, chain_id: u64) -> DispatchResult {
            // Ensure the call is made by the root account
            ensure_root(origin)?;

            ChainId::<T>::set(chain_id);
            Ok(())
        }

        /// A public interface for `pallet_grandpa::Pallet::schedule_grandpa_change`.
        ///
        /// Schedule a change in the authorities.
        ///
        /// The change will be applied at the end of execution of the block `in_blocks` after the
        /// current block. This value may be 0, in which case the change is applied at the end of
        /// the current block.
        ///
        /// If the `forced` parameter is defined, this indicates that the current set has been
        /// synchronously determined to be offline and that after `in_blocks` the given change
        /// should be applied. The given block number indicates the median last finalized block
        /// number and it should be used as the canon block when starting the new grandpa voter.
        ///
        /// No change should be signaled while any change is pending. Returns an error if a change
        /// is already pending.
        #[pallet::call_index(59)]
        #[pallet::weight(Weight::from_parts(11_550_000, 0)
        .saturating_add(T::DbWeight::get().reads(1_u64))
        .saturating_add(T::DbWeight::get().writes(1_u64)))]
        pub fn schedule_grandpa_change(
            origin: OriginFor<T>,
            // grandpa ID is always the same type, so we don't need to parametrize it via `Config`
            next_authorities: AuthorityList,
            in_blocks: BlockNumberFor<T>,
            forced: Option<BlockNumberFor<T>>,
        ) -> DispatchResult {
            ensure_root(origin)?;
            T::Grandpa::schedule_change(next_authorities, in_blocks, forced)
        }

        /// Enable or disable atomic alpha transfers for a given subnet.
        ///
        /// # Parameters
        /// - `origin`: The origin of the call, which must be the root account or subnet owner.
        /// - `netuid`: The unique identifier for the subnet.
        /// - `enabled`: A boolean flag to enable or disable Liquid Alpha.
        ///
        /// # Weight
        /// This function has a fixed weight of 0 and is classified as an operational transaction that does not incur any fees.
        #[pallet::call_index(61)]
        #[pallet::weight((0, DispatchClass::Operational, Pays::No))]
        pub fn sudo_set_toggle_transfer(
            origin: OriginFor<T>,
            netuid: NetUid,
            toggle: bool,
        ) -> DispatchResult {
            pallet_subtensor::Pallet::<T>::ensure_subnet_owner_or_root(origin, netuid)?;
            pallet_subtensor::Pallet::<T>::toggle_transfer(netuid, toggle)
        }

        /// Toggles the enablement of an EVM precompile.
        ///
        /// # Arguments
        /// * `origin` - The origin of the call, which must be the root account.
        /// * `precompile_id` - The identifier of the EVM precompile to toggle.
        /// * `enabled` - The new enablement state of the precompile.
        ///
        /// # Errors
        /// * `BadOrigin` - If the caller is not the root account.
        ///
        /// # Weight
        /// Weight is handled by the `#[pallet::weight]` attribute.
        #[pallet::call_index(62)]
        #[pallet::weight((0, DispatchClass::Operational, Pays::No))]
        pub fn sudo_toggle_evm_precompile(
            origin: OriginFor<T>,
            precompile_id: PrecompileEnum,
            enabled: bool,
        ) -> DispatchResult {
            ensure_root(origin)?;
            if PrecompileEnable::<T>::get(precompile_id) != enabled {
                PrecompileEnable::<T>::insert(precompile_id, enabled);
                Self::deposit_event(Event::PrecompileUpdated {
                    precompile_id,
                    enabled,
                });
            }
            Ok(())
        }

        ///
        ///
        /// # Arguments
        /// * `origin` - The origin of the call, which must be the root account.
        /// * `alpha` - The new moving alpha value for the SubnetMovingAlpha.
        ///
        /// # Errors
        /// * `BadOrigin` - If the caller is not the root account.
        ///
        /// # Weight
        /// Weight is handled by the `#[pallet::weight]` attribute.
        #[pallet::call_index(63)]
        #[pallet::weight((0, DispatchClass::Operational, Pays::No))]
        pub fn sudo_set_subnet_moving_alpha(origin: OriginFor<T>, alpha: I96F32) -> DispatchResult {
            ensure_root(origin)?;
            pallet_subtensor::SubnetMovingAlpha::<T>::set(alpha);

            log::debug!("SubnetMovingAlphaSet( alpha: {:?} )", alpha);
            Ok(())
        }

        /// Change the SubnetOwnerHotkey for a given subnet.
        ///
        /// # Arguments
        /// * `origin` - The origin of the call, which must be the subnet owner.
        /// * `netuid` - The unique identifier for the subnet.
        /// * `hotkey` - The new hotkey for the subnet owner.
        ///
        /// # Errors
        /// * `BadOrigin` - If the caller is not the subnet owner or root account.
        ///
        /// # Weight
        /// Weight is handled by the `#[pallet::weight]` attribute.
        #[pallet::call_index(64)]
        #[pallet::weight((0, DispatchClass::Operational, Pays::No))]
        pub fn sudo_set_subnet_owner_hotkey(
            origin: OriginFor<T>,
            netuid: NetUid,
            hotkey: T::AccountId,
        ) -> DispatchResult {
            pallet_subtensor::Pallet::<T>::ensure_subnet_owner(origin.clone(), netuid)?;
            pallet_subtensor::Pallet::<T>::set_subnet_owner_hotkey(netuid, &hotkey);

            log::debug!(
                "SubnetOwnerHotkeySet( netuid: {:?}, hotkey: {:?} )",
                netuid,
                hotkey
            );
            Ok(())
        }

        ///
        ///
        /// # Arguments
        /// * `origin` - The origin of the call, which must be the root account.
        /// * `ema_alpha_period` - Number of blocks for EMA price to halve
        ///
        /// # Errors
        /// * `BadOrigin` - If the caller is not the root account.
        ///
        /// # Weight
        /// Weight is handled by the `#[pallet::weight]` attribute.
        #[pallet::call_index(65)]
        #[pallet::weight((0, DispatchClass::Operational, Pays::No))]
        pub fn sudo_set_ema_price_halving_period(
            origin: OriginFor<T>,
            netuid: NetUid,
            ema_halving: u64,
        ) -> DispatchResult {
            ensure_root(origin)?;
            pallet_subtensor::EMAPriceHalvingBlocks::<T>::set(netuid, ema_halving);

            log::debug!(
                "EMAPriceHalvingBlocks( netuid: {:?}, ema_halving: {:?} )",
                netuid,
                ema_halving
            );
            Ok(())
        }

        ///
        ///
        /// # Arguments
        /// * `origin` - The origin of the call, which must be the root account.
        /// * `netuid` - The unique identifier for the subnet.
        /// * `steepness` - The new steepness for the alpha sigmoid function.
        ///
        /// # Errors
        /// * `BadOrigin` - If the caller is not the root account.
        /// # Weight
        /// Weight is handled by the `#[pallet::weight]` attribute.
        #[pallet::call_index(68)]
        #[pallet::weight((0, DispatchClass::Operational, Pays::No))]
        pub fn sudo_set_alpha_sigmoid_steepness(
            origin: OriginFor<T>,
            netuid: NetUid,
            steepness: u16,
        ) -> DispatchResult {
            ensure_root(origin)?;
            pallet_subtensor::Pallet::<T>::set_alpha_sigmoid_steepness(netuid, steepness);

            log::debug!(
                "AlphaSigmoidSteepnessSet( netuid: {:?}, steepness: {:?} )",
                netuid,
                steepness
            );
            Ok(())
        }

        /// Enables or disables Yuma3 for a given subnet.
        ///
        /// # Parameters
        /// - `origin`: The origin of the call, which must be the root account or subnet owner.
        /// - `netuid`: The unique identifier for the subnet.
        /// - `enabled`: A boolean flag to enable or disable Yuma3.
        ///
        /// # Weight
        /// This function has a fixed weight of 0 and is classified as an operational transaction that does not incur any fees.
        #[pallet::call_index(69)]
        #[pallet::weight((0, DispatchClass::Operational, Pays::No))]
        pub fn sudo_set_yuma3_enabled(
            origin: OriginFor<T>,
            netuid: NetUid,
            enabled: bool,
        ) -> DispatchResult {
            pallet_subtensor::Pallet::<T>::ensure_subnet_owner_or_root(origin, netuid)?;
            pallet_subtensor::Pallet::<T>::set_yuma3_enabled(netuid, enabled);

            Self::deposit_event(Event::Yuma3EnableToggled { netuid, enabled });
            log::debug!(
                "Yuma3EnableToggled( netuid: {:?}, Enabled: {:?} ) ",
                netuid,
                enabled
            );
            Ok(())
        }

        /// Enables or disables Bonds Reset for a given subnet.
        ///
        /// # Parameters
        /// - `origin`: The origin of the call, which must be the root account or subnet owner.
        /// - `netuid`: The unique identifier for the subnet.
        /// - `enabled`: A boolean flag to enable or disable Bonds Reset.
        ///
        /// # Weight
        /// This function has a fixed weight of 0 and is classified as an operational transaction that does not incur any fees.
        #[pallet::call_index(70)]
        #[pallet::weight((0, DispatchClass::Operational, Pays::No))]
        pub fn sudo_set_bonds_reset_enabled(
            origin: OriginFor<T>,
            netuid: NetUid,
            enabled: bool,
        ) -> DispatchResult {
            pallet_subtensor::Pallet::<T>::ensure_subnet_owner_or_root(origin, netuid)?;
            pallet_subtensor::Pallet::<T>::set_bonds_reset(netuid, enabled);

            Self::deposit_event(Event::BondsResetToggled { netuid, enabled });
            log::debug!(
                "BondsResetToggled( netuid: {:?} bonds_reset: {:?} ) ",
                netuid,
                enabled
            );
            Ok(())
        }

        /// Sets or updates the hotkey account associated with the owner of a specific subnet.
        ///
        /// This function allows either the root origin or the current subnet owner to set or update
        /// the hotkey for a given subnet. The subnet must already exist. To prevent abuse, the call is
        /// rate-limited to once per configured interval (default: one week) per subnet.
        ///
        /// # Parameters
        /// - `origin`: The dispatch origin of the call. Must be either root or the current owner of the subnet.
        /// - `netuid`: The unique identifier of the subnet whose owner hotkey is being set.
        /// - `hotkey`: The new hotkey account to associate with the subnet owner.
        ///
        /// # Returns
        /// - `DispatchResult`: Returns `Ok(())` if the hotkey was successfully set, or an appropriate error otherwise.
        ///
        /// # Errors
        /// - `Error::SubnetNotExists`: If the specified subnet does not exist.
        /// - `Error::TxRateLimitExceeded`: If the function is called more frequently than the allowed rate limit.
        ///
        /// # Access Control
        /// Only callable by:
        /// - Root origin, or
        /// - The coldkey account that owns the subnet.
        ///
        /// # Storage
        /// - Updates [`SubnetOwnerHotkey`] for the given `netuid`.
        /// - Reads and updates [`LastRateLimitedBlock`] for rate-limiting.
        /// - Reads [`DefaultSetSNOwnerHotkeyRateLimit`] to determine the interval between allowed updates.
        ///
        /// # Rate Limiting
        /// This function is rate-limited to one call per subnet per interval (e.g., one week).
        #[pallet::call_index(67)]
        #[pallet::weight((0, DispatchClass::Operational, Pays::No))]
        pub fn sudo_set_sn_owner_hotkey(
            origin: OriginFor<T>,
            netuid: NetUid,
            hotkey: T::AccountId,
        ) -> DispatchResult {
            pallet_subtensor::Pallet::<T>::do_set_sn_owner_hotkey(origin, netuid, &hotkey)
        }

        /// Enables or disables subtoken trading for a given subnet.
        ///
        /// # Arguments
        /// * `origin` - The origin of the call, which must be the root account.
        /// * `netuid` - The unique identifier of the subnet.
        /// * `subtoken_enabled` - A boolean indicating whether subtoken trading should be enabled or disabled.
        ///
        /// # Errors
        /// * `BadOrigin` - If the caller is not the root account.
        ///
        /// # Weight
        /// Weight is handled by the `#[pallet::weight]` attribute.
        #[pallet::call_index(66)]
        #[pallet::weight((0, DispatchClass::Operational, Pays::No))]
        pub fn sudo_set_subtoken_enabled(
            origin: OriginFor<T>,
            netuid: NetUid,
            subtoken_enabled: bool,
        ) -> DispatchResult {
            ensure_root(origin)?;
            pallet_subtensor::SubtokenEnabled::<T>::set(netuid, subtoken_enabled);

            log::debug!(
                "SubtokenEnabled( netuid: {:?}, subtoken_enabled: {:?} )",
                netuid,
                subtoken_enabled
            );
            Ok(())
        }

        /// Global setter that delegates to the same logic every individual
        /// admin-extrinsic had before.  
        /// Nothing has been dropped: every `ensure!`, rate-limit guard,
        /// log line, and event remains intact.
        #[pallet::call_index(71)]
        #[pallet::weight((0, DispatchClass::Operational, Pays::No))]
        pub fn sudo_set_hyperparameter(
            origin: OriginFor<T>,
            param: HyperParam<T>,
        ) -> DispatchResult {
            match param {
                /*────────── NETWORK-WIDE ──────────*/
                HyperParam::DefaultTake(v) => {
                    ensure_root(origin.clone())?;
                    pallet_subtensor::Pallet::<T>::set_max_delegate_take(v);
                    log::debug!("DefaultTakeSet( default_take: {:?} ) ", v);
                    Ok(())
                }

                HyperParam::TxRateLimit(v) => {
                    ensure_root(origin.clone())?;
                    pallet_subtensor::Pallet::<T>::set_tx_rate_limit(v);
                    log::debug!("TxRateLimitSet( tx_rate_limit: {:?} ) ", v);
                    Ok(())
                }

                HyperParam::NetworkRateLimit(v) => {
                    ensure_root(origin.clone())?;
                    pallet_subtensor::Pallet::<T>::set_network_rate_limit(v);
                    log::debug!("NetworkRateLimit( rate_limit: {:?} ) ", v);
                    Ok(())
                }

                HyperParam::TotalIssuance(v) => {
                    ensure_root(origin.clone())?;
                    pallet_subtensor::Pallet::<T>::set_total_issuance(v);
                    Ok(())
                }

                HyperParam::NetworkImmunityPeriod(v) => {
                    ensure_root(origin.clone())?;
                    pallet_subtensor::Pallet::<T>::set_network_immunity_period(v);
                    log::debug!("NetworkImmunityPeriod( period: {:?} ) ", v);
                    Ok(())
                }

                HyperParam::NetworkMinLockCost(v) => {
                    ensure_root(origin.clone())?;
                    pallet_subtensor::Pallet::<T>::set_network_min_lock(v);
                    log::debug!("NetworkMinLockCost( lock_cost: {:?} ) ", v);
                    Ok(())
                }

                HyperParam::SubnetLimit(_) => {
                    ensure_root(origin)?;
                    Ok(())
                }

                HyperParam::LockReductionInterval(v) => {
                    ensure_root(origin.clone())?;
                    pallet_subtensor::Pallet::<T>::set_lock_reduction_interval(v);
                    log::debug!("NetworkLockReductionInterval( interval: {:?} ) ", v);
                    Ok(())
                }

                HyperParam::StakeThreshold(v) => {
                    ensure_root(origin.clone())?;
                    pallet_subtensor::Pallet::<T>::set_stake_threshold(v);
                    Ok(())
                }

                HyperParam::NominatorMinRequiredStake(v) => {
                    ensure_root(origin.clone())?;
                    let prev = pallet_subtensor::Pallet::<T>::get_nominator_min_required_stake();
                    pallet_subtensor::Pallet::<T>::set_nominator_min_required_stake(v);
                    if v > prev {
                        log::trace!("Clearing small nominations");
                        pallet_subtensor::Pallet::<T>::clear_small_nominations();
                        log::trace!("Small nominations cleared");
                    }
                    Ok(())
                }

                HyperParam::TxDelegateTakeRateLimit(v) => {
                    ensure_root(origin.clone())?;
                    pallet_subtensor::Pallet::<T>::set_tx_delegate_take_rate_limit(v);
                    log::debug!(
                        "TxRateLimitDelegateTakeSet( tx_delegate_take_rate_limit: {:?} ) ",
                        v
                    );
                    Ok(())
                }

                HyperParam::MinDelegateTake(v) => {
                    ensure_root(origin.clone())?;
                    pallet_subtensor::Pallet::<T>::set_min_delegate_take(v);
                    log::debug!("TxMinDelegateTakeSet( tx_min_delegate_take: {:?} ) ", v);
                    Ok(())
                }

                HyperParam::EvmChainId(v) => {
                    ensure_root(origin.clone())?;
                    pallet_evm_chain_id::ChainId::<T>::set(v);
                    Ok(())
                }

                HyperParam::SubnetOwnerCut(v) => {
                    ensure_root(origin.clone())?;
                    pallet_subtensor::Pallet::<T>::set_subnet_owner_cut(v);
                    log::debug!("SubnetOwnerCut( subnet_owner_cut: {:?} ) ", v);
                    Ok(())
                }

                HyperParam::SubnetMovingAlpha(v) => {
                    ensure_root(origin.clone())?;
                    pallet_subtensor::SubnetMovingAlpha::<T>::set(v);
                    log::debug!("SubnetMovingAlphaSet( alpha: {:?} )", v);
                    Ok(())
                }

                HyperParam::ColdkeySwapScheduleDuration(d) => {
                    ensure_root(origin.clone())?;
                    pallet_subtensor::Pallet::<T>::set_coldkey_swap_schedule_duration(d);
                    log::trace!("ColdkeySwapScheduleDurationSet( duration: {:?} )", d);
                    Ok(())
                }

                HyperParam::DissolveNetworkScheduleDuration(d) => {
                    ensure_root(origin.clone())?;
                    pallet_subtensor::Pallet::<T>::set_dissolve_network_schedule_duration(d);
                    log::trace!("DissolveNetworkScheduleDurationSet( duration: {:?} )", d);
                    Ok(())
                }

                /*──────────── CONSENSUS ───────────*/
                HyperParam::ScheduleGrandpaChange {
                    next_authorities,
                    in_blocks,
                    forced,
                } => {
                    ensure_root(origin)?;
                    T::Grandpa::schedule_change(next_authorities, in_blocks, forced)
                }

                /*──────── PRECOMPILE GATE ─────────*/
                HyperParam::ToggleEvmPrecompile {
                    precompile_id,
                    enabled,
                } => {
                    ensure_root(origin)?;
                    if PrecompileEnable::<T>::get(precompile_id) != enabled {
                        PrecompileEnable::<T>::insert(precompile_id, enabled);
                        Self::deposit_event(Event::PrecompileUpdated {
                            precompile_id,
                            enabled,
                        });
                    }
                    Ok(())
                }

                /*────────── SUBNET-SPECIFIC ───────*/
                HyperParam::ServingRateLimit { netuid, limit } => {
                    pallet_subtensor::Pallet::<T>::ensure_subnet_owner_or_root(origin, netuid)?;
                    pallet_subtensor::Pallet::<T>::set_serving_rate_limit(netuid, limit);
                    log::debug!("ServingRateLimitSet( serving_rate_limit: {:?} ) ", limit);
                    Ok(())
                }

                HyperParam::MinDifficulty { netuid, value } => {
                    ensure_root(origin.clone())?;
                    ensure!(
                        pallet_subtensor::Pallet::<T>::if_subnet_exist(netuid),
                        Error::<T>::SubnetDoesNotExist
                    );
                    pallet_subtensor::Pallet::<T>::set_min_difficulty(netuid, value);
                    log::debug!(
                        "MinDifficultySet( netuid: {:?} min_difficulty: {:?} ) ",
                        netuid,
                        value
                    );
                    Ok(())
                }

                HyperParam::MaxDifficulty { netuid, value } => {
                    pallet_subtensor::Pallet::<T>::ensure_subnet_owner_or_root(
                        origin.clone(),
                        netuid,
                    )?;
                    ensure!(
                        pallet_subtensor::Pallet::<T>::if_subnet_exist(netuid),
                        Error::<T>::SubnetDoesNotExist
                    );
                    pallet_subtensor::Pallet::<T>::set_max_difficulty(netuid, value);
                    log::debug!(
                        "MaxDifficultySet( netuid: {:?} max_difficulty: {:?} ) ",
                        netuid,
                        value
                    );
                    Ok(())
                }

                HyperParam::WeightsVersionKey { netuid, key } => {
                    pallet_subtensor::Pallet::<T>::ensure_subnet_owner_or_root(
                        origin.clone(),
                        netuid,
                    )?;

                    // Extra rate-limit logic (unchanged)
                    if let Ok(RawOrigin::Signed(who)) = origin.clone().into() {
                        ensure!(
                            pallet_subtensor::Pallet::<T>::passes_rate_limit_on_subnet(
                                &TransactionType::SetWeightsVersionKey,
                                &who,
                                netuid,
                            ),
                            pallet_subtensor::Error::<T>::TxRateLimitExceeded
                        );

                        let block_now = pallet_subtensor::Pallet::<T>::get_current_block_as_u64();
                        pallet_subtensor::Pallet::<T>::set_last_transaction_block_on_subnet(
                            &who,
                            netuid,
                            &TransactionType::SetWeightsVersionKey,
                            block_now,
                        );
                    }

                    pallet_subtensor::Pallet::<T>::set_weights_version_key(netuid, key);
                    log::debug!(
                        "WeightsVersionKeySet( netuid: {:?} key: {:?} ) ",
                        netuid,
                        key
                    );
                    Ok(())
                }

                HyperParam::WeightsSetRateLimit { netuid, limit } => {
                    ensure_root(origin)?;
                    ensure!(
                        pallet_subtensor::Pallet::<T>::if_subnet_exist(netuid),
                        Error::<T>::SubnetDoesNotExist
                    );
                    pallet_subtensor::Pallet::<T>::set_weights_set_rate_limit(netuid, limit);
                    log::debug!(
                        "WeightsSetRateLimitSet( netuid: {:?} limit: {:?} ) ",
                        netuid,
                        limit
                    );
                    Ok(())
                }

                HyperParam::AdjustmentInterval { netuid, interval } => {
                    ensure_root(origin)?;
                    ensure!(
                        pallet_subtensor::Pallet::<T>::if_subnet_exist(netuid),
                        Error::<T>::SubnetDoesNotExist
                    );
                    pallet_subtensor::Pallet::<T>::set_adjustment_interval(netuid, interval);
                    log::debug!(
                        "AdjustmentIntervalSet( netuid: {:?} interval: {:?} ) ",
                        netuid,
                        interval
                    );
                    Ok(())
                }

                HyperParam::AdjustmentAlpha { netuid, alpha } => {
                    pallet_subtensor::Pallet::<T>::ensure_subnet_owner_or_root(origin, netuid)?;
                    ensure!(
                        pallet_subtensor::Pallet::<T>::if_subnet_exist(netuid),
                        Error::<T>::SubnetDoesNotExist
                    );
                    pallet_subtensor::Pallet::<T>::set_adjustment_alpha(netuid, alpha);
                    log::debug!(
                        "AdjustmentAlphaSet( netuid: {:?} alpha: {:?} ) ",
                        netuid,
                        alpha
                    );
                    Ok(())
                }

                HyperParam::MaxWeightLimit { netuid, limit } => {
                    pallet_subtensor::Pallet::<T>::ensure_subnet_owner_or_root(origin, netuid)?;
                    ensure!(
                        pallet_subtensor::Pallet::<T>::if_subnet_exist(netuid),
                        Error::<T>::SubnetDoesNotExist
                    );
                    pallet_subtensor::Pallet::<T>::set_max_weight_limit(netuid, limit);
                    log::debug!(
                        "MaxWeightLimitSet( netuid: {:?} limit: {:?} ) ",
                        netuid,
                        limit
                    );
                    Ok(())
                }

                HyperParam::ImmunityPeriod { netuid, period } => {
                    pallet_subtensor::Pallet::<T>::ensure_subnet_owner_or_root(origin, netuid)?;
                    ensure!(
                        pallet_subtensor::Pallet::<T>::if_subnet_exist(netuid),
                        Error::<T>::SubnetDoesNotExist
                    );
                    pallet_subtensor::Pallet::<T>::set_immunity_period(netuid, period);
                    log::debug!(
                        "ImmunityPeriodSet( netuid: {:?} immunity_period: {:?} ) ",
                        netuid,
                        period
                    );
                    Ok(())
                }

                HyperParam::MinAllowedWeights { netuid, min } => {
                    pallet_subtensor::Pallet::<T>::ensure_subnet_owner_or_root(origin, netuid)?;
                    ensure!(
                        pallet_subtensor::Pallet::<T>::if_subnet_exist(netuid),
                        Error::<T>::SubnetDoesNotExist
                    );
                    pallet_subtensor::Pallet::<T>::set_min_allowed_weights(netuid, min);
                    log::debug!(
                        "MinAllowedWeightSet( netuid: {:?} min_allowed_weights: {:?} ) ",
                        netuid,
                        min
                    );
                    Ok(())
                }

                HyperParam::MaxAllowedUids { netuid, max } => {
                    ensure_root(origin.clone())?;
                    ensure!(
                        pallet_subtensor::Pallet::<T>::if_subnet_exist(netuid),
                        Error::<T>::SubnetDoesNotExist
                    );
                    ensure!(
                        pallet_subtensor::Pallet::<T>::get_subnetwork_n(netuid) < max,
                        Error::<T>::MaxAllowedUIdsLessThanCurrentUIds
                    );
                    pallet_subtensor::Pallet::<T>::set_max_allowed_uids(netuid, max);
                    log::debug!(
                        "MaxAllowedUidsSet( netuid: {:?} max_allowed_uids: {:?} ) ",
                        netuid,
                        max
                    );
                    Ok(())
                }

                HyperParam::Kappa { netuid, kappa } => {
                    pallet_subtensor::Pallet::<T>::ensure_subnet_owner_or_root(origin, netuid)?;
                    ensure!(
                        pallet_subtensor::Pallet::<T>::if_subnet_exist(netuid),
                        Error::<T>::SubnetDoesNotExist
                    );
                    pallet_subtensor::Pallet::<T>::set_kappa(netuid, kappa);
                    log::debug!("KappaSet( netuid: {:?} kappa: {:?} ) ", netuid, kappa);
                    Ok(())
                }

                HyperParam::Rho { netuid, rho } => {
                    pallet_subtensor::Pallet::<T>::ensure_subnet_owner_or_root(origin, netuid)?;
                    ensure!(
                        pallet_subtensor::Pallet::<T>::if_subnet_exist(netuid),
                        Error::<T>::SubnetDoesNotExist
                    );
                    pallet_subtensor::Pallet::<T>::set_rho(netuid, rho);
                    log::debug!("RhoSet( netuid: {:?} rho: {:?} ) ", netuid, rho);
                    Ok(())
                }

                HyperParam::ActivityCutoff { netuid, cutoff } => {
                    pallet_subtensor::Pallet::<T>::ensure_subnet_owner_or_root(
                        origin.clone(),
                        netuid,
                    )?;
                    ensure!(
                        pallet_subtensor::Pallet::<T>::if_subnet_exist(netuid),
                        Error::<T>::SubnetDoesNotExist
                    );
                    ensure!(
                        cutoff >= pallet_subtensor::MinActivityCutoff::<T>::get(),
                        pallet_subtensor::Error::<T>::ActivityCutoffTooLow
                    );
                    pallet_subtensor::Pallet::<T>::set_activity_cutoff(netuid, cutoff);
                    log::debug!(
                        "ActivityCutoffSet( netuid: {:?} cutoff: {:?} ) ",
                        netuid,
                        cutoff
                    );
                    Ok(())
                }

                HyperParam::NetworkRegistrationAllowed { netuid, allowed } => {
                    pallet_subtensor::Pallet::<T>::ensure_subnet_owner_or_root(origin, netuid)?;
                    pallet_subtensor::Pallet::<T>::set_network_registration_allowed(
                        netuid, allowed,
                    );
                    log::debug!(
                        "NetworkRegistrationAllowed( registration_allowed: {:?} ) ",
                        allowed
                    );
                    Ok(())
                }

                HyperParam::NetworkPowRegistrationAllowed { netuid, allowed } => {
                    pallet_subtensor::Pallet::<T>::ensure_subnet_owner_or_root(origin, netuid)?;
                    pallet_subtensor::Pallet::<T>::set_network_pow_registration_allowed(
                        netuid, allowed,
                    );
                    log::debug!(
                        "NetworkPowRegistrationAllowed( registration_allowed: {:?} ) ",
                        allowed
                    );
                    Ok(())
                }

                HyperParam::TargetRegistrationsPerInterval { netuid, target } => {
                    ensure_root(origin.clone())?;
                    ensure!(
                        pallet_subtensor::Pallet::<T>::if_subnet_exist(netuid),
                        Error::<T>::SubnetDoesNotExist
                    );
                    pallet_subtensor::Pallet::<T>::set_target_registrations_per_interval(
                        netuid, target,
                    );
                    log::debug!(
                        "RegistrationPerIntervalSet( netuid: {:?} target: {:?} ) ",
                        netuid,
                        target
                    );
                    Ok(())
                }

                HyperParam::MinBurn { netuid, min } => {
                    ensure_root(origin.clone())?;
                    ensure!(
                        pallet_subtensor::Pallet::<T>::if_subnet_exist(netuid),
                        Error::<T>::SubnetDoesNotExist
                    );
                    pallet_subtensor::Pallet::<T>::set_min_burn(netuid, min);
                    log::debug!("MinBurnSet( netuid: {:?} min_burn: {:?} ) ", netuid, min);
                    Ok(())
                }

                HyperParam::MaxBurn { netuid, max } => {
                    ensure_root(origin.clone())?;
                    ensure!(
                        pallet_subtensor::Pallet::<T>::if_subnet_exist(netuid),
                        Error::<T>::SubnetDoesNotExist
                    );
                    pallet_subtensor::Pallet::<T>::set_max_burn(netuid, max);
                    log::debug!("MaxBurnSet( netuid: {:?} max_burn: {:?} ) ", netuid, max);
                    Ok(())
                }

                HyperParam::Difficulty { netuid, value } => {
                    ensure_root(origin.clone())?;
                    ensure!(
                        pallet_subtensor::Pallet::<T>::if_subnet_exist(netuid),
                        Error::<T>::SubnetDoesNotExist
                    );
                    pallet_subtensor::Pallet::<T>::set_difficulty(netuid, value);
                    log::debug!(
                        "DifficultySet( netuid: {:?} difficulty: {:?} ) ",
                        netuid,
                        value
                    );
                    Ok(())
                }

                HyperParam::MaxAllowedValidators { netuid, max } => {
                    ensure_root(origin.clone())?;
                    ensure!(
                        pallet_subtensor::Pallet::<T>::if_subnet_exist(netuid),
                        Error::<T>::SubnetDoesNotExist
                    );
                    ensure!(
                        max <= pallet_subtensor::Pallet::<T>::get_max_allowed_uids(netuid),
                        Error::<T>::MaxValidatorsLargerThanMaxUIds
                    );
                    pallet_subtensor::Pallet::<T>::set_max_allowed_validators(netuid, max);
                    log::debug!(
                        "MaxAllowedValidatorsSet( netuid: {:?} max_allowed_validators: {:?} ) ",
                        netuid,
                        max
                    );
                    Ok(())
                }

                HyperParam::BondsMovingAverage { netuid, ma } => {
                    pallet_subtensor::Pallet::<T>::ensure_subnet_owner_or_root(
                        origin.clone(),
                        netuid,
                    )?;
                    if pallet_subtensor::Pallet::<T>::ensure_subnet_owner(origin.clone(), netuid)
                        .is_ok()
                    {
                        ensure!(ma <= 975_000, Error::<T>::BondsMovingAverageMaxReached);
                    }
                    ensure!(
                        pallet_subtensor::Pallet::<T>::if_subnet_exist(netuid),
                        Error::<T>::SubnetDoesNotExist
                    );
                    pallet_subtensor::Pallet::<T>::set_bonds_moving_average(netuid, ma);
                    log::debug!(
                        "BondsMovingAverageSet( netuid: {:?} ma: {:?} ) ",
                        netuid,
                        ma
                    );
                    Ok(())
                }

                HyperParam::BondsPenalty { netuid, penalty } => {
                    pallet_subtensor::Pallet::<T>::ensure_subnet_owner_or_root(origin, netuid)?;
                    ensure!(
                        pallet_subtensor::Pallet::<T>::if_subnet_exist(netuid),
                        Error::<T>::SubnetDoesNotExist
                    );
                    pallet_subtensor::Pallet::<T>::set_bonds_penalty(netuid, penalty);
                    log::debug!(
                        "BondsPenalty( netuid: {:?} bonds_penalty: {:?} ) ",
                        netuid,
                        penalty
                    );
                    Ok(())
                }

                HyperParam::MaxRegistrationsPerBlock { netuid, max } => {
                    ensure_root(origin.clone())?;
                    ensure!(
                        pallet_subtensor::Pallet::<T>::if_subnet_exist(netuid),
                        Error::<T>::SubnetDoesNotExist
                    );
                    pallet_subtensor::Pallet::<T>::set_max_registrations_per_block(netuid, max);
                    log::debug!(
                        "MaxRegistrationsPerBlock( netuid: {:?} max: {:?} ) ",
                        netuid,
                        max
                    );
                    Ok(())
                }

                HyperParam::Tempo { netuid, tempo } => {
                    ensure_root(origin.clone())?;
                    ensure!(
                        pallet_subtensor::Pallet::<T>::if_subnet_exist(netuid),
                        Error::<T>::SubnetDoesNotExist
                    );
                    pallet_subtensor::Pallet::<T>::set_tempo(netuid, tempo);
                    log::debug!("TempoSet( netuid: {:?} tempo: {:?} ) ", netuid, tempo);
                    Ok(())
                }

                HyperParam::RaoRecycled { netuid, recycled } => {
                    ensure_root(origin.clone())?;
                    ensure!(
                        pallet_subtensor::Pallet::<T>::if_subnet_exist(netuid),
                        Error::<T>::SubnetDoesNotExist
                    );
                    pallet_subtensor::Pallet::<T>::set_rao_recycled(netuid, recycled);
                    Ok(())
                }

                HyperParam::CommitRevealWeightsEnabled { netuid, enabled } => {
                    pallet_subtensor::Pallet::<T>::ensure_subnet_owner_or_root(origin, netuid)?;
                    ensure!(
                        pallet_subtensor::Pallet::<T>::if_subnet_exist(netuid),
                        Error::<T>::SubnetDoesNotExist
                    );
                    pallet_subtensor::Pallet::<T>::set_commit_reveal_weights_enabled(
                        netuid, enabled,
                    );
                    log::debug!("ToggleSetWeightsCommitReveal( netuid: {:?} ) ", netuid);
                    Ok(())
                }

                HyperParam::LiquidAlphaEnabled { netuid, enabled } => {
                    pallet_subtensor::Pallet::<T>::ensure_subnet_owner_or_root(origin, netuid)?;
                    pallet_subtensor::Pallet::<T>::set_liquid_alpha_enabled(netuid, enabled);
                    log::debug!(
                        "LiquidAlphaEnableToggled( netuid: {:?}, Enabled: {:?} ) ",
                        netuid,
                        enabled
                    );
                    Ok(())
                }

                HyperParam::AlphaValues { netuid, low, high } => {
                    pallet_subtensor::Pallet::<T>::ensure_subnet_owner_or_root(
                        origin.clone(),
                        netuid,
                    )?;
                    pallet_subtensor::Pallet::<T>::do_set_alpha_values(origin, netuid, low, high)
                }

                HyperParam::NetworkMaxStake { .. } => {
                    ensure_root(origin)?;
                    Ok(())
                }

                HyperParam::CommitRevealWeightsInterval { netuid, interval } => {
                    pallet_subtensor::Pallet::<T>::ensure_subnet_owner_or_root(origin, netuid)?;
                    ensure!(
                        pallet_subtensor::Pallet::<T>::if_subnet_exist(netuid),
                        Error::<T>::SubnetDoesNotExist
                    );
                    pallet_subtensor::Pallet::<T>::set_reveal_period(netuid, interval);
                    log::debug!(
                        "SetWeightCommitInterval( netuid: {:?}, interval: {:?} ) ",
                        netuid,
                        interval
                    );
                    Ok(())
                }

                HyperParam::ToggleTransfer { netuid, enabled } => {
                    pallet_subtensor::Pallet::<T>::ensure_subnet_owner_or_root(origin, netuid)?;
                    pallet_subtensor::Pallet::<T>::toggle_transfer(netuid, enabled)
                }

                HyperParam::SubnetOwnerHotkey { netuid, hotkey } => {
                    // **owner only**, root may NOT call this one
                    pallet_subtensor::Pallet::<T>::ensure_subnet_owner(origin.clone(), netuid)?;
                    pallet_subtensor::Pallet::<T>::set_subnet_owner_hotkey(netuid, &hotkey);
                    log::debug!(
                        "SubnetOwnerHotkeySet( netuid: {:?}, hotkey: {:?} )",
                        netuid,
                        hotkey
                    );
                    Ok(())
                }

                HyperParam::SNOwnerHotkey { netuid, hotkey } => {
                    pallet_subtensor::Pallet::<T>::do_set_sn_owner_hotkey(origin, netuid, &hotkey)
                }

                HyperParam::EMAPriceHalvingPeriod { netuid, period } => {
                    ensure_root(origin.clone())?;
                    pallet_subtensor::EMAPriceHalvingBlocks::<T>::set(netuid, period);
                    log::debug!(
                        "EMAPriceHalvingBlocks( netuid: {:?}, period: {:?} )",
                        netuid,
                        period
                    );
                    Ok(())
                }

                HyperParam::AlphaSigmoidSteepness { netuid, steepness } => {
                    ensure_root(origin.clone())?;
                    pallet_subtensor::Pallet::<T>::set_alpha_sigmoid_steepness(netuid, steepness);
                    log::debug!(
                        "AlphaSigmoidSteepnessSet( netuid: {:?}, steepness: {:?} )",
                        netuid,
                        steepness
                    );
                    Ok(())
                }

                HyperParam::Yuma3Enabled { netuid, enabled } => {
                    pallet_subtensor::Pallet::<T>::ensure_subnet_owner_or_root(origin, netuid)?;
                    pallet_subtensor::Pallet::<T>::set_yuma3_enabled(netuid, enabled);
                    Self::deposit_event(Event::Yuma3EnableToggled { netuid, enabled });
                    log::debug!(
                        "Yuma3EnableToggled( netuid: {:?}, Enabled: {:?} ) ",
                        netuid,
                        enabled
                    );
                    Ok(())
                }

                HyperParam::BondsResetEnabled { netuid, enabled } => {
                    pallet_subtensor::Pallet::<T>::ensure_subnet_owner_or_root(origin, netuid)?;
                    pallet_subtensor::Pallet::<T>::set_bonds_reset(netuid, enabled);
                    Self::deposit_event(Event::BondsResetToggled { netuid, enabled });
                    log::debug!(
                        "BondsResetToggled( netuid: {:?} bonds_reset: {:?} ) ",
                        netuid,
                        enabled
                    );
                    Ok(())
                }

                HyperParam::SubtokenEnabled { netuid, enabled } => {
                    ensure_root(origin)?;
                    pallet_subtensor::SubtokenEnabled::<T>::set(netuid, enabled);
                    log::debug!(
                        "SubtokenEnabled( netuid: {:?}, subtoken_enabled: {:?} )",
                        netuid,
                        enabled
                    );
                    Ok(())
                }
            }
        }
    }
    /// Unified payload for `sudo_set_hyperparameter`.
    ///
    /// * **Tuple-like variants** change *network-wide* parameters
    ///   (there is only a single value to set).
    /// * **Struct-like variants** target a *specific subnet* or require
    ///   multiple arguments; every field is documented for clarity.
    #[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo)]
    #[scale_info(skip_type_params(T))]
    pub enum HyperParam<T: Config> {
        /*─────────── NETWORK-WIDE ───────────*/
        /// Default delegate-take applied (in basis-points) when a delegator
        /// first bonds to a hotkey.
        DefaultTake(u16),

        /// Global, per-second extrinsic rate-limit
        /// (`tx · s⁻¹ × 10³` to preserve precision).
        TxRateLimit(u64),

        /// Chain-wide operations limit (applied per block).
        NetworkRateLimit(u64),

        /// Manually override the total TAO issuance in circulation.
        TotalIssuance(u64),

        /// Default immunity period (in blocks) given to all future subnets.
        NetworkImmunityPeriod(u64),

        /// Minimum TAO that must be locked to spin-up a subnet.
        NetworkMinLockCost(u64),

        /// Hard cap on the number of subnets that can exist at once.
        SubnetLimit(u16),

        /// Interval (in blocks) at which locked TAO is linearly released.
        LockReductionInterval(u64),

        /// Minimum self-stake a miner must hold before it can emit weights.
        StakeThreshold(u64),

        /// Minimum stake a nominator must supply to back a hotkey.
        NominatorMinRequiredStake(u64),

        /// Cool-down (in blocks) between successive `delegate_take` calls.
        TxDelegateTakeRateLimit(u64),

        /// Network-wide lower bound for delegate-take (basis-points).
        MinDelegateTake(u16),

        /// EVM chain-ID reported by the precompile layer.
        EvmChainId(u64),

        /// Share of subnet emissions reserved for the subnet owner (b.p.).
        SubnetOwnerCut(u16),

        /// Exponential moving-average α used for KPI smoothing.
        SubnetMovingAlpha(I96F32),

        /// Duration (in blocks) of the coldkey-swap unbonding schedule.
        ColdkeySwapScheduleDuration(BlockNumberFor<T>),

        /// Duration (in blocks) of the dissolve-network schedule.
        DissolveNetworkScheduleDuration(BlockNumberFor<T>),

        /*──────────── CONSENSUS ────────────*/
        /// Schedule a GRANDPA authority-set change.
        ScheduleGrandpaChange {
            /// New weighted authority list.
            next_authorities: AuthorityList,
            /// Delay (in blocks) before the change becomes active.
            in_blocks: BlockNumberFor<T>,
            /// Median last-finalised block for a *forced* replacement.
            forced: Option<BlockNumberFor<T>>,
        },

        /*──────── EVM PRECOMPILE GATE ───────*/
        /// Enable or disable a specific EVM precompile.
        ToggleEvmPrecompile {
            /// Target precompile identifier.
            precompile_id: PrecompileEnum,
            /// `true` to enable, `false` to disable.
            enabled: bool,
        },

        /*────────── SUBNET-SPECIFIC ─────────*/
        /// Set the serving-rate limit (queries · block⁻¹).
        ServingRateLimit {
            /// Subnet identifier.
            netuid: NetUid,
            /// Maximum queries allowed per block.
            limit: u64,
        },

        /// Minimum PoW difficulty accepted during registration.
        MinDifficulty {
            /// Subnet identifier.
            netuid: NetUid,
            /// New minimum difficulty.
            value: u64,
        },

        /// Maximum PoW difficulty enforced during registration.
        MaxDifficulty {
            /// Subnet identifier.
            netuid: NetUid,
            /// New maximum difficulty.
            value: u64,
        },

        /// Bump the weights-version key to invalidate all cached weights.
        WeightsVersionKey {
            /// Subnet identifier.
            netuid: NetUid,
            /// New version key.
            key: u64,
        },

        /// Cool-down (blocks) between weight-set extrinsics.
        WeightsSetRateLimit {
            /// Subnet identifier.
            netuid: NetUid,
            /// Minimum blocks between calls.
            limit: u64,
        },

        /// Epoch count between automatic difficulty adjustments.
        AdjustmentInterval {
            /// Subnet identifier.
            netuid: NetUid,
            /// Interval in epochs.
            interval: u16,
        },

        /// α parameter for exponential difficulty adjustment.
        AdjustmentAlpha {
            /// Subnet identifier.
            netuid: NetUid,
            /// Alpha value (fixed-point, scaled ×10⁶).
            alpha: u64,
        },

        /// Hard cap on non-zero weights per submission.
        MaxWeightLimit {
            /// Subnet identifier.
            netuid: NetUid,
            /// Maximum allowed weights.
            limit: u16,
        },

        /// Immunity period (epochs) before pruning can occur.
        ImmunityPeriod {
            /// Subnet identifier.
            netuid: NetUid,
            /// Period length.
            period: u16,
        },

        /// Minimum number of non-zero weights required in a submission.
        MinAllowedWeights {
            /// Subnet identifier.
            netuid: NetUid,
            /// Minimum weights.
            min: u16,
        },

        /// Maximum UID capacity of the subnet.
        MaxAllowedUids {
            /// Subnet identifier.
            netuid: NetUid,
            /// New UID cap.
            max: u16,
        },

        /// κ (kappa) parameter for incentive-decay curve.
        Kappa {
            /// Subnet identifier.
            netuid: NetUid,
            /// Kappa value.
            kappa: u16,
        },

        /// ρ (rho) parameter for incentive-decay curve.
        Rho {
            /// Subnet identifier.
            netuid: NetUid,
            /// Rho value.
            rho: u16,
        },

        /// Block-age threshold after which miners are considered inactive.
        ActivityCutoff {
            /// Subnet identifier.
            netuid: NetUid,
            /// Cut-off (blocks).
            cutoff: u16,
        },

        /// Toggle extrinsic-based (signature) registration.
        NetworkRegistrationAllowed {
            /// Subnet identifier.
            netuid: NetUid,
            /// Whether registration is allowed.
            allowed: bool,
        },

        /// Toggle PoW-based registration.
        NetworkPowRegistrationAllowed {
            /// Subnet identifier.
            netuid: NetUid,
            /// Whether registration is allowed.
            allowed: bool,
        },

        /// Target number of registrations per difficulty interval.
        TargetRegistrationsPerInterval {
            /// Subnet identifier.
            netuid: NetUid,
            /// Target registrations.
            target: u16,
        },

        /// Minimum TAO to burn during registration.
        MinBurn {
            /// Subnet identifier.
            netuid: NetUid,
            /// Minimum burn.
            min: u64,
        },

        /// Maximum TAO that can be burned.
        MaxBurn {
            /// Subnet identifier.
            netuid: NetUid,
            /// Maximum burn.
            max: u64,
        },

        /// Directly set subnet PoW difficulty.
        Difficulty {
            /// Subnet identifier.
            netuid: NetUid,
            /// New difficulty.
            value: u64,
        },

        /// Cap on validator seats in the subnet.
        MaxAllowedValidators {
            /// Subnet identifier.
            netuid: NetUid,
            /// Seat limit.
            max: u16,
        },

        /// Window size (blocks) for bonds moving-average.
        BondsMovingAverage {
            /// Subnet identifier.
            netuid: NetUid,
            /// Window length.
            ma: u64,
        },

        /// Penalty factor (%) applied to stale bonds.
        BondsPenalty {
            /// Subnet identifier.
            netuid: NetUid,
            /// Penalty percentage.
            penalty: u16,
        },

        /// Hard cap on registrations per block.
        MaxRegistrationsPerBlock {
            /// Subnet identifier.
            netuid: NetUid,
            /// Maximum per block.
            max: u16,
        },

        /// Epoch length (tempo) of the subnet, in blocks.
        Tempo {
            /// Subnet identifier.
            netuid: NetUid,
            /// Tempo (blocks).
            tempo: u16,
        },

        /// TAO recycled back into rewards pool.
        RaoRecycled {
            /// Subnet identifier.
            netuid: NetUid,
            /// Recycled amount.
            recycled: u64,
        },

        /// Enable or disable commit-reveal weights scheme.
        CommitRevealWeightsEnabled {
            /// Subnet identifier.
            netuid: NetUid,
            /// Whether the feature is enabled.
            enabled: bool,
        },

        /// Enable or disable Liquid Alpha staking.
        LiquidAlphaEnabled {
            /// Subnet identifier.
            netuid: NetUid,
            /// Whether Liquid Alpha is enabled.
            enabled: bool,
        },

        /// Lower and upper α bounds for Liquid Alpha sigmoid.
        AlphaValues {
            /// Subnet identifier.
            netuid: NetUid,
            /// Lower bound.
            low: u16,
            /// Upper bound.
            high: u16,
        },

        /// Maximum stake (RAO) a miner can bond.
        NetworkMaxStake {
            /// Subnet identifier.
            netuid: NetUid,
            /// Stake cap (RAO).
            max_stake: u64,
        },

        /// Reveal period (in blocks) for committed weights.
        CommitRevealWeightsInterval {
            /// Subnet identifier.
            netuid: NetUid,
            /// Reveal window length.
            interval: u64,
        },

        /// Enable or block α transfers between miners.
        ToggleTransfer {
            /// Subnet identifier.
            netuid: NetUid,
            /// Transfer enabled?
            enabled: bool,
        },

        /// Force-set the owner hotkey (emergency only).
        SubnetOwnerHotkey {
            /// Subnet identifier.
            netuid: NetUid,
            /// New owner hotkey account.
            hotkey: T::AccountId,
        },

        /// Same as `SubnetOwnerHotkey` but rate-limited.
        SNOwnerHotkey {
            /// Subnet identifier.
            netuid: NetUid,
            /// New owner hotkey account.
            hotkey: T::AccountId,
        },

        /// EMA price-halving period (blocks).
        EMAPriceHalvingPeriod {
            /// Subnet identifier.
            netuid: NetUid,
            /// Halving period.
            period: u64,
        },

        /// Steepness of the α-sigmoid emission curve.
        AlphaSigmoidSteepness {
            /// Subnet identifier.
            netuid: NetUid,
            /// Sigmoid steepness.
            steepness: u16,
        },

        /// Enable or disable Yuma3 staking.
        Yuma3Enabled {
            /// Subnet identifier.
            netuid: NetUid,
            /// Whether Yuma3 is enabled.
            enabled: bool,
        },

        /// Enable or disable automatic bonds reset.
        BondsResetEnabled {
            /// Subnet identifier.
            netuid: NetUid,
            /// Whether reset is enabled.
            enabled: bool,
        },

        /// Enable or disable the internal sub-token marketplace.
        SubtokenEnabled {
            /// Subnet identifier.
            netuid: NetUid,
            /// Whether trading is enabled.
            enabled: bool,
        },
    }

    /*──────────────  Stub Debug impl  ───────────────*/
    impl<T: Config> fmt::Debug for HyperParam<T> {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("HyperParam(..)")
        }
    }
}

impl<T: Config> sp_runtime::BoundToRuntimeAppPublic for Pallet<T> {
    type Public = <T as Config>::AuthorityId;
}

// Interfaces to interact with other pallets
use sp_runtime::BoundedVec;

pub trait AuraInterface<AuthorityId, MaxAuthorities> {
    fn change_authorities(new: BoundedVec<AuthorityId, MaxAuthorities>);
}

impl<A, M> AuraInterface<A, M> for () {
    fn change_authorities(_: BoundedVec<A, M>) {}
}

pub trait GrandpaInterface<Runtime>
where
    Runtime: frame_system::Config,
{
    fn schedule_change(
        next_authorities: AuthorityList,
        in_blocks: BlockNumberFor<Runtime>,
        forced: Option<BlockNumberFor<Runtime>>,
    ) -> DispatchResult;
}

impl<R> GrandpaInterface<R> for ()
where
    R: frame_system::Config,
{
    fn schedule_change(
        _next_authorities: AuthorityList,
        _in_blocks: BlockNumberFor<R>,
        _forced: Option<BlockNumberFor<R>>,
    ) -> DispatchResult {
        Ok(())
    }
}
