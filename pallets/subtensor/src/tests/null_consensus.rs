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
    for budget in [0, 1, 7, 4_097, 1_000_003, u64::MAX] {
        let mut shares = vec![u128::from(u16::MAX); 4_096];
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
        apportion_units(&[u128::MAX; 4_096], u64::MAX)
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
        // Populate the registered-UID fixtures without 4,096 independent
        // extrinsic transactions. Submission and reveal use production paths.
        SubtensorModule::set_max_allowed_uids(netuid, 4_096);
        for uid in 4..4_096u16 {
            Keys::<Test>::insert(netuid, uid, U256::from(uid));
            Uids::<Test>::insert(netuid, U256::from(uid), uid);
        }
        SubnetworkN::<Test>::insert(netuid, 4_096);
        let mut permits = vec![false; 4_096]; permits[0] = true; permits[1] = true;
        ValidatorPermit::<Test>::insert(netuid, permits);
        LastUpdate::<Test>::insert(index, vec![0u64; 4_096]);
        SubtensorModule::set_commit_reveal_weights_enabled(netuid, true);
        assert_ok!(SubtensorModule::set_reveal_period(netuid, 1));
        let round = 1_000u64;
        let mut values = vec![1u16; 4_096]; values[2] = 65_534;
        let payload = WeightsTlockPayload { hotkey: U256::from(0).encode(),
            uids: (0..4_096u16).collect(), values: values.clone(), version_key: 0 };
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
        assert_eq!(stored.len(), 4_096);
        assert_eq!(stored.iter().map(|(_, weight)| *weight).collect::<Vec<_>>(), values);
        let output = SubtensorModule::epoch_mechanism(netuid, MechId::MAIN, 1_000_001.into());
        assert_eq!(output.as_map().len(), 4_096);
        assert!(output.as_map().values().filter(|t| t.server_emission > 0.into()).count() > 4_000);
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
        for uid in 0..2u16 {
            for mechanism in 0..2u8 {
                assert_ok!(SubtensorModule::do_commit_timelocked_mechanism_weights(
                    RuntimeOrigin::signed(U256::from(uid)),
                    netuid,
                    mechanism.into(),
                    vec![0u8; MAX_CRV3_COMMIT_SIZE_BYTES as usize / 2]
                        .try_into()
                        .unwrap(),
                    1000,
                    version,
                ));
            }
        }
        assert_noop!(
            SubtensorModule::do_commit_timelocked_mechanism_weights(
                RuntimeOrigin::signed(U256::from(2)),
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

#[test]
fn null_timelock_winner_preempts_junk_and_replaces_own_row() {
    use frame_support::assert_noop;
    new_test_ext(1).execute_with(|| {
        let netuid = setup([300_000_000, 100_000_000]);
        SubtensorModule::set_commit_reveal_weights_enabled(netuid, true);
        let version = SubtensorModule::get_commit_reveal_weights_version();
        let submit = |uid, byte| {
            SubtensorModule::do_commit_timelocked_mechanism_weights(
                RuntimeOrigin::signed(U256::from(uid)),
                netuid,
                MechId::MAIN,
                vec![byte; MAX_CRV3_COMMIT_SIZE_BYTES as usize]
                    .try_into()
                    .unwrap(),
                1000,
                version,
            )
        };
        assert_ok!(submit(2u16, 2u8));
        assert_ok!(submit(3u16, 3u8));
        // Two junk rows fill the byte quota. Neither has a validator permit.
        assert_ok!(submit(0u16, 0u8));
        assert_ok!(submit(1u16, 1u8));
        assert_noop!(submit(2u16, 2u8), Error::<Test>::CommitQueueFull);
        assert_ok!(submit(0u16, 9u8));
        let epoch = SubtensorModule::current_epoch_with_lookahead(netuid);
        let queue = TimelockedWeightCommits::<Test>::get(NetUidStorageIndex::from(netuid), epoch);
        assert_eq!(queue.len(), 2);
        assert!(
            queue
                .iter()
                .any(|(who, _, bytes, _)| *who == U256::from(0) && bytes.iter().all(|b| *b == 9))
        );
        assert!(queue.iter().any(|(who, ..)| *who == U256::from(1)));
        // A newly highest-stake permitted validator also gets admission.
        SubtensorModule::increase_stake_for_hotkey_and_coldkey_on_subnet(
            &U256::from(2),
            &U256::from(2),
            netuid,
            500_000_000u64.into(),
        );
        SubtensorModule::set_validator_permit_for_uid(netuid, 2, true);
        assert_ok!(submit(2u16, 2u8));
        let queue = TimelockedWeightCommits::<Test>::get(NetUidStorageIndex::from(netuid), epoch);
        assert!(queue.iter().any(|(who, ..)| *who == U256::from(2)));
        assert!(!queue.iter().any(|(who, ..)| *who == U256::from(1)));
    });
}

#[test]
fn null_timelock_equal_stake_first_uid_preempts_at_count_limit() {
    new_test_ext(1).execute_with(|| {
        let netuid = setup([300_000_000, 300_000_000]);
        SubtensorModule::set_commit_reveal_weights_enabled(netuid, true);
        SubtensorModule::set_max_allowed_uids(netuid, 65);
        SubtensorModule::set_max_allowed_validators(netuid, 65);
        for uid in 4..65u16 {
            let hotkey = U256::from(uid);
            SubtensorModule::append_neuron(netuid, &hotkey, 0);
        }
        for uid in 2..65u16 {
            SubtensorModule::increase_stake_for_hotkey_and_coldkey_on_subnet(
                &U256::from(uid),
                &U256::from(uid),
                netuid,
                300_000_000u64.into(),
            );
            SubtensorModule::set_validator_permit_for_uid(netuid, uid, true);
        }
        let version = SubtensorModule::get_commit_reveal_weights_version();
        for uid in 1..65u16 {
            assert_ok!(SubtensorModule::do_commit_timelocked_mechanism_weights(
                RuntimeOrigin::signed(U256::from(uid)),
                netuid,
                MechId::MAIN,
                vec![1u8].try_into().unwrap(),
                1000,
                version,
            ));
        }
        assert_ok!(SubtensorModule::do_commit_timelocked_mechanism_weights(
            RuntimeOrigin::signed(U256::from(0)),
            netuid,
            MechId::MAIN,
            vec![1u8].try_into().unwrap(),
            1000,
            version,
        ));
        let epoch = SubtensorModule::current_epoch_with_lookahead(netuid);
        let queue = TimelockedWeightCommits::<Test>::get(NetUidStorageIndex::from(netuid), epoch);
        assert_eq!(queue.len(), NULL_COMMIT_QUEUE_COUNT);
        assert!(queue.iter().any(|(who, ..)| *who == U256::from(0)));
        assert!(!queue.iter().any(|(who, ..)| *who == U256::from(64)));
    });
}

#[test]
fn null_consensus_switch_waits_for_timelock_queues_to_drain() {
    use frame_support::assert_noop;
    new_test_ext(1).execute_with(|| {
        let netuid = setup([300_000_000, 100_000_000]);
        SubtensorModule::set_commit_reveal_weights_enabled(netuid, true);
        let version = SubtensorModule::get_commit_reveal_weights_version();
        assert_ok!(SubtensorModule::do_commit_timelocked_mechanism_weights(
            RuntimeOrigin::signed(U256::from(0)),
            netuid,
            MechId::MAIN,
            vec![1u8].try_into().unwrap(),
            1000,
            version,
        ));
        assert_noop!(
            SubtensorModule::do_set_epoch_consensus(netuid, EpochConsensus::Yuma),
            Error::<Test>::InvalidValue
        );
        let index = NetUidStorageIndex::from(netuid);
        let epoch = SubtensorModule::current_epoch_with_lookahead(netuid);
        TimelockedWeightCommits::<Test>::remove(index, epoch);
        assert_ok!(SubtensorModule::do_set_epoch_consensus(
            netuid,
            EpochConsensus::Yuma
        ));
        assert_ok!(SubtensorModule::do_commit_timelocked_mechanism_weights(
            RuntimeOrigin::signed(U256::from(0)),
            netuid,
            MechId::MAIN,
            vec![1u8].try_into().unwrap(),
            1000,
            version,
        ));
        assert_noop!(
            SubtensorModule::do_set_epoch_consensus(netuid, EpochConsensus::Null),
            Error::<Test>::InvalidValue
        );
        TimelockedWeightCommits::<Test>::remove(index, epoch);
        assert_ok!(SubtensorModule::do_set_epoch_consensus(
            netuid,
            EpochConsensus::Null
        ));
    });
}

#[test]
fn null_no_validator_fallback_excludes_recycled_root_budget() {
    new_test_ext(1).execute_with(|| {
        let netuid = setup([0, 0]);
        // Ensure every participant is a miner, including UID zero.
        SubtensorModule::set_validator_permit_for_uid(netuid, 0, false);
        SubtensorModule::set_validator_permit_for_uid(netuid, 1, false);
        SubnetOwner::<Test>::insert(netuid, U256::from(100));
        SubnetAlphaOut::<Test>::insert(netuid, AlphaBalance::from(1_000u64));
        let before: u64 = (0..4u16)
            .map(|uid| {
                SubtensorModule::get_stake_for_hotkey_on_subnet(&U256::from(uid), netuid).to_u64()
            })
            .sum();
        SubtensorModule::distribute_emission(
            netuid,
            500u64.into(),
            200u64.into(),
            300u64.into(),
            0u64.into(),
        );
        let after: u64 = (0..4u16)
            .map(|uid| {
                SubtensorModule::get_stake_for_hotkey_on_subnet(&U256::from(uid), netuid).to_u64()
            })
            .sum();
        assert_eq!(after - before, 700);
        assert_eq!(SubnetAlphaOut::<Test>::get(netuid).to_u64(), 700);
        assert_eq!(
            Emission::<Test>::get(netuid)
                .iter()
                .map(|e| e.to_u64())
                .sum::<u64>(),
            700
        );
    });
}

#[test]
fn null_fallback_whole_units_respect_each_mechanism_budget() {
    new_test_ext(1).execute_with(|| {
        let netuid = setup([0, 0]);
        SubtensorModule::set_validator_permit_for_uid(netuid, 0, false);
        SubtensorModule::set_validator_permit_for_uid(netuid, 1, false);
        assert_ok!(SubtensorModule::do_set_max_mechanism_count(4.into()));
        assert_ok!(SubtensorModule::do_set_mechanism_count(netuid, 4.into()));
        for local_budget in [0u64, 1, 2, 7, 700, u64::MAX - 300] {
            let total_budget = local_budget.saturating_add(300);
            let output = SubtensorModule::epoch_with_mechanism_budgets(
                netuid,
                local_budget.into(),
                0u64.into(),
                total_budget.saturating_sub(local_budget).into(),
            );
            assert_eq!(
                output
                    .iter()
                    .map(|(_, miner, _)| miner.to_u64())
                    .sum::<u64>(),
                local_budget
            );
            assert!(output.iter().all(|(_, _, dividend)| dividend.is_zero()));
            assert_eq!(
                Emission::<Test>::get(netuid)
                    .iter()
                    .map(|amount| amount.to_u64())
                    .sum::<u64>(),
                local_budget
            );
        }
    });
}

#[test]
fn null_bond_cutoff_tracks_last_yuma_epoch_not_null_epochs() {
    new_test_ext(1).execute_with(|| {
        let netuid = setup([300_000_000, 100_000_000]);
        SubtensorModule::set_epoch_consensus(netuid, EpochConsensus::Yuma);
        LastMechansimStepBlock::<Test>::insert(netuid, 10);
        assert_eq!(SubtensorModule::bond_cutoff_block(netuid, 20, 5), 11);
        assert_ok!(SubtensorModule::do_set_epoch_consensus(
            netuid,
            EpochConsensus::Null
        ));
        LastMechansimStepBlock::<Test>::insert(netuid, 100);
        assert_eq!(SubtensorModule::bond_cutoff_block(netuid, 100, 5), 11);
        assert_ok!(SubtensorModule::do_set_epoch_consensus(
            netuid,
            EpochConsensus::Yuma
        ));
        assert_eq!(SubtensorModule::bond_cutoff_block(netuid, 101, 5), 11);
        LastYumaStepBlock::<Test>::insert(netuid, 101);
        assert_eq!(SubtensorModule::bond_cutoff_block(netuid, 102, 5), 102);
    });
}

#[test]
fn null_replaced_miner_bonds_match_clean_baseline_after_return_to_yuma() {
    for yuma3 in [false, true] {
        let run = |stale_bonds| {
            new_test_ext(1).execute_with(|| {
                let netuid = setup([300_000_000, 300_000_000]);
                SubtensorModule::set_epoch_consensus(netuid, EpochConsensus::Yuma);
                SubtensorModule::set_yuma3_enabled(netuid, yuma3);
                LastMechansimStepBlock::<Test>::insert(netuid, 10);
                if stale_bonds {
                    Bonds::<Test>::insert(
                        NetUidStorageIndex::from(netuid),
                        0,
                        vec![(2, 32_767), (3, 32_768)],
                    );
                    Bonds::<Test>::insert(NetUidStorageIndex::from(netuid), 1, vec![(3, u16::MAX)]);
                } else {
                    Bonds::<Test>::insert(NetUidStorageIndex::from(netuid), 0, vec![(2, 32_767)]);
                }
                assert_ok!(SubtensorModule::do_set_epoch_consensus(
                    netuid,
                    EpochConsensus::Null
                ));
                // UID 3 was replaced while bonds were frozen, then Null epochs advanced.
                SubtensorModule::replace_neuron(netuid, 3, &U256::from(4), 50);
                LastMechansimStepBlock::<Test>::insert(netuid, 100);
                System::set_block_number(110);
                set_weights(netuid, 0, vec![2], vec![u16::MAX]);
                set_weights(netuid, 1, vec![2], vec![u16::MAX]);
                assert_ok!(SubtensorModule::do_set_epoch_consensus(
                    netuid,
                    EpochConsensus::Yuma
                ));
                System::set_block_number(111);
                let output =
                    SubtensorModule::epoch_mechanism(netuid, MechId::MAIN, 1_000u64.into());
                output
                    .0
                    .into_iter()
                    .map(|(hotkey, terms)| {
                        (hotkey, terms.bond, terms.dividend, terms.validator_emission)
                    })
                    .collect::<Vec<_>>()
            })
        };
        assert_eq!(run(true), run(false), "Yuma3={yuma3}");
    }
}

#[test]
fn null_multiple_mechanisms_cannot_multiply_miner_rounding_remainders() {
    new_test_ext(1).execute_with(|| {
        let netuid = setup([300_000_000, 100_000_000]);
        assert_ok!(SubtensorModule::do_set_max_mechanism_count(4.into()));
        assert_ok!(SubtensorModule::do_set_mechanism_count(netuid, 4.into()));
        for miner_budget in [0u64, 1, 2, 3, 5, 501, 1_000_001] {
            let output = SubtensorModule::epoch_with_mechanism_budgets(
                netuid,
                miner_budget.into(),
                200u64.into(),
                300u64.into(),
            );
            assert_eq!(
                output
                    .iter()
                    .map(|(_, miner, _)| miner.to_u64())
                    .sum::<u64>(),
                miner_budget
            );
            assert_eq!(
                output
                    .iter()
                    .map(|(_, _, dividend)| dividend.to_u64())
                    .sum::<u64>(),
                500
            );
            assert_eq!(
                Emission::<Test>::get(netuid)
                    .iter()
                    .map(|amount| amount.to_u64())
                    .sum::<u64>(),
                miner_budget + 500
            );
        }
        // Independently rounded 251-unit mechanism totals would pay 504 miners;
        // the public 1,002-unit epoch must pay exactly 501 instead.
        let output = SubtensorModule::epoch_with_mechanisms(netuid, 1_002u64.into());
        assert_eq!(
            output
                .iter()
                .map(|(_, miner, _)| miner.to_u64())
                .sum::<u64>(),
            501
        );
        assert_eq!(
            output
                .iter()
                .map(|(_, _, dividend)| dividend.to_u64())
                .sum::<u64>(),
            501
        );
    });
}
