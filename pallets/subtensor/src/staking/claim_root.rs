use super::*;
use frame_support::weights::Weight;
use sp_core::Get;
use substrate_fixed::types::{I110F18, I96F32};

impl<T: Config> Pallet<T> {
    pub fn block_hash_to_indices(block_hash: T::Hash, k: u64, n: u64) -> Vec<u64> {
        let block_hash_bytes = block_hash.as_ref();
        let mut indices: Vec<u64> = Vec::new();
        // k < n
        let start_index: u64 = u64::from_be_bytes(
            block_hash_bytes
                .get(0..8)
                .unwrap_or(&[0; 8])
                .try_into()
                .unwrap_or([0; 8]),
        );
        let mut last_idx = start_index;
        for i in 0..k {
            let bh_idx: usize = ((i.saturating_mul(8)) % 32) as usize;
            let idx_step = u64::from_be_bytes(
                block_hash_bytes
                    .get(bh_idx..(bh_idx.saturating_add(8)))
                    .unwrap_or(&[0; 8])
                    .try_into()
                    .unwrap_or([0; 8]),
            );
            let idx = last_idx
                .saturating_add(idx_step)
                .checked_rem(n)
                .unwrap_or(0);
            indices.push(idx);
            last_idx = idx;
        }
        indices
    }

    pub fn increase_root_claimable_for_hotkey_and_subnet(
        hotkey: &T::AccountId,
        netuid: u16,
        amount: u64,
    ) {
        // Get total stake on this hotkey on root.
        let total: I96F32 =
            I96F32::saturating_from_num(Self::get_stake_for_hotkey_on_subnet(hotkey, netuid));

        // Get increment
        let increment: I96F32 = I96F32::saturating_from_num(amount)
            .checked_div(total)
            .unwrap_or(I96F32::saturating_from_num(0.0));

        // Convert increment to u64, mapping negative values to 0
        let increment_u64: u64 = if increment.is_negative() {
            0
        } else {
            increment.saturating_to_num::<u64>()
        };

        // Increment claimable for this subnet.
        RootClaimable::<T>::mutate(hotkey, netuid, |total| {
            *total = total.saturating_add(increment_u64);
        });
    }

    pub fn get_root_claimable_for_hotkey_coldkey(
        hotkey: &T::AccountId,
        coldkey: &T::AccountId,
        netuid: u16,
    ) -> I110F18 {
        // Get this keys stake balance on root.
        let root_stake: I110F18 =
            I110F18::saturating_from_num(Self::get_stake_for_hotkey_and_coldkey_on_subnet(
                hotkey,
                coldkey,
                Self::get_root_netuid(),
            ));

        // Get the total claimable_rate for this hotkey and this network
        let claimable_rate: I110F18 =
            I110F18::saturating_from_num(RootClaimable::<T>::get(hotkey, netuid));

        // Compute the proportion owed to this coldkey via balance.
        let claimable: I110F18 = claimable_rate.saturating_mul(root_stake);

        claimable
    }

    pub fn get_root_owed_for_hotkey_coldkey_float(
        hotkey: &T::AccountId,
        coldkey: &T::AccountId,
        netuid: u16,
    ) -> I110F18 {
        let claimable = Self::get_root_claimable_for_hotkey_coldkey(hotkey, coldkey, netuid);

        // Attain the claimable debt to avoid overclaiming.
        let debt: I110F18 =
            I110F18::saturating_from_num(RootDebt::<T>::get((hotkey, coldkey, netuid)));

        // Substract the debt.
        let owed: I110F18 = claimable.saturating_sub(debt);

        owed
    }

    pub fn get_root_owed_for_hotkey_coldkey(
        hotkey: &T::AccountId,
        coldkey: &T::AccountId,
        netuid: u16,
    ) -> u64 {
        let owed = Self::get_root_owed_for_hotkey_coldkey_float(hotkey, coldkey, netuid);

        // Convert owed to u64, mapping negative values to 0
        let owed_u64: u64 = if owed.is_negative() {
            0
        } else {
            owed.saturating_to_num::<u64>()
        };

        owed_u64
    }

    pub fn root_claim_on_subnet(
        hotkey: &T::AccountId,
        coldkey: &T::AccountId,
        netuid: u16,
        root_claim_type: RootClaimTypeEnum,
    ) {
        // Substract the debt.
        let owed: I110F18 = Self::get_root_owed_for_hotkey_coldkey_float(hotkey, coldkey, netuid);

        if owed == 0 || owed < I110F18::saturating_from_num(DefaultMinRootClaimAmount::<T>::get()) {
            return; // no-op
        }

        // Increase root debt by owed amount.
        RootDebt::<T>::mutate((hotkey, coldkey, netuid), |debt| {
            *debt = debt.saturating_add(owed.saturating_to_num::<I96F32>());
        });

        // Convert owed to u64, mapping negative values to 0
        let owed_u64: u64 = if owed.is_negative() {
            0
        } else {
            owed.saturating_to_num::<u64>()
        };

        if owed_u64 == 0 {
            return; // no-op
        }

        match root_claim_type {
            RootClaimTypeEnum::Swap => {
                // Swap the alpha owed to TAO and then increase stake on root
                let owed_tao: u64 = Self::swap_alpha_for_tao(netuid, owed_u64);

                Self::increase_stake_for_hotkey_and_coldkey_on_subnet(
                    hotkey,
                    coldkey,
                    Self::get_root_netuid(),
                    owed_tao,
                );
            }
            RootClaimTypeEnum::Keep => {
                // Incerase the stake with the alpha owned
                Self::increase_stake_for_hotkey_and_coldkey_on_subnet(
                    hotkey, coldkey, netuid, owed_u64,
                );
            }
        };
    }

    pub fn root_claim_all(hotkey: &T::AccountId, coldkey: &T::AccountId) -> Weight {
        let mut weight = Weight::default();

        weight.saturating_accrue(T::DbWeight::get().reads(1));
        let root_claim_type = RootClaimType::<T>::get(coldkey);

        // Iterate over all the subnets this hotkey has claimable for root.
        RootClaimable::<T>::iter_prefix(hotkey).for_each(|(netuid, _)| {
            weight.saturating_accrue(T::DbWeight::get().reads(1));
            weight.saturating_accrue(<T as Config>::WeightInfo::root_claim_on_subnet(
                root_claim_type.clone(),
            ));

            Self::root_claim_on_subnet(hotkey, coldkey, netuid, root_claim_type.clone());
        });

        weight
    }

    pub fn add_stake_adjust_debt_for_hotkey_and_coldkey(
        hotkey: &T::AccountId,
        coldkey: &T::AccountId,
        amount: u64,
    ) {
        // Add to StakingColdkeys if not already present
        if !StakingColdkeys::<T>::contains_key(coldkey) {
            StakingColdkeys::<T>::insert(coldkey, Vec::new());
            NumStakingColdkeys::<T>::mutate(|n| {
                // Increment the number of coldkeys
                *n = n.saturating_add(1);
            });
        }

        // Iterate over all the subnets this hotkey is staked on for root.
        for (netuid, claimable_rate) in RootClaimable::<T>::iter_prefix(hotkey) {
            // Get the total claimable_rate for this hotkey and this network
            let claimable_rate_float = I110F18::saturating_from_num(claimable_rate);

            // Get current staker-debt.
            let debt: I110F18 =
                I110F18::saturating_from_num(RootDebt::<T>::get((hotkey, coldkey, netuid)));

            // Increase debt based on the claimable rate.
            let new_debt: I110F18 = debt.saturating_add(
                claimable_rate_float.saturating_mul(I110F18::saturating_from_num(amount)),
            );

            // Set the new debt.
            RootDebt::<T>::insert(
                (hotkey, coldkey, netuid),
                new_debt.saturating_to_num::<I96F32>(),
            );
        }
    }

    pub fn remove_stake_adjust_debt_for_hotkey_and_coldkey(
        hotkey: &T::AccountId,
        coldkey: &T::AccountId,
        amount: u64,
    ) {
        // Iterate over all the subnets this hotkey is staked on for root.
        for (netuid, claimable_rate) in RootClaimable::<T>::iter_prefix(hotkey) {
            if netuid == Self::get_root_netuid() {
                continue; // Skip the root netuid.
            }

            // Get the total claimable_rate for this hotkey and this network
            let claimable_rate_float = I110F18::saturating_from_num(claimable_rate);

            // Get current staker-debt.
            let debt: I110F18 =
                I110F18::saturating_from_num(RootDebt::<T>::get((hotkey, coldkey, netuid)));

            // Decrease debt based on the claimable rate.
            let new_debt: I110F18 = debt.saturating_sub(
                claimable_rate_float.saturating_mul(I110F18::saturating_from_num(amount)),
            );

            // Set the new debt.
            RootDebt::<T>::insert(
                (hotkey, coldkey, netuid),
                new_debt.saturating_to_num::<I96F32>(),
            );
        }
    }

    pub fn do_root_claim(coldkey: T::AccountId) -> Weight {
        let mut weight = Weight::default();

        let hotkeys = StakingHotkeys::<T>::get(&coldkey);
        weight.saturating_accrue(T::DbWeight::get().reads(1));

        hotkeys.iter().for_each(|hotkey| {
            weight.saturating_accrue(T::DbWeight::get().reads(1));
            weight.saturating_accrue(Self::root_claim_all(hotkey, &coldkey));
        });

        Self::deposit_event(Event::RootClaimed(coldkey));

        weight
    }

    pub fn run_auto_claim_root_divs(last_block_hash: T::Hash) -> Weight {
        let mut weight: Weight = Weight::default();

        let n = NumStakingColdkeys::<T>::get();
        let k = NumRootClaim::<T>::get();
        weight.saturating_accrue(T::DbWeight::get().reads(2));

        let coldkeys_to_claim: Vec<u64> = Self::block_hash_to_indices(last_block_hash, k, n);
        weight.saturating_accrue(<T as Config>::WeightInfo::block_hash_to_indices(k, n));

        for i in coldkeys_to_claim.iter() {
            weight.saturating_accrue(T::DbWeight::get().reads(1));
            if let Ok(coldkey) = StakingColdkeys::<T>::try_get(i) {
                weight.saturating_accrue(Self::do_root_claim(coldkey.clone()));
            }

            continue;
        }

        weight
    }

    pub fn change_root_claim_type(coldkey: &T::AccountId, new_type: RootClaimTypeEnum) {
        RootClaimType::<T>::insert(coldkey.clone(), new_type.clone());

        Self::deposit_event(Event::RootClaimTypeSet(coldkey.clone(), new_type));
    }
}
