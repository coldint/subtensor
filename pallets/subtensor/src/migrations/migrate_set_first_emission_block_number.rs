use super::*;
use crate::HasMigrationRun;
use frame_support::{traits::Get, weights::Weight};
use scale_info::prelude::string::String;

pub fn migrate_set_first_emission_block_number<T: Config>() -> Weight {
    let migration_name = b"migrate_set_first_emission_block_number".to_vec();

    let mut weight = <T as frame_system::Config>::DbWeight::get().reads(1);
    if HasMigrationRun::<T>::get(&migration_name) {
        log::info!(
            "Migration '{:?}' has already run. Skipping.",
            String::from_utf8_lossy(&migration_name)
        );
        return weight;
    }

    log::info!(
        "Running migration '{:?}'",
        String::from_utf8_lossy(&migration_name)
    );

    // ------------------------------
    // Step 1: Set the first emission block for all subnets except root
    // ------------------------------
    let netuids = Pallet::<T>::get_all_subnet_netuids();
    let current_block_number = Pallet::<T>::get_current_block_as_u64();
    for netuid in netuids.iter() {
        if *netuid != 0 {
            FirstEmissionBlockNumber::<T>::insert(netuid, current_block_number);
        }
    }

    // ------------------------------
    // Step 2: Mark Migration as Completed
    // ------------------------------

    HasMigrationRun::<T>::insert(&migration_name, true);
    weight = weight.saturating_add(<T as frame_system::Config>::DbWeight::get().reads(2));

    if netuids.is_empty() {
        weight = weight.saturating_add(<T as frame_system::Config>::DbWeight::get().writes(1_u64));
    } else {
        weight = weight.saturating_add(<T as frame_system::Config>::DbWeight::get().writes(netuids.len() as u64));
    }

    log::info!(
        "Migration '{:?}' completed successfully.",
        String::from_utf8_lossy(&migration_name)
    );

    weight
}
