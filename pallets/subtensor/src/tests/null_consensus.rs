#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use super::mock::*;
use crate::*;
use frame_support::assert_ok;
use sp_core::U256;
use subtensor_runtime_common::{MechId, NetUid, NetUidStorageIndex};

fn setup(stakes: [u64; 2]) -> NetUid {
    let netuid = NetUid::from(1);
    add_network_disable_commit_reveal(netuid, u16::MAX - 1, 0);
    SubtensorModule::set_max_allowed_uids(netuid, 4);
    SubtensorModule::set_max_allowed_validators(netuid, 2);
    SubtensorModule::set_weights_set_rate_limit(netuid, 0);
    for uid in 0..4u16 {
        let hotkey = U256::from(uid);
        SubtensorModule::append_neuron(netuid, &hotkey, 0);
        if let Some(stake) = stakes.get(uid as usize) {
            SubtensorModule::increase_stake_for_hotkey_and_coldkey_on_subnet(
                &hotkey,
                &hotkey,
                netuid,
                (*stake).into(),
            );
            SubtensorModule::set_validator_permit_for_uid(netuid, uid, true);
        }
    }
    System::set_block_number(1);
    SubtensorModule::set_epoch_consensus(netuid, EpochConsensus::Null);
    netuid
}

fn set_weights(netuid: NetUid, uid: u16, destinations: Vec<u16>, weights: Vec<u16>) {
    assert_ok!(SubtensorModule::set_weights(
        RuntimeOrigin::signed(U256::from(uid)),
        netuid,
        destinations,
        weights,
        0,
    ));
}

#[test]
fn null_consensus_winner_incentives_and_stake_dividends_ignore_activity() {
    new_test_ext(1).execute_with(|| {
        let netuid = setup([300_000_000, 100_000_000]);
        set_weights(netuid, 0, vec![2, 3], vec![3, 1]);
        set_weights(netuid, 1, vec![3], vec![u16::MAX]);
        // Both validators become inactive; Null must still use their stake.
        System::set_block_number(
            SubtensorModule::get_activity_cutoff_blocks(netuid).saturating_add(2),
        );
        let output = SubtensorModule::epoch_mechanism(netuid, MechId::MAIN, 1_000_000.into());
        let terms = output.as_map();
        let winner = &terms[&U256::from(0)];
        let other = &terms[&U256::from(1)];
        assert!(!winner.active);
        assert!(!other.active);
        assert!(winner.validator_emission.to_u64().abs_diff(375_000) <= 5);
        assert!(other.validator_emission.to_u64().abs_diff(125_000) <= 5);
        assert!(
            terms[&U256::from(2)]
                .server_emission
                .to_u64()
                .abs_diff(375_000)
                <= 5
        );
        assert!(
            terms[&U256::from(3)]
                .server_emission
                .to_u64()
                .abs_diff(125_000)
                <= 5
        );
        assert_eq!(winner.server_emission.to_u64(), 0);
        assert_eq!(other.server_emission.to_u64(), 0);
    });
}

#[test]
fn null_consensus_tie_selects_first_uid() {
    new_test_ext(1).execute_with(|| {
        let netuid = setup([100_000_000, 100_000_000]);
        set_weights(netuid, 0, vec![2], vec![u16::MAX]);
        set_weights(netuid, 1, vec![3], vec![u16::MAX]);
        let output = SubtensorModule::epoch_mechanism(netuid, MechId::MAIN, 1_000_000.into());
        assert!(
            output.as_map()[&U256::from(2)]
                .server_emission
                .to_u64()
                .abs_diff(500_000)
                <= 1
        );
        assert_eq!(output.as_map()[&U256::from(3)].server_emission.to_u64(), 0);
    });
}

#[test]
fn null_consensus_empty_winner_weights_share_across_every_uid() {
    new_test_ext(1).execute_with(|| {
        let netuid = setup([300_000_000, 100_000_000]);
        set_weights(netuid, 1, vec![3], vec![u16::MAX]);
        // The loser's row must not substitute for the empty winner row.
        let output = SubtensorModule::epoch_mechanism(netuid, MechId::MAIN, 1_000_000.into());
        for terms in output.as_map().values() {
            assert!(terms.server_emission.to_u64().abs_diff(125_000) <= 1);
        }
        assert!(
            output.as_map()[&U256::from(0)]
                .validator_emission
                .to_u64()
                .abs_diff(375_000)
                <= 1
        );
        assert!(
            output.as_map()[&U256::from(1)]
                .validator_emission
                .to_u64()
                .abs_diff(125_000)
                <= 1
        );
    });
}

#[test]
fn null_consensus_selects_winner_before_normalized_stake_rounding() {
    new_test_ext(1).execute_with(|| {
        let netuid = setup([1_000_000_000, 1_000_000_001]);
        set_weights(netuid, 0, vec![2], vec![u16::MAX]);
        set_weights(netuid, 1, vec![3], vec![u16::MAX]);
        let output = SubtensorModule::epoch_mechanism(netuid, MechId::MAIN, 1_000_000.into());
        assert_eq!(output.as_map()[&U256::from(2)].server_emission.to_u64(), 0);
        assert!(
            output.as_map()[&U256::from(3)]
                .server_emission
                .to_u64()
                .abs_diff(500_000)
                <= 1
        );
    });
}

#[test]
fn null_consensus_stale_winner_row_uses_equal_fallback() {
    new_test_ext(1).execute_with(|| {
        let netuid = setup([300_000_000, 100_000_000]);
        set_weights(netuid, 0, vec![2], vec![u16::MAX]);
        set_weights(netuid, 1, vec![3], vec![u16::MAX]);
        // UID 2 was replaced after the winner submitted its weights.
        BlockAtRegistration::<Test>::insert(netuid, 2, 2u64);
        let output = SubtensorModule::epoch_mechanism(netuid, MechId::MAIN, 1_000_000.into());
        for terms in output.as_map().values() {
            assert!(terms.server_emission.to_u64().abs_diff(125_000) <= 1);
        }
    });
}

#[test]
fn null_consensus_preserves_bonds_and_can_switch_back_to_yuma() {
    new_test_ext(1).execute_with(|| {
        let netuid = setup([300_000_000, 100_000_000]);
        let index = SubtensorModule::get_mechanism_storage_index(netuid, MechId::MAIN);
        Bonds::<Test>::insert(index, 0, vec![(2u16, 12345u16)]);
        Bonds::<Test>::insert(index, 1, vec![(3u16, 54321u16)]);
        let before = (Bonds::<Test>::get(index, 0), Bonds::<Test>::get(index, 1));
        SubtensorModule::epoch(netuid, 1_000_000.into());
        assert_eq!(
            before,
            (Bonds::<Test>::get(index, 0), Bonds::<Test>::get(index, 1))
        );
        SubtensorModule::epoch_dense(netuid, 1_000_000.into());
        assert_eq!(
            before,
            (Bonds::<Test>::get(index, 0), Bonds::<Test>::get(index, 1))
        );
        SubtensorModule::set_epoch_consensus(netuid, EpochConsensus::Yuma);
        assert_eq!(
            SubtensorModule::get_epoch_consensus(netuid),
            EpochConsensus::Yuma
        );
        SubtensorModule::epoch(netuid, 1_000_000.into());
    });
}

#[test]
fn null_consensus_no_stake_has_only_equal_miner_emissions() {
    new_test_ext(1).execute_with(|| {
        let netuid = setup([0, 0]);
        let output = SubtensorModule::epoch_mechanism(netuid, MechId::MAIN, 1_000_000.into());
        for terms in output.as_map().values() {
            assert_eq!(terms.server_emission.to_u64(), 250_000);
            assert_eq!(terms.validator_emission.to_u64(), 0);
        }
    });
}

#[test]
fn null_consensus_default_and_empty_subnet() {
    new_test_ext(1).execute_with(|| {
        let netuid = NetUid::from(1);
        add_network_disable_commit_reveal(netuid, u16::MAX - 1, 0);
        assert_eq!(
            SubtensorModule::get_epoch_consensus(netuid),
            EpochConsensus::Yuma
        );
        SubtensorModule::set_epoch_consensus(netuid, EpochConsensus::Null);
        assert!(
            SubtensorModule::epoch_mechanism(netuid, MechId::MAIN, 1_000_000.into())
                .as_map()
                .is_empty()
        );
    });
}

#[test]
fn null_apportionment_conserves_large_and_subatomic_budgets() {
    use crate::epoch::run_epoch::apportion_units;
    for budget in [0, 1, 7, 16_001, 1_000_003, u64::MAX] {
        let mut shares = vec![u128::from(u16::MAX); 16_000];
        shares[0] = 1;
        shares[100] = 0;
        let payouts = apportion_units(&shares, budget);
        let sum: u128 = payouts.iter().map(|p| u128::from(u64::from(*p))).sum();
        assert_eq!(sum, u128::from(budget));
        assert_eq!(u64::from(payouts[100]), 0);
        assert_eq!(payouts, apportion_units(&shares, budget));
        let total: u128 = shares.iter().sum();
        for (share, paid) in shares.iter().zip(&payouts) {
            let floor = u128::from(budget) * share / total;
            assert!([floor, floor + 1].contains(&u128::from(u64::from(*paid))));
        }
    }
    assert_eq!(
        apportion_units(&[u128::MAX; 16_000], u64::MAX)
            .iter()
            .map(|p| u128::from(u64::from(*p)))
            .sum::<u128>(),
        u128::from(u64::MAX)
    );
}

#[test]
fn null_consensus_preserves_raw_weights_and_epoch_budget() {
    new_test_ext(1).execute_with(|| {
        let netuid = setup([300_000_000, 100_000_000]);
        set_weights(netuid, 0, vec![2, 3], vec![65_534, 1]);
        assert_eq!(
            Weights::<Test>::get(NetUidStorageIndex::from(netuid), 0),
            vec![(2, 65_534), (3, 1)]
        );
        for budget in [0, 1, 3, 131_071, u64::MAX] {
            let output = SubtensorModule::epoch_mechanism(netuid, MechId::MAIN, budget.into());
            let miner_sum: u128 = output
                .as_map()
                .values()
                .map(|t| u128::from(u64::from(t.server_emission)))
                .sum();
            let validator_sum: u128 = output
                .as_map()
                .values()
                .map(|t| u128::from(u64::from(t.validator_emission)))
                .sum();
            assert_eq!(miner_sum, u128::from(budget - budget / 2));
            assert_eq!(validator_sum, u128::from(budget / 2));
            assert_eq!(miner_sum + validator_sum, u128::from(budget));
            for term in output.as_map().values() {
                assert_eq!(
                    term.emission,
                    term.server_emission.saturating_add(term.validator_emission)
                );
            }
        }
    });
}

#[test]
fn null_consensus_full_row_timelock_reveal_to_emissions() {
    use crate::coinbase::reveal_commits::WeightsTlockPayload;
    use ark_serialize::{CanonicalDeserialize, CanonicalSerialize};
    use codec::Encode;
    use pallet_drand::types::Pulse;
    use rand_chacha::{ChaCha20Rng, rand_core::SeedableRng};
    use sha2::Digest;
    use tle::{
        curves::drand::TinyBLS381, ibe::fullident::Identity,
        stream_ciphers::AESGCMStreamCipherProvider, tlock::tle,
    };
    use w3f_bls::EngineBLS;

    new_test_ext(1).execute_with(|| {
        let netuid = setup([300_000_000, 100_000_000]);
        let index = NetUidStorageIndex::from(netuid);
        // Populate the registered-UID fixtures without 16,000 independent
        // extrinsic transactions. Submission and reveal use production paths.
        SubtensorModule::set_max_allowed_uids(netuid, 16_000);
        for uid in 4..16_000u16 {
            Keys::<Test>::insert(netuid, uid, U256::from(uid));
            Uids::<Test>::insert(netuid, U256::from(uid), uid);
        }
        SubnetworkN::<Test>::insert(netuid, 16_000);
        let mut permits = vec![false; 16_000]; permits[0] = true; permits[1] = true;
        ValidatorPermit::<Test>::insert(netuid, permits);
        LastUpdate::<Test>::insert(index, vec![0u64; 16_000]);
        SubtensorModule::set_commit_reveal_weights_enabled(netuid, true);
        assert_ok!(SubtensorModule::set_reveal_period(netuid, 1));
        let round = 1_000u64;
        let mut values = vec![1u16; 16_000]; values[2] = 65_534;
        let payload = WeightsTlockPayload { hotkey: U256::from(0).encode(),
            uids: (0..16_000u16).collect(), values: values.clone(), version_key: 0 };
        let pk_bytes = hex::decode("83cf0f2896adee7eb8b5f01fcad3912212c437e0073e911fb90022d3e760183c8c4b450b6a0a6c3ac6a5776a2d1064510d1fec758c921cc22b0e17e63aaf4bcb5ed66304de9cf809bd274ca73bab4af5a6e9c76a4bc09e76eae8991ef5ece45a").unwrap();
        let pk = <TinyBLS381 as EngineBLS>::PublicKeyGroup::deserialize_compressed(&*pk_bytes).unwrap();
        let identity = Identity::new(b"", vec![sha2::Sha256::digest(round.to_be_bytes()).to_vec()]);
        let cipher = tle::<TinyBLS381, AESGCMStreamCipherProvider, ChaCha20Rng>(
            pk, [2; 32], &payload.encode(), identity, ChaCha20Rng::seed_from_u64(0)).unwrap();
        let mut bytes = Vec::new(); cipher.serialize_compressed(&mut bytes).unwrap();
        assert!(bytes.len() > YUMA_COMMIT_SIZE_BYTES as usize);
        assert!(bytes.len() <= MAX_CRV3_COMMIT_SIZE_BYTES as usize);
        assert_ok!(SubtensorModule::do_commit_timelocked_weights(RuntimeOrigin::signed(U256::from(0)),
            netuid, bytes.try_into().unwrap(), round, SubtensorModule::get_commit_reveal_weights_version()));
        let committed_epoch = SubtensorModule::current_epoch_with_lookahead(netuid);
        let signature = hex::decode("b44679b9a59af2ec876b1a6b1ad52ea9b1615fc3982b19576350f93447cb1125e342b73a8dd2bacbe47e4b6b63ed5e39").unwrap();
        pallet_drand::Pulses::<Test>::insert(round, Pulse { round, randomness: vec![0; 32].try_into().unwrap(),
            signature: signature.try_into().unwrap() });
        SubnetEpochIndex::<Test>::insert(netuid, committed_epoch + 1);
        assert_ok!(SubtensorModule::reveal_crv3_commits_for_subnet(netuid));
        let stored = Weights::<Test>::get(index, 0);
        assert_eq!(stored.len(), 16_000);
        assert_eq!(stored.iter().map(|(_, weight)| *weight).collect::<Vec<_>>(), values);
        let output = SubtensorModule::epoch_mechanism(netuid, MechId::MAIN, 1_000_001.into());
        assert_eq!(output.as_map().len(), 16_000);
        assert!(output.as_map().values().filter(|t| t.server_emission > 0.into()).count() > 15_000);
        let miners: u128 = output.as_map().values().map(|t| u128::from(u64::from(t.server_emission))).sum();
        let validators: u128 = output.as_map().values().map(|t| u128::from(u64::from(t.validator_emission))).sum();
        assert_eq!(miners, 500_001); assert_eq!(miners + validators, 1_000_001);
    });
}

#[test]
fn null_consensus_commit_queue_budget_is_shared_by_mechanisms() {
    use frame_support::assert_noop;
    new_test_ext(1).execute_with(|| {
        let netuid = setup([300_000_000, 100_000_000]);
        SubtensorModule::set_commit_reveal_weights_enabled(netuid, true);
        assert_ok!(SubtensorModule::do_set_mechanism_count(
            netuid,
            MechId::from(2)
        ));
        let version = SubtensorModule::get_commit_reveal_weights_version();
        for mechanism in 0..2u8 {
            assert_ok!(SubtensorModule::do_commit_timelocked_mechanism_weights(
                RuntimeOrigin::signed(U256::from(0)),
                netuid,
                mechanism.into(),
                vec![0u8; MAX_CRV3_COMMIT_SIZE_BYTES as usize]
                    .try_into()
                    .unwrap(),
                1000,
                version
            ));
        }
        assert_noop!(
            SubtensorModule::do_commit_timelocked_mechanism_weights(
                RuntimeOrigin::signed(U256::from(1)),
                netuid,
                MechId::MAIN,
                vec![0u8; 1].try_into().unwrap(),
                1000,
                version
            ),
            Error::<Test>::CommitQueueFull
        );
    });
}

#[test]
fn null_consensus_yuma_keeps_legacy_ciphertext_limit() {
    use frame_support::assert_noop;
    new_test_ext(1).execute_with(|| {
        let netuid = setup([300_000_000, 100_000_000]);
        SubtensorModule::set_epoch_consensus(netuid, EpochConsensus::Yuma);
        SubtensorModule::set_commit_reveal_weights_enabled(netuid, true);
        assert_noop!(
            SubtensorModule::do_commit_timelocked_weights(
                RuntimeOrigin::signed(U256::from(0)),
                netuid,
                vec![0u8; YUMA_COMMIT_SIZE_BYTES as usize + 1]
                    .try_into()
                    .unwrap(),
                1000,
                SubtensorModule::get_commit_reveal_weights_version()
            ),
            Error::<Test>::CommitPayloadTooLarge
        );
    });
}
