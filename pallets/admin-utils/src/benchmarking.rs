//! Benchmarking setup
#![cfg(feature = "runtime-benchmarks")]
#![allow(clippy::arithmetic_side_effects)]

extern crate alloc;

#[allow(unused)]
use crate::Pallet as AdminUtils;
use frame_benchmarking::v2::*;
use frame_system::RawOrigin;

use super::*;

#[benchmarks]
mod benchmarks {
    use super::*;

    #[benchmark]
    fn sudo_set_default_take() {
        #[extrinsic_call]
		_(RawOrigin::Root, 100u16/*default_take*/)/*sudo_set_default_take*/;
    }

    #[benchmark]
    fn sudo_set_serving_rate_limit() {
        #[extrinsic_call]
		_(RawOrigin::Root, 1u16.into()/*netuid*/, 100u64/*serving_rate_limit*/)/*sudo_set_serving_rate_limit*/;
    }

    #[benchmark]
    fn sudo_set_max_difficulty() {
        pallet_subtensor::Pallet::<T>::init_new_network(
            1u16.into(), /*netuid*/
            1u16,        /*tempo*/
        );

        #[extrinsic_call]
		_(RawOrigin::Root, 1u16.into()/*netuid*/, 10000u64/*max_difficulty*/)/*sudo_set_max_difficulty*/;
    }

    #[benchmark]
    fn sudo_set_min_difficulty() {
        pallet_subtensor::Pallet::<T>::init_new_network(
            1u16.into(), /*netuid*/
            1u16,        /*tempo*/
        );

        #[extrinsic_call]
		_(RawOrigin::Root, 1u16.into()/*netuid*/, 1000u64/*min_difficulty*/)/*sudo_set_min_difficulty*/;
    }

    #[benchmark]
    fn sudo_set_weights_set_rate_limit() {
        pallet_subtensor::Pallet::<T>::init_new_network(
            1u16.into(), /*netuid*/
            1u16,        /*tempo*/
        );

        #[extrinsic_call]
		_(RawOrigin::Root, 1u16.into()/*netuid*/, 3u64/*rate_limit*/)/*sudo_set_weights_set_rate_limit*/;
    }

    #[benchmark]
    fn sudo_set_weights_version_key() {
        pallet_subtensor::Pallet::<T>::init_new_network(
            1u16.into(), /*netuid*/
            1u16,        /*tempo*/
        );

        #[extrinsic_call]
		_(RawOrigin::Root, 1u16.into()/*netuid*/, 1u64/*version_key*/)/*sudo_set_weights_version_key*/;
    }

    #[benchmark]
    fn sudo_set_bonds_moving_average() {
        pallet_subtensor::Pallet::<T>::init_new_network(
            1u16.into(), /*netuid*/
            1u16,        /*tempo*/
        );

        #[extrinsic_call]
		_(RawOrigin::Root, 1u16.into()/*netuid*/, 100u64/*bonds_moving_average*/)/*sudo_set_bonds_moving_average*/;
    }

    #[benchmark]
    fn sudo_set_bonds_penalty() {
        pallet_subtensor::Pallet::<T>::init_new_network(
            1u16.into(), /*netuid*/
            1u16,        /*tempo*/
        );

        #[extrinsic_call]
        _(RawOrigin::Root, 1u16.into()/*netuid*/, 100u16/*bonds_penalty*/)/*sudo_set_bonds_penalty*/;
    }

    #[benchmark]
    fn sudo_set_max_allowed_validators() {
        pallet_subtensor::Pallet::<T>::init_new_network(
            1u16.into(), /*netuid*/
            1u16,        /*tempo*/
        );

        #[extrinsic_call]
		_(RawOrigin::Root, 1u16.into()/*netuid*/, 10u16/*max_allowed_validators*/)/*sudo_set_max_allowed_validators*/;
    }

    #[benchmark]
    fn sudo_set_difficulty() {
        pallet_subtensor::Pallet::<T>::init_new_network(
            1u16.into(), /*netuid*/
            1u16,        /*tempo*/
        );

        #[extrinsic_call]
		_(RawOrigin::Root, 1u16.into()/*netuid*/, 1200000u64/*difficulty*/)/*sudo_set_difficulty*/;
    }

    #[benchmark]
    fn sudo_set_adjustment_interval() {
        pallet_subtensor::Pallet::<T>::init_new_network(
            1u16.into(), /*netuid*/
            1u16,        /*tempo*/
        );

        #[extrinsic_call]
		_(RawOrigin::Root, 1u16.into()/*netuid*/, 12u16/*adjustment_interval*/)/*sudo_set_adjustment_interval*/;
    }

    #[benchmark]
    fn sudo_set_target_registrations_per_interval() {
        pallet_subtensor::Pallet::<T>::init_new_network(
            1u16.into(), /*netuid*/
            1u16,        /*tempo*/
        );

        #[extrinsic_call]
		_(RawOrigin::Root, 1u16.into()/*netuid*/, 300u16/*target_registrations*/)/*sudo_set_target_registrations_per_interval*/;
    }

    #[benchmark]
    fn sudo_set_activity_cutoff() {
        pallet_subtensor::Pallet::<T>::init_new_network(
            1u16.into(), /*netuid*/
            1u16,        /*tempo*/
        );

        #[extrinsic_call]
		_(RawOrigin::Root, 1u16.into()/*netuid*/, 361u16/*activity_cutoff*/)/*sudo_set_activity_cutoff*/;
    }

    #[benchmark]
    fn sudo_set_rho() {
        pallet_subtensor::Pallet::<T>::init_new_network(
            1u16.into(), /*netuid*/
            1u16,        /*tempo*/
        );

        #[extrinsic_call]
		_(RawOrigin::Root, 1u16.into()/*netuid*/, 300u16/*rho*/)/*sudo_set_rho*/;
    }

    #[benchmark]
    fn sudo_set_kappa() {
        pallet_subtensor::Pallet::<T>::init_new_network(
            1u16.into(), /*netuid*/
            1u16,        /*sudo_tempo*/
        );

        #[extrinsic_call]
		_(RawOrigin::Root, 1u16.into()/*netuid*/, 3u16/*kappa*/)/*set_kappa*/;
    }

    #[benchmark]
    fn sudo_set_max_allowed_uids() {
        pallet_subtensor::Pallet::<T>::init_new_network(
            1u16.into(), /*netuid*/
            1u16,        /*tempo*/
        );

        #[extrinsic_call]
		_(RawOrigin::Root, 1u16.into()/*netuid*/, 4097u16/*max_allowed_uids*/)/*sudo_set_max_allowed_uids*/;
    }

    #[benchmark]
    fn sudo_set_min_allowed_weights() {
        pallet_subtensor::Pallet::<T>::init_new_network(
            1u16.into(), /*netuid*/
            1u16,        /*tempo*/
        );

        #[extrinsic_call]
		_(RawOrigin::Root, 1u16.into()/*netuid*/, 10u16/*max_allowed_uids*/)/*sudo_set_min_allowed_weights*/;
    }

    #[benchmark]
    fn sudo_set_immunity_period() {
        pallet_subtensor::Pallet::<T>::init_new_network(
            1u16.into(), /*netuid*/
            1u16,        /*tempo*/
        );

        #[extrinsic_call]
		_(RawOrigin::Root, 1u16.into()/*netuid*/, 100u16/*immunity_period*/)/*sudo_set_immunity_period*/;
    }

    #[benchmark]
    fn sudo_set_max_weight_limit() {
        pallet_subtensor::Pallet::<T>::init_new_network(
            1u16.into(), /*netuid*/
            1u16,        /*tempo*/
        );

        #[extrinsic_call]
		_(RawOrigin::Root, 1u16.into()/*netuid*/, 100u16/*max_weight_limit*/)/*sudo_set_max_weight_limit*/;
    }

    #[benchmark]
    fn sudo_set_max_registrations_per_block() {
        pallet_subtensor::Pallet::<T>::init_new_network(
            1u16.into(), /*netuid*/
            1u16,        /*tempo*/
        );

        #[extrinsic_call]
		_(RawOrigin::Root, 1u16.into()/*netuid*/, 100u16/*max_registrations*/)/*sudo_set_max_registrations_per_block*/;
    }

    #[benchmark]
    fn sudo_set_max_burn() {
        pallet_subtensor::Pallet::<T>::init_new_network(
            1u16.into(), /*netuid*/
            1u16,        /*tempo*/
        );

        #[extrinsic_call]
		_(RawOrigin::Root, 1u16.into()/*netuid*/, 10.into()/*max_burn*/)/*sudo_set_max_burn*/;
    }

    #[benchmark]
    fn sudo_set_min_burn() {
        pallet_subtensor::Pallet::<T>::init_new_network(
            1u16.into(), /*netuid*/
            1u16,        /*tempo*/
        );

        #[extrinsic_call]
		_(RawOrigin::Root, 1u16.into()/*netuid*/, 10.into()/*min_burn*/)/*sudo_set_min_burn*/;
    }

    #[benchmark]
    fn sudo_set_network_registration_allowed() {
        pallet_subtensor::Pallet::<T>::init_new_network(
            1u16.into(), /*netuid*/
            1u16,        /*tempo*/
        );

        #[extrinsic_call]
		_(RawOrigin::Root, 1u16.into()/*netuid*/, true/*registration_allowed*/)/*sudo_set_network_registration_allowed*/;
    }

    /*
        benchmark_sudo_set_tempo {
        let netuid = NetUid::from(1);
        let tempo_default: u16 = 1; <------- unused?
        let tempo: u16 = 15;
        let modality: u16 = 0;

        pallet_subtensor::Pallet::<T>::init_new_network(netuid, tempo);

    }: sudo_set_tempo(RawOrigin::<AccountIdOf<T>>::Root, netuid, tempo)
    */
    #[benchmark]
    fn sudo_set_tempo() {
        pallet_subtensor::Pallet::<T>::init_new_network(
            1u16.into(), /*netuid*/
            1u16,        /*tempo*/
        );

        #[extrinsic_call]
		_(RawOrigin::Root, 1u16.into()/*netuid*/, 1u16/*tempo*/)/*sudo_set_tempo*/;
    }

    #[benchmark]
    fn sudo_set_commit_reveal_weights_interval() {
        pallet_subtensor::Pallet::<T>::init_new_network(
            1u16.into(), /*netuid*/
            1u16,        /*sudo_tempo*/
        );

        #[extrinsic_call]
		_(RawOrigin::Root, 1u16.into()/*netuid*/, 3u64/*interval*/)/*sudo_set_commit_reveal_weights_interval()*/;
    }

    #[benchmark]
    fn sudo_set_commit_reveal_weights_enabled() {
        pallet_subtensor::Pallet::<T>::init_new_network(
            1u16.into(), /*netuid*/
            1u16,        /*sudo_tempo*/
        );

        #[extrinsic_call]
		_(RawOrigin::Root, 1u16.into()/*netuid*/, true/*enabled*/)/*set_commit_reveal_weights_enabled*/;
    }

    #[benchmark]
    fn sudo_set_commit_reveal_version() {
        pallet_subtensor::Pallet::<T>::init_new_network(
            1u16.into(), /*netuid*/
            1u16,        /*sudo_tempo*/
        );

        #[extrinsic_call]
		_(RawOrigin::Root, 5u16/*version*/)/*sudo_set_commit_reveal_version()*/;
    }

    //impl_benchmark_test_suite!(AdminUtils, crate::mock::new_test_ext(), crate::mock::Test);
}
