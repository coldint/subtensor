#![cfg(test)]
#![allow(clippy::arithmetic_side_effects, clippy::unwrap_used)]

use frame_support::{StorageDoubleMap, assert_err, assert_ok, traits::StorePreimage};
use frame_system::pallet_prelude::BlockNumberFor;
use sp_core::U256;
use sp_runtime::DispatchError;

use crate::{BalanceOf, CrowdloanId, CrowdloanInfo, mock::*, pallet as pallet_crowdloan};

#[test]
fn test_create_succeeds() {
    TestState::default()
        .with_balance(U256::from(1), 100)
        .build_and_execute(|| {
            let creator: AccountOf<Test> = U256::from(1);
            let deposit: BalanceOf<Test> = 50;
            let min_contribution: BalanceOf<Test> = 10;
            let cap: BalanceOf<Test> = 300;
            let end: BlockNumberFor<Test> = 50;

            assert_ok!(Crowdloan::create(
                RuntimeOrigin::signed(creator),
                deposit,
                min_contribution,
                cap,
                end,
                Some(noop_call()),
                None,
            ));

            let crowdloan_id = 0;
            let funds_account = pallet_crowdloan::Pallet::<Test>::funds_account(crowdloan_id);
            // ensure the crowdloan is stored correctly
            let call = pallet_preimage::Pallet::<Test>::bound(*noop_call()).unwrap();
            assert_eq!(
                pallet_crowdloan::Crowdloans::<Test>::get(crowdloan_id),
                Some(CrowdloanInfo {
                    creator,
                    deposit,
                    min_contribution,
                    cap,
                    end,
                    funds_account,
                    raised: deposit,
                    target_address: None,
                    call: Some(call),
                    finalized: false,
                    contributors_count: 1,
                })
            );
            // ensure the crowdloan account has the deposit
            assert_eq!(Balances::free_balance(funds_account), deposit);
            // ensure the creator has been deducted the deposit
            assert_eq!(Balances::free_balance(creator), 100 - deposit);
            // ensure the contributions have been updated
            assert_eq!(
                pallet_crowdloan::Contributions::<Test>::iter_prefix(crowdloan_id)
                    .collect::<Vec<_>>(),
                vec![(creator, deposit)]
            );
            // ensure the raised amount is updated correctly
            assert!(
                pallet_crowdloan::Crowdloans::<Test>::get(crowdloan_id)
                    .is_some_and(|c| c.raised == deposit)
            );
            // ensure the event is emitted
            assert_eq!(
                last_event(),
                pallet_crowdloan::Event::<Test>::Created {
                    crowdloan_id,
                    creator,
                    end,
                    cap,
                }
                .into()
            );
            // ensure next crowdloan id is incremented
            assert_eq!(
                pallet_crowdloan::NextCrowdloanId::<Test>::get(),
                crowdloan_id + 1
            );
        });
}

#[test]
fn test_create_fails_if_bad_origin() {
    TestState::default().build_and_execute(|| {
        let deposit: BalanceOf<Test> = 50;
        let min_contribution: BalanceOf<Test> = 10;
        let cap: BalanceOf<Test> = 300;
        let end: BlockNumberFor<Test> = 50;

        assert_err!(
            Crowdloan::create(
                RuntimeOrigin::none(),
                deposit,
                min_contribution,
                cap,
                end,
                Some(noop_call()),
                None
            ),
            DispatchError::BadOrigin
        );

        assert_err!(
            Crowdloan::create(
                RuntimeOrigin::root(),
                deposit,
                min_contribution,
                cap,
                end,
                Some(noop_call()),
                None
            ),
            DispatchError::BadOrigin
        );
    });
}

#[test]
fn test_create_fails_if_deposit_is_too_low() {
    TestState::default()
        .with_balance(U256::from(1), 100)
        .build_and_execute(|| {
            let creator: AccountOf<Test> = U256::from(1);
            let deposit: BalanceOf<Test> = 20;
            let min_contribution: BalanceOf<Test> = 10;
            let cap: BalanceOf<Test> = 300;
            let end: BlockNumberFor<Test> = 50;

            assert_err!(
                Crowdloan::create(
                    RuntimeOrigin::signed(creator),
                    deposit,
                    min_contribution,
                    cap,
                    end,
                    Some(noop_call()),
                    None
                ),
                pallet_crowdloan::Error::<Test>::DepositTooLow
            );
        });
}

#[test]
fn test_create_fails_if_cap_is_not_greater_than_deposit() {
    TestState::default()
        .with_balance(U256::from(1), 100)
        .build_and_execute(|| {
            let creator: AccountOf<Test> = U256::from(1);
            let deposit: BalanceOf<Test> = 50;
            let min_contribution: BalanceOf<Test> = 10;
            let cap: BalanceOf<Test> = 40;
            let end: BlockNumberFor<Test> = 50;

            assert_err!(
                Crowdloan::create(
                    RuntimeOrigin::signed(creator),
                    deposit,
                    min_contribution,
                    cap,
                    end,
                    Some(noop_call()),
                    None
                ),
                pallet_crowdloan::Error::<Test>::CapTooLow
            );
        });
}

#[test]
fn test_create_fails_if_min_contribution_is_too_low() {
    TestState::default()
        .with_balance(U256::from(1), 100)
        .build_and_execute(|| {
            let creator: AccountOf<Test> = U256::from(1);
            let deposit: BalanceOf<Test> = 50;
            let min_contribution: BalanceOf<Test> = 5;
            let cap: BalanceOf<Test> = 300;
            let end: BlockNumberFor<Test> = 50;

            assert_err!(
                Crowdloan::create(
                    RuntimeOrigin::signed(creator),
                    deposit,
                    min_contribution,
                    cap,
                    end,
                    Some(noop_call()),
                    None
                ),
                pallet_crowdloan::Error::<Test>::MinimumContributionTooLow
            );
        });
}

#[test]
fn test_create_fails_if_end_is_in_the_past() {
    let current_block_number: BlockNumberFor<Test> = 10;

    TestState::default()
        .with_block_number(current_block_number)
        .with_balance(U256::from(1), 100)
        .build_and_execute(|| {
            let creator: AccountOf<Test> = U256::from(1);
            let deposit: BalanceOf<Test> = 50;
            let min_contribution: BalanceOf<Test> = 10;
            let cap: BalanceOf<Test> = 300;
            let end: BlockNumberFor<Test> = current_block_number - 5;

            assert_err!(
                Crowdloan::create(
                    RuntimeOrigin::signed(creator),
                    deposit,
                    min_contribution,
                    cap,
                    end,
                    Some(noop_call()),
                    None
                ),
                pallet_crowdloan::Error::<Test>::CannotEndInPast
            );
        });
}

#[test]
fn test_create_fails_if_block_duration_is_too_short() {
    TestState::default()
        .with_balance(U256::from(1), 100)
        .build_and_execute(|| {
            let creator: AccountOf<Test> = U256::from(1);
            let deposit: BalanceOf<Test> = 50;
            let min_contribution: BalanceOf<Test> = 10;
            let cap: BalanceOf<Test> = 300;
            let end: BlockNumberFor<Test> = 11;

            assert_err!(
                Crowdloan::create(
                    RuntimeOrigin::signed(creator),
                    deposit,
                    min_contribution,
                    cap,
                    end,
                    Some(noop_call()),
                    None
                ),
                pallet_crowdloan::Error::<Test>::BlockDurationTooShort
            );
        });
}

#[test]
fn test_create_fails_if_block_duration_is_too_long() {
    TestState::default()
        .with_balance(U256::from(1), 100)
        .build_and_execute(|| {
            let creator: AccountOf<Test> = U256::from(1);
            let deposit: BalanceOf<Test> = 50;
            let min_contribution: BalanceOf<Test> = 10;
            let cap: BalanceOf<Test> = 300;
            let end: BlockNumberFor<Test> = 1000;

            assert_err!(
                Crowdloan::create(
                    RuntimeOrigin::signed(creator),
                    deposit,
                    min_contribution,
                    cap,
                    end,
                    Some(noop_call()),
                    None
                ),
                pallet_crowdloan::Error::<Test>::BlockDurationTooLong
            );
        });
}

#[test]
fn test_create_fails_if_creator_has_insufficient_balance() {
    TestState::default()
        .with_balance(U256::from(1), 100)
        .build_and_execute(|| {
            let creator: AccountOf<Test> = U256::from(1);
            let deposit: BalanceOf<Test> = 200;
            let min_contribution: BalanceOf<Test> = 10;
            let cap: BalanceOf<Test> = 300;
            let end: BlockNumberFor<Test> = 50;

            assert_err!(
                Crowdloan::create(
                    RuntimeOrigin::signed(creator),
                    deposit,
                    min_contribution,
                    cap,
                    end,
                    Some(noop_call()),
                    None
                ),
                pallet_crowdloan::Error::<Test>::InsufficientBalance
            );
        });
}

#[test]
fn test_contribute_succeeds() {
    TestState::default()
        .with_balance(U256::from(1), 200)
        .with_balance(U256::from(2), 500)
        .with_balance(U256::from(3), 200)
        .build_and_execute(|| {
            // create a crowdloan
            let creator: AccountOf<Test> = U256::from(1);
            let initial_deposit: BalanceOf<Test> = 50;
            let min_contribution: BalanceOf<Test> = 10;
            let cap: BalanceOf<Test> = 300;
            let end: BlockNumberFor<Test> = 50;

            assert_ok!(Crowdloan::create(
                RuntimeOrigin::signed(creator),
                initial_deposit,
                min_contribution,
                cap,
                end,
                Some(noop_call()),
                None
            ));

            // run some blocks
            run_to_block(10);

            let crowdloan_id: CrowdloanId = 0;

            // only the creator has contributed so far
            assert!(
                pallet_crowdloan::Crowdloans::<Test>::get(crowdloan_id)
                    .is_some_and(|c| c.contributors_count == 1)
            );

            // first contribution to the crowdloan from creator
            let amount: BalanceOf<Test> = 50;
            assert_ok!(Crowdloan::contribute(
                RuntimeOrigin::signed(creator),
                crowdloan_id,
                amount
            ));
            assert_eq!(
                last_event(),
                pallet_crowdloan::Event::<Test>::Contributed {
                    crowdloan_id,
                    contributor: creator,
                    amount,
                }
                .into()
            );
            assert_eq!(
                pallet_crowdloan::Contributions::<Test>::get(crowdloan_id, creator),
                Some(100)
            );
            assert!(
                pallet_crowdloan::Crowdloans::<Test>::get(crowdloan_id)
                    .is_some_and(|c| c.contributors_count == 1)
            );
            assert_eq!(
                Balances::free_balance(creator),
                200 - amount - initial_deposit
            );

            // second contribution to the crowdloan
            let contributor1: AccountOf<Test> = U256::from(2);
            let amount: BalanceOf<Test> = 100;
            assert_ok!(Crowdloan::contribute(
                RuntimeOrigin::signed(contributor1),
                crowdloan_id,
                amount
            ));
            assert_eq!(
                last_event(),
                pallet_crowdloan::Event::<Test>::Contributed {
                    crowdloan_id,
                    contributor: contributor1,
                    amount,
                }
                .into()
            );
            assert_eq!(
                pallet_crowdloan::Contributions::<Test>::get(crowdloan_id, contributor1),
                Some(100)
            );
            assert!(
                pallet_crowdloan::Crowdloans::<Test>::get(crowdloan_id)
                    .is_some_and(|c| c.contributors_count == 2)
            );
            assert_eq!(Balances::free_balance(contributor1), 500 - amount);

            // third contribution to the crowdloan
            let contributor2: AccountOf<Test> = U256::from(3);
            let amount: BalanceOf<Test> = 50;
            assert_ok!(Crowdloan::contribute(
                RuntimeOrigin::signed(contributor2),
                crowdloan_id,
                amount
            ));
            assert_eq!(
                last_event(),
                pallet_crowdloan::Event::<Test>::Contributed {
                    crowdloan_id,
                    contributor: contributor2,
                    amount,
                }
                .into()
            );
            assert_eq!(
                pallet_crowdloan::Contributions::<Test>::get(crowdloan_id, contributor2),
                Some(50)
            );
            assert!(
                pallet_crowdloan::Crowdloans::<Test>::get(crowdloan_id)
                    .is_some_and(|c| c.contributors_count == 3)
            );
            assert_eq!(Balances::free_balance(contributor2), 200 - amount);

            // ensure the contributions are present in the funds account
            let funds_account = pallet_crowdloan::Pallet::<Test>::funds_account(crowdloan_id);
            assert_eq!(Balances::free_balance(funds_account), 250);

            // ensure the crowdloan raised amount is updated correctly
            assert!(
                pallet_crowdloan::Crowdloans::<Test>::get(crowdloan_id)
                    .is_some_and(|c| c.raised == 250)
            );
        });
}

#[test]
fn test_contribute_succeeds_if_contribution_will_make_the_raised_amount_exceed_the_cap() {
    TestState::default()
        .with_balance(U256::from(1), 200)
        .with_balance(U256::from(2), 500)
        .build_and_execute(|| {
            // create a crowdloan
            let creator: AccountOf<Test> = U256::from(1);
            let initial_deposit: BalanceOf<Test> = 50;
            let min_contribution: BalanceOf<Test> = 10;
            let cap: BalanceOf<Test> = 300;
            let end: BlockNumberFor<Test> = 50;

            assert_ok!(Crowdloan::create(
                RuntimeOrigin::signed(creator),
                initial_deposit,
                min_contribution,
                cap,
                end,
                Some(noop_call()),
                None
            ));

            // run some blocks
            run_to_block(10);

            // first contribution to the crowdloan from creator
            let crowdloan_id: CrowdloanId = 0;
            let amount: BalanceOf<Test> = 50;
            assert_ok!(Crowdloan::contribute(
                RuntimeOrigin::signed(creator),
                crowdloan_id,
                amount
            ));
            assert_eq!(
                last_event(),
                pallet_crowdloan::Event::<Test>::Contributed {
                    crowdloan_id,
                    contributor: creator,
                    amount,
                }
                .into()
            );
            assert_eq!(
                pallet_crowdloan::Contributions::<Test>::get(crowdloan_id, creator),
                Some(100)
            );
            assert_eq!(
                Balances::free_balance(creator),
                200 - amount - initial_deposit
            );

            // second contribution to the crowdloan above the cap
            let contributor1: AccountOf<Test> = U256::from(2);
            let amount: BalanceOf<Test> = 300;
            assert_ok!(Crowdloan::contribute(
                RuntimeOrigin::signed(contributor1),
                crowdloan_id,
                amount
            ));
            assert_eq!(
                last_event(),
                pallet_crowdloan::Event::<Test>::Contributed {
                    crowdloan_id,
                    contributor: contributor1,
                    amount: 200, // the amount is capped at the cap
                }
                .into()
            );
            assert_eq!(
                pallet_crowdloan::Contributions::<Test>::get(crowdloan_id, contributor1),
                Some(200)
            );
            assert_eq!(Balances::free_balance(contributor1), 500 - 200);

            // ensure the contributions are present in the crowdloan account up to the cap
            let funds_account = pallet_crowdloan::Pallet::<Test>::funds_account(crowdloan_id);
            assert_eq!(Balances::free_balance(funds_account), 300);

            // ensure the crowdloan raised amount is updated correctly
            assert!(
                pallet_crowdloan::Crowdloans::<Test>::get(crowdloan_id)
                    .is_some_and(|c| c.raised == 300)
            );
        });
}

#[test]
fn test_contribute_fails_if_bad_origin() {
    TestState::default().build_and_execute(|| {
        let crowdloan_id: CrowdloanId = 0;
        let amount: BalanceOf<Test> = 100;

        assert_err!(
            Crowdloan::contribute(RuntimeOrigin::none(), crowdloan_id, amount),
            DispatchError::BadOrigin
        );

        assert_err!(
            Crowdloan::contribute(RuntimeOrigin::root(), crowdloan_id, amount),
            DispatchError::BadOrigin
        );
    });
}

#[test]
fn test_contribute_fails_if_crowdloan_does_not_exist() {
    TestState::default()
        .with_balance(U256::from(1), 100)
        .build_and_execute(|| {
            let contributor: AccountOf<Test> = U256::from(1);
            let crowdloan_id: CrowdloanId = 0;
            let amount: BalanceOf<Test> = 20;

            assert_err!(
                Crowdloan::contribute(RuntimeOrigin::signed(contributor), crowdloan_id, amount),
                pallet_crowdloan::Error::<Test>::InvalidCrowdloanId
            );
        });
}

#[test]
fn test_contribute_fails_if_contribution_period_ended() {
    TestState::default()
        .with_balance(U256::from(1), 100)
        .with_balance(U256::from(2), 100)
        .build_and_execute(|| {
            // create a crowdloan
            let creator: AccountOf<Test> = U256::from(1);
            let initial_deposit: BalanceOf<Test> = 50;
            let min_contribution: BalanceOf<Test> = 10;
            let cap: BalanceOf<Test> = 300;
            let end: BlockNumberFor<Test> = 50;

            assert_ok!(Crowdloan::create(
                RuntimeOrigin::signed(creator),
                initial_deposit,
                min_contribution,
                cap,
                end,
                Some(noop_call()),
                None
            ));

            // run past the end of the crowdloan
            run_to_block(60);

            // contribute to the crowdloan
            let contributor: AccountOf<Test> = U256::from(2);
            let crowdloan_id: CrowdloanId = 0;
            let amount: BalanceOf<Test> = 20;
            assert_err!(
                Crowdloan::contribute(RuntimeOrigin::signed(contributor), crowdloan_id, amount),
                pallet_crowdloan::Error::<Test>::ContributionPeriodEnded
            );
        });
}

#[test]
fn test_contribute_fails_if_cap_has_been_raised() {
    TestState::default()
        .with_balance(U256::from(1), 100)
        .with_balance(U256::from(2), 1000)
        .with_balance(U256::from(3), 100)
        .build_and_execute(|| {
            // create a crowdloan
            let creator: AccountOf<Test> = U256::from(1);
            let initial_deposit: BalanceOf<Test> = 50;
            let min_contribution: BalanceOf<Test> = 10;
            let cap: BalanceOf<Test> = 300;
            let end: BlockNumberFor<Test> = 50;

            assert_ok!(Crowdloan::create(
                RuntimeOrigin::signed(creator),
                initial_deposit,
                min_contribution,
                cap,
                end,
                Some(noop_call()),
                None
            ));

            // run some blocks
            run_to_block(10);

            // first contribution to the crowdloan fully raise the cap
            let crowdloan_id: CrowdloanId = 0;
            let contributor1: AccountOf<Test> = U256::from(2);
            let amount: BalanceOf<Test> = cap - initial_deposit;
            assert_ok!(Crowdloan::contribute(
                RuntimeOrigin::signed(contributor1),
                crowdloan_id,
                amount
            ));

            // second contribution to the crowdloan
            let contributor2: AccountOf<Test> = U256::from(3);
            let amount: BalanceOf<Test> = 10;
            assert_err!(
                Crowdloan::contribute(RuntimeOrigin::signed(contributor2), crowdloan_id, amount),
                pallet_crowdloan::Error::<Test>::CapRaised
            );
        });
}

#[test]
fn test_contribute_fails_if_contribution_is_below_minimum_contribution() {
    TestState::default()
        .with_balance(U256::from(1), 100)
        .with_balance(U256::from(2), 100)
        .build_and_execute(|| {
            // create a crowdloan
            let creator: AccountOf<Test> = U256::from(1);
            let initial_deposit: BalanceOf<Test> = 50;
            let min_contribution: BalanceOf<Test> = 10;
            let cap: BalanceOf<Test> = 300;
            let end: BlockNumberFor<Test> = 50;

            assert_ok!(Crowdloan::create(
                RuntimeOrigin::signed(creator),
                initial_deposit,
                min_contribution,
                cap,
                end,
                Some(noop_call()),
                None
            ));

            // run some blocks
            run_to_block(10);

            // contribute to the crowdloan
            let contributor: AccountOf<Test> = U256::from(2);
            let crowdloan_id: CrowdloanId = 0;
            let amount: BalanceOf<Test> = 5;
            assert_err!(
                Crowdloan::contribute(RuntimeOrigin::signed(contributor), crowdloan_id, amount),
                pallet_crowdloan::Error::<Test>::ContributionTooLow
            )
        });
}

#[test]
fn test_contribute_fails_if_max_contributors_has_been_reached() {
    TestState::default()
        .with_balance(U256::from(1), 100)
        .with_balance(U256::from(2), 100)
        .with_balance(U256::from(3), 100)
        .with_balance(U256::from(4), 100)
        .with_balance(U256::from(5), 100)
        .with_balance(U256::from(6), 100)
        .with_balance(U256::from(7), 100)
        .with_balance(U256::from(8), 100)
        .with_balance(U256::from(9), 100)
        .with_balance(U256::from(10), 100)
        .with_balance(U256::from(11), 100)
        .build_and_execute(|| {
            // create a crowdloan
            let creator: AccountOf<Test> = U256::from(1);
            let initial_deposit: BalanceOf<Test> = 50;
            let min_contribution: BalanceOf<Test> = 10;
            let cap: BalanceOf<Test> = 1000;
            let end: BlockNumberFor<Test> = 50;

            assert_ok!(Crowdloan::create(
                RuntimeOrigin::signed(creator),
                initial_deposit,
                min_contribution,
                cap,
                end,
                Some(noop_call()),
                None
            ));

            // run some blocks
            run_to_block(10);

            // contribute to the crowdloan
            let crowdloan_id: CrowdloanId = 0;
            let amount: BalanceOf<Test> = 20;
            for i in 2..=10 {
                let contributor: AccountOf<Test> = U256::from(i);
                assert_ok!(Crowdloan::contribute(
                    RuntimeOrigin::signed(contributor),
                    crowdloan_id,
                    amount
                ));
            }

            // try to contribute
            let contributor: AccountOf<Test> = U256::from(10);
            assert_err!(
                Crowdloan::contribute(RuntimeOrigin::signed(contributor), crowdloan_id, amount),
                pallet_crowdloan::Error::<Test>::MaxContributorsReached
            );
        });
}

#[test]
fn test_contribute_fails_if_contributor_has_insufficient_balance() {
    TestState::default()
        .with_balance(U256::from(1), 100)
        .with_balance(U256::from(2), 50)
        .build_and_execute(|| {
            // create a crowdloan
            let creator: AccountOf<Test> = U256::from(1);
            let initial_deposit: BalanceOf<Test> = 50;
            let min_contribution: BalanceOf<Test> = 10;
            let cap: BalanceOf<Test> = 300;
            let end: BlockNumberFor<Test> = 50;

            assert_ok!(Crowdloan::create(
                RuntimeOrigin::signed(creator),
                initial_deposit,
                min_contribution,
                cap,
                end,
                Some(noop_call()),
                None
            ));

            // run some blocks
            run_to_block(10);

            // contribute to the crowdloan
            let crowdloan_id: CrowdloanId = 0;
            let contributor: AccountOf<Test> = U256::from(2);
            let amount: BalanceOf<Test> = 100;

            assert_err!(
                Crowdloan::contribute(RuntimeOrigin::signed(contributor), crowdloan_id, amount),
                pallet_crowdloan::Error::<Test>::InsufficientBalance
            );
        });
}

#[test]
fn test_withdraw_from_contributor_succeeds() {
    TestState::default()
        .with_balance(U256::from(1), 100)
        .with_balance(U256::from(2), 100)
        .with_balance(U256::from(3), 100)
        .build_and_execute(|| {
            // create a crowdloan
            let creator: AccountOf<Test> = U256::from(1);
            let initial_deposit: BalanceOf<Test> = 50;
            let min_contribution: BalanceOf<Test> = 10;
            let cap: BalanceOf<Test> = 300;
            let end: BlockNumberFor<Test> = 50;

            assert_ok!(Crowdloan::create(
                RuntimeOrigin::signed(creator),
                initial_deposit,
                min_contribution,
                cap,
                end,
                Some(noop_call()),
                None
            ));

            // run some blocks
            run_to_block(10);

            // contribute to the crowdloan
            let crowdloan_id: CrowdloanId = 0;

            let contributor1: AccountOf<Test> = U256::from(2);
            let amount1: BalanceOf<Test> = 100;
            assert_ok!(Crowdloan::contribute(
                RuntimeOrigin::signed(contributor1),
                crowdloan_id,
                amount1
            ));

            let contributor2: AccountOf<Test> = U256::from(3);
            let amount2: BalanceOf<Test> = 100;
            assert_ok!(Crowdloan::contribute(
                RuntimeOrigin::signed(contributor2),
                crowdloan_id,
                amount2
            ));

            // run some more blocks past the end of the contribution period
            run_to_block(60);

            // ensure the contributor count is correct
            assert!(
                pallet_crowdloan::Crowdloans::<Test>::get(crowdloan_id)
                    .is_some_and(|c| c.contributors_count == 3)
            );

            // withdraw from contributor1
            assert_ok!(Crowdloan::withdraw(
                RuntimeOrigin::signed(contributor1),
                crowdloan_id
            ));
            // ensure the contributor1 contribution has been removed
            assert_eq!(
                pallet_crowdloan::Contributions::<Test>::get(crowdloan_id, contributor1),
                None,
            );
            assert!(
                pallet_crowdloan::Crowdloans::<Test>::get(crowdloan_id)
                    .is_some_and(|c| c.contributors_count == 2)
            );
            // ensure the contributor1 has the correct amount
            assert_eq!(
                pallet_balances::Pallet::<Test>::free_balance(contributor1),
                100
            );

            // withdraw from contributor2
            assert_ok!(Crowdloan::withdraw(
                RuntimeOrigin::signed(contributor2),
                crowdloan_id
            ));
            // ensure the contributor2 contribution has been removed
            assert_eq!(
                pallet_crowdloan::Contributions::<Test>::get(crowdloan_id, contributor2),
                None,
            );
            assert!(
                pallet_crowdloan::Crowdloans::<Test>::get(crowdloan_id)
                    .is_some_and(|c| c.contributors_count == 1)
            );
            // ensure the contributor2 has the correct amount
            assert_eq!(
                pallet_balances::Pallet::<Test>::free_balance(contributor2),
                100
            );

            // ensure the crowdloan account has the correct amount
            let funds_account = pallet_crowdloan::Pallet::<Test>::funds_account(crowdloan_id);
            assert_eq!(Balances::free_balance(funds_account), initial_deposit);
            // ensure the crowdloan raised amount is updated correctly
            assert!(
                pallet_crowdloan::Crowdloans::<Test>::get(crowdloan_id)
                    .is_some_and(|c| c.raised == initial_deposit)
            );
        });
}

#[test]
fn test_withdraw_from_creator_with_contribution_over_deposit_succeeds() {
    TestState::default()
        .with_balance(U256::from(1), 200)
        .build_and_execute(|| {
            // create a crowdloan
            let creator: AccountOf<Test> = U256::from(1);
            let initial_deposit: BalanceOf<Test> = 50;
            let min_contribution: BalanceOf<Test> = 10;
            let cap: BalanceOf<Test> = 300;
            let end: BlockNumberFor<Test> = 50;

            assert_ok!(Crowdloan::create(
                RuntimeOrigin::signed(creator),
                initial_deposit,
                min_contribution,
                cap,
                end,
                Some(noop_call()),
                None
            ));

            // contribute to the crowdloan as the creator
            let crowdloan_id: CrowdloanId = 0;

            let amount: BalanceOf<Test> = 100;
            assert_ok!(Crowdloan::contribute(
                RuntimeOrigin::signed(creator),
                crowdloan_id,
                amount
            ));

            // ensure the contributor count is correct
            assert!(
                pallet_crowdloan::Crowdloans::<Test>::get(crowdloan_id)
                    .is_some_and(|c| c.contributors_count == 1)
            );

            // withdraw
            let crowdloan_id: CrowdloanId = 0;
            assert_ok!(Crowdloan::withdraw(
                RuntimeOrigin::signed(creator),
                crowdloan_id
            ));

            // ensure the creator has the correct amount
            assert_eq!(
                pallet_balances::Pallet::<Test>::free_balance(creator),
                200 - initial_deposit
            );
            // ensure the creator contribution has been removed
            assert_eq!(
                pallet_crowdloan::Contributions::<Test>::get(crowdloan_id, creator),
                Some(initial_deposit),
            );
            // ensure the contributor count hasn't changed because deposit is kept
            assert!(
                pallet_crowdloan::Crowdloans::<Test>::get(crowdloan_id)
                    .is_some_and(|c| c.contributors_count == 1)
            );

            // ensure the crowdloan account has the correct amount
            let funds_account = pallet_crowdloan::Pallet::<Test>::funds_account(crowdloan_id);
            assert_eq!(Balances::free_balance(funds_account), initial_deposit);
            // ensure the crowdloan raised amount is updated correctly
            assert!(
                pallet_crowdloan::Crowdloans::<Test>::get(crowdloan_id)
                    .is_some_and(|c| c.raised == initial_deposit)
            );
        });
}
#[test]
fn test_withdraw_fails_from_creator_with_no_contribution_over_deposit() {
    TestState::default()
        .with_balance(U256::from(1), 100)
        .with_balance(U256::from(2), 200)
        .build_and_execute(|| {
            // create a crowdloan
            let creator: AccountOf<Test> = U256::from(1);
            let initial_deposit: BalanceOf<Test> = 50;
            let min_contribution: BalanceOf<Test> = 10;
            let cap: BalanceOf<Test> = 300;
            let end: BlockNumberFor<Test> = 50;

            assert_ok!(Crowdloan::create(
                RuntimeOrigin::signed(creator),
                initial_deposit,
                min_contribution,
                cap,
                end,
                Some(noop_call()),
                None
            ));

            // try to withdraw
            let crowdloan_id: CrowdloanId = 0;
            assert_err!(
                Crowdloan::withdraw(RuntimeOrigin::signed(creator), crowdloan_id),
                pallet_crowdloan::Error::<Test>::DepositCannotBeWithdrawn
            );

            // ensure the crowdloan account has the correct amount
            let funds_account = pallet_crowdloan::Pallet::<Test>::funds_account(crowdloan_id);
            assert_eq!(Balances::free_balance(funds_account), initial_deposit);
            // ensure the crowdloan raised amount is updated correctly
            assert!(
                pallet_crowdloan::Crowdloans::<Test>::get(crowdloan_id)
                    .is_some_and(|c| c.raised == initial_deposit)
            );
        });
}

#[test]
fn test_withdraw_fails_if_bad_origin() {
    TestState::default().build_and_execute(|| {
        let crowdloan_id: CrowdloanId = 0;

        assert_err!(
            Crowdloan::withdraw(RuntimeOrigin::none(), crowdloan_id),
            DispatchError::BadOrigin
        );

        assert_err!(
            Crowdloan::withdraw(RuntimeOrigin::root(), crowdloan_id),
            DispatchError::BadOrigin
        );
    });
}

#[test]
fn test_withdraw_fails_if_crowdloan_does_not_exists() {
    TestState::default().build_and_execute(|| {
        let contributor: AccountOf<Test> = U256::from(1);
        let crowdloan_id: CrowdloanId = 0;

        assert_err!(
            Crowdloan::withdraw(RuntimeOrigin::signed(contributor), crowdloan_id),
            pallet_crowdloan::Error::<Test>::InvalidCrowdloanId
        );
    });
}

#[test]
fn test_withdraw_fails_if_crowdloan_has_already_been_finalized() {
    TestState::default()
        .with_balance(U256::from(1), 100)
        .with_balance(U256::from(2), 200)
        .build_and_execute(|| {
            // create a crowdloan
            let creator: AccountOf<Test> = U256::from(1);
            let deposit: BalanceOf<Test> = 50;
            let min_contribution: BalanceOf<Test> = 10;
            let cap: BalanceOf<Test> = 100;
            let end: BlockNumberFor<Test> = 50;

            assert_ok!(Crowdloan::create(
                RuntimeOrigin::signed(creator),
                deposit,
                min_contribution,
                cap,
                end,
                Some(noop_call()),
                None,
            ));

            // some contribution
            let crowdloan_id: CrowdloanId = 0;
            let contributor: AccountOf<Test> = U256::from(2);
            let amount: BalanceOf<Test> = 50;

            assert_ok!(Crowdloan::contribute(
                RuntimeOrigin::signed(contributor),
                crowdloan_id,
                amount
            ));

            // run some more blocks past the end of the contribution period
            run_to_block(60);

            // finalize the crowdloan
            assert_ok!(Crowdloan::finalize(
                RuntimeOrigin::signed(creator),
                crowdloan_id
            ));

            // try to withdraw
            assert_err!(
                Crowdloan::withdraw(RuntimeOrigin::signed(creator), crowdloan_id),
                pallet_crowdloan::Error::<Test>::AlreadyFinalized
            );
        });
}

#[test]
fn test_withdraw_fails_if_no_contribution_exists() {
    TestState::default()
        .with_balance(U256::from(1), 100)
        .with_balance(U256::from(2), 200)
        .build_and_execute(|| {
            // create a crowdloan
            let creator: AccountOf<Test> = U256::from(1);
            let initial_deposit: BalanceOf<Test> = 50;
            let min_contribution: BalanceOf<Test> = 10;
            let cap: BalanceOf<Test> = 300;
            let end: BlockNumberFor<Test> = 50;

            assert_ok!(Crowdloan::create(
                RuntimeOrigin::signed(creator),
                initial_deposit,
                min_contribution,
                cap,
                end,
                Some(noop_call()),
                None
            ));

            // run some more blocks past the end of the contribution period
            run_to_block(60);

            // try to withdraw
            let crowdloan_id: CrowdloanId = 0;
            let contributor: AccountOf<Test> = U256::from(2);
            assert_err!(
                Crowdloan::withdraw(RuntimeOrigin::signed(contributor), crowdloan_id),
                pallet_crowdloan::Error::<Test>::NoContribution
            );
        });
}

#[test]
fn test_finalize_succeeds() {
    TestState::default()
        .with_balance(U256::from(1), 100)
        .with_balance(U256::from(2), 100)
        .build_and_execute(|| {
            // create a crowdloan
            let creator: AccountOf<Test> = U256::from(1);
            let deposit: BalanceOf<Test> = 50;
            let min_contribution: BalanceOf<Test> = 10;
            let cap: BalanceOf<Test> = 100;
            let end: BlockNumberFor<Test> = 50;
            let call = Box::new(RuntimeCall::TestPallet(
                pallet_test::Call::<Test>::transfer_funds {
                    dest: U256::from(42),
                },
            ));

            assert_ok!(Crowdloan::create(
                RuntimeOrigin::signed(creator),
                deposit,
                min_contribution,
                cap,
                end,
                Some(call),
                None
            ));

            // run some blocks
            run_to_block(10);

            // some contribution
            let crowdloan_id: CrowdloanId = 0;
            let contributor: AccountOf<Test> = U256::from(2);
            let amount: BalanceOf<Test> = 50;

            assert_ok!(Crowdloan::contribute(
                RuntimeOrigin::signed(contributor),
                crowdloan_id,
                amount
            ));

            // run some more blocks past the end of the contribution period
            run_to_block(60);

            // finalize the crowdloan
            assert_ok!(Crowdloan::finalize(
                RuntimeOrigin::signed(creator),
                crowdloan_id
            ));

            // ensure the transfer was a success from the dispatched call
            assert_eq!(
                pallet_balances::Pallet::<Test>::free_balance(U256::from(42)),
                100
            );

            // ensure the crowdloan is marked as finalized
            assert!(
                pallet_crowdloan::Crowdloans::<Test>::get(crowdloan_id)
                    .is_some_and(|c| c.finalized)
            );

            // ensure the event is emitted
            assert_eq!(
                last_event(),
                pallet_crowdloan::Event::<Test>::Finalized { crowdloan_id }.into()
            );

            // ensure the current crowdloan id was accessible from the dispatched call
            assert_eq!(
                pallet_test::PassedCrowdloanId::<Test>::get(),
                Some(crowdloan_id)
            );
        });
}

#[test]
fn test_finalize_succeeds_with_target_address() {
    TestState::default()
        .with_balance(U256::from(1), 100)
        .with_balance(U256::from(2), 100)
        .build_and_execute(|| {
            // create a crowdloan
            let creator: AccountOf<Test> = U256::from(1);
            let deposit: BalanceOf<Test> = 50;
            let min_contribution: BalanceOf<Test> = 10;
            let cap: BalanceOf<Test> = 100;
            let end: BlockNumberFor<Test> = 50;
            let target_address: AccountOf<Test> = U256::from(42);
            let call = Box::new(RuntimeCall::TestPallet(
                pallet_test::Call::<Test>::set_passed_crowdloan_id {},
            ));

            assert_ok!(Crowdloan::create(
                RuntimeOrigin::signed(creator),
                deposit,
                min_contribution,
                cap,
                end,
                Some(call),
                Some(target_address),
            ));

            // run some blocks
            run_to_block(10);

            // some contribution
            let crowdloan_id: CrowdloanId = 0;
            let contributor: AccountOf<Test> = U256::from(2);
            let amount: BalanceOf<Test> = 50;

            assert_ok!(Crowdloan::contribute(
                RuntimeOrigin::signed(contributor),
                crowdloan_id,
                amount
            ));

            // run some more blocks past the end of the contribution period
            run_to_block(60);

            // finalize the crowdloan
            assert_ok!(Crowdloan::finalize(
                RuntimeOrigin::signed(creator),
                crowdloan_id
            ));

            // ensure the target address has received the funds
            assert_eq!(
                pallet_balances::Pallet::<Test>::free_balance(target_address),
                100
            );

            // ensure the crowdloan is marked as finalized
            assert!(
                pallet_crowdloan::Crowdloans::<Test>::get(crowdloan_id)
                    .is_some_and(|c| c.finalized)
            );

            // ensure the event is emitted
            assert_eq!(
                last_event(),
                pallet_crowdloan::Event::<Test>::Finalized { crowdloan_id }.into()
            );

            // ensure the current crowdloan id was accessible from the dispatched call
            assert_eq!(
                pallet_test::PassedCrowdloanId::<Test>::get(),
                Some(crowdloan_id)
            );
        })
}

#[test]
fn test_finalize_fails_if_bad_origin() {
    TestState::default()
        .with_balance(U256::from(1), 100)
        .build_and_execute(|| {
            let crowdloan_id: CrowdloanId = 0;

            assert_err!(
                Crowdloan::finalize(RuntimeOrigin::none(), crowdloan_id),
                DispatchError::BadOrigin
            );

            assert_err!(
                Crowdloan::finalize(RuntimeOrigin::root(), crowdloan_id),
                DispatchError::BadOrigin
            );
        });
}

#[test]
fn test_finalize_fails_if_crowdloan_does_not_exist() {
    TestState::default()
        .with_balance(U256::from(1), 100)
        .build_and_execute(|| {
            let creator: AccountOf<Test> = U256::from(1);
            let crowdloan_id: CrowdloanId = 0;

            // try to finalize
            assert_err!(
                Crowdloan::finalize(RuntimeOrigin::signed(creator), crowdloan_id),
                pallet_crowdloan::Error::<Test>::InvalidCrowdloanId
            );
        });
}

#[test]
fn test_finalize_fails_if_not_creator_origin() {
    TestState::default()
        .with_balance(U256::from(1), 100)
        .with_balance(U256::from(2), 100)
        .build_and_execute(|| {
            // create a crowdloan
            let creator: AccountOf<Test> = U256::from(1);
            let deposit: BalanceOf<Test> = 50;
            let min_contribution: BalanceOf<Test> = 10;
            let cap: BalanceOf<Test> = 100;
            let end: BlockNumberFor<Test> = 50;

            assert_ok!(Crowdloan::create(
                RuntimeOrigin::signed(creator),
                deposit,
                min_contribution,
                cap,
                end,
                Some(noop_call()),
                None
            ));

            // run some blocks
            run_to_block(10);

            // some contribution
            let crowdloan_id: CrowdloanId = 0;
            let contributor: AccountOf<Test> = U256::from(2);
            let amount: BalanceOf<Test> = 50;
            assert_ok!(Crowdloan::contribute(
                RuntimeOrigin::signed(contributor),
                crowdloan_id,
                amount
            ));

            // run some more blocks past the end of the contribution period
            run_to_block(60);

            // try finalize the crowdloan
            assert_err!(
                Crowdloan::finalize(RuntimeOrigin::signed(contributor), crowdloan_id),
                pallet_crowdloan::Error::<Test>::InvalidOrigin
            );
        });
}

#[test]
fn test_finalize_fails_if_crowdloan_has_not_ended() {
    TestState::default()
        .with_balance(U256::from(1), 100)
        .with_balance(U256::from(2), 100)
        .build_and_execute(|| {
            // create a crowdloan
            let creator: AccountOf<Test> = U256::from(1);
            let deposit: BalanceOf<Test> = 50;
            let min_contribution: BalanceOf<Test> = 10;
            let cap: BalanceOf<Test> = 100;
            let end: BlockNumberFor<Test> = 50;

            assert_ok!(Crowdloan::create(
                RuntimeOrigin::signed(creator),
                deposit,
                min_contribution,
                cap,
                end,
                Some(noop_call()),
                None,
            ));

            // run some blocks
            run_to_block(10);

            // some contribution
            let crowdloan_id: CrowdloanId = 0;
            let contributor: AccountOf<Test> = U256::from(2);
            let amount: BalanceOf<Test> = 50;

            assert_ok!(Crowdloan::contribute(
                RuntimeOrigin::signed(contributor),
                crowdloan_id,
                amount
            ));

            // run some more blocks before end of contribution period
            run_to_block(10);

            // try to finalize
            assert_err!(
                Crowdloan::finalize(RuntimeOrigin::signed(creator), crowdloan_id),
                pallet_crowdloan::Error::<Test>::ContributionPeriodNotEnded
            );
        });
}

#[test]
fn test_finalize_fails_if_crowdloan_cap_is_not_raised() {
    TestState::default()
        .with_balance(U256::from(1), 100)
        .with_balance(U256::from(2), 100)
        .build_and_execute(|| {
            // create a crowdloan
            let creator: AccountOf<Test> = U256::from(1);
            let deposit: BalanceOf<Test> = 50;
            let min_contribution: BalanceOf<Test> = 10;
            let cap: BalanceOf<Test> = 100;
            let end: BlockNumberFor<Test> = 50;

            assert_ok!(Crowdloan::create(
                RuntimeOrigin::signed(creator),
                deposit,
                min_contribution,
                cap,
                end,
                Some(noop_call()),
                None,
            ));

            // run some blocks
            run_to_block(10);

            // some contribution
            let crowdloan_id: CrowdloanId = 0;
            let contributor: AccountOf<Test> = U256::from(2);
            let amount: BalanceOf<Test> = 49; // below cap

            assert_ok!(Crowdloan::contribute(
                RuntimeOrigin::signed(contributor),
                crowdloan_id,
                amount
            ));

            // run some more blocks past the end of the contribution period
            run_to_block(60);

            // try finalize the crowdloan
            assert_err!(
                Crowdloan::finalize(RuntimeOrigin::signed(creator), crowdloan_id),
                pallet_crowdloan::Error::<Test>::CapNotRaised
            );
        });
}

#[test]
fn test_finalize_fails_if_crowdloan_has_already_been_finalized() {
    TestState::default()
        .with_balance(U256::from(1), 100)
        .with_balance(U256::from(2), 100)
        .build_and_execute(|| {
            // create a crowdloan
            let creator: AccountOf<Test> = U256::from(1);
            let deposit: BalanceOf<Test> = 50;
            let min_contribution: BalanceOf<Test> = 10;
            let cap: BalanceOf<Test> = 100;
            let end: BlockNumberFor<Test> = 50;

            assert_ok!(Crowdloan::create(
                RuntimeOrigin::signed(creator),
                deposit,
                min_contribution,
                cap,
                end,
                Some(noop_call()),
                None,
            ));

            // some contribution
            let crowdloan_id: CrowdloanId = 0;
            let contributor: AccountOf<Test> = U256::from(2);
            let amount: BalanceOf<Test> = 50;

            assert_ok!(Crowdloan::contribute(
                RuntimeOrigin::signed(contributor),
                crowdloan_id,
                amount
            ));

            // run some more blocks past the end of the contribution period
            run_to_block(60);

            // finalize the crowdloan
            assert_ok!(Crowdloan::finalize(
                RuntimeOrigin::signed(creator),
                crowdloan_id
            ));

            // try finalize the crowdloan a second time
            assert_err!(
                Crowdloan::finalize(RuntimeOrigin::signed(creator), crowdloan_id),
                pallet_crowdloan::Error::<Test>::AlreadyFinalized
            );
        });
}

#[test]
fn test_finalize_fails_if_call_fails() {
    TestState::default()
        .with_balance(U256::from(1), 100)
        .with_balance(U256::from(2), 100)
        .build_and_execute(|| {
            // create a crowdloan
            let creator: AccountOf<Test> = U256::from(1);
            let deposit: BalanceOf<Test> = 50;
            let min_contribution: BalanceOf<Test> = 10;
            let cap: BalanceOf<Test> = 100;
            let end: BlockNumberFor<Test> = 50;
            let call = Box::new(RuntimeCall::TestPallet(
                pallet_test::Call::<Test>::failing_extrinsic {},
            ));

            assert_ok!(Crowdloan::create(
                RuntimeOrigin::signed(creator),
                deposit,
                min_contribution,
                cap,
                end,
                Some(call),
                None,
            ));

            // run some blocks
            run_to_block(10);

            // some contribution
            let crowdloan_id: CrowdloanId = 0;
            let contributor: AccountOf<Test> = U256::from(2);
            let amount: BalanceOf<Test> = 50;
            assert_ok!(Crowdloan::contribute(
                RuntimeOrigin::signed(contributor),
                crowdloan_id,
                amount
            ));

            // run some more blocks past the end of the contribution period
            run_to_block(60);

            // try finalize the crowdloan
            assert_err!(
                Crowdloan::finalize(RuntimeOrigin::signed(creator), crowdloan_id),
                pallet_test::Error::<Test>::ShouldFail
            );
        });
}

#[test]
fn test_refund_succeeds() {
    TestState::default()
        .with_balance(U256::from(1), 100)
        .with_balance(U256::from(2), 100)
        .with_balance(U256::from(3), 100)
        .with_balance(U256::from(4), 100)
        .with_balance(U256::from(5), 100)
        .with_balance(U256::from(6), 100)
        .with_balance(U256::from(7), 100)
        .build_and_execute(|| {
            // create a crowdloan
            let creator: AccountOf<Test> = U256::from(1);
            let initial_deposit: BalanceOf<Test> = 50;
            let min_contribution: BalanceOf<Test> = 10;
            let cap: BalanceOf<Test> = 400;
            let end: BlockNumberFor<Test> = 50;
            assert_ok!(Crowdloan::create(
                RuntimeOrigin::signed(creator),
                initial_deposit,
                min_contribution,
                cap,
                end,
                Some(noop_call()),
                None,
            ));

            // run some blocks
            run_to_block(10);

            // make 6 contributions to reach 350 raised amount (initial deposit + contributions)
            let crowdloan_id: CrowdloanId = 0;
            let amount: BalanceOf<Test> = 50;
            for i in 2..8 {
                let contributor: AccountOf<Test> = U256::from(i);
                assert_ok!(Crowdloan::contribute(
                    RuntimeOrigin::signed(contributor),
                    crowdloan_id,
                    amount
                ));
            }

            // ensure the contributor count is correct
            assert!(
                pallet_crowdloan::Crowdloans::<Test>::get(crowdloan_id)
                    .is_some_and(|c| c.contributors_count == 7)
            );

            // run some more blocks past the end of the contribution period
            run_to_block(60);

            //  first round of refund
            assert_ok!(Crowdloan::refund(
                RuntimeOrigin::signed(creator),
                crowdloan_id
            ));

            // ensure the contributor count is correct, we processed 5 refunds
            assert!(
                pallet_crowdloan::Crowdloans::<Test>::get(crowdloan_id)
                    .is_some_and(|c| c.contributors_count == 2)
            );

            // ensure the crowdloan account has the correct amount
            let funds_account = pallet_crowdloan::Pallet::<Test>::funds_account(crowdloan_id);
            assert_eq!(Balances::free_balance(funds_account), 350 - 5 * amount);
            // ensure raised amount is updated correctly
            assert!(
                pallet_crowdloan::Crowdloans::<Test>::get(crowdloan_id)
                    .is_some_and(|c| c.raised == 350 - 5 * amount)
            );
            // ensure the event is emitted
            assert_eq!(
                last_event(),
                pallet_crowdloan::Event::<Test>::PartiallyRefunded { crowdloan_id }.into()
            );

            // run some more blocks
            run_to_block(70);

            //  second round of refund
            assert_ok!(Crowdloan::refund(
                RuntimeOrigin::signed(creator),
                crowdloan_id
            ));

            // ensure the contributor count is correct, we processed 1 more refund
            // keeping deposit
            assert!(
                pallet_crowdloan::Crowdloans::<Test>::get(crowdloan_id)
                    .is_some_and(|c| c.contributors_count == 1)
            );

            // ensure the crowdloan account has the correct amount
            assert_eq!(
                pallet_balances::Pallet::<Test>::free_balance(funds_account),
                initial_deposit
            );
            // ensure the raised amount is updated correctly
            assert!(
                pallet_crowdloan::Crowdloans::<Test>::get(crowdloan_id)
                    .is_some_and(|c| c.raised == initial_deposit)
            );

            // ensure creator has the correct amount
            assert_eq!(
                pallet_balances::Pallet::<Test>::free_balance(creator),
                initial_deposit
            );

            // ensure each contributor has been refunded and  removed from the crowdloan
            for i in 2..8 {
                let contributor: AccountOf<Test> = U256::from(i);
                assert_eq!(
                    pallet_balances::Pallet::<Test>::free_balance(contributor),
                    100
                );
                assert_eq!(
                    pallet_crowdloan::Contributions::<Test>::get(crowdloan_id, contributor),
                    None,
                );
            }

            // ensure the event is emitted
            assert_eq!(
                last_event(),
                pallet_crowdloan::Event::<Test>::AllRefunded { crowdloan_id }.into()
            );
        })
}

#[test]
fn test_refund_fails_if_bad_origin() {
    TestState::default().build_and_execute(|| {
        let crowdloan_id: CrowdloanId = 0;

        assert_err!(
            Crowdloan::refund(RuntimeOrigin::none(), crowdloan_id),
            DispatchError::BadOrigin
        );

        assert_err!(
            Crowdloan::refund(RuntimeOrigin::root(), crowdloan_id),
            DispatchError::BadOrigin
        );
    });
}

#[test]
fn test_refund_fails_if_crowdloan_does_not_exist() {
    TestState::default()
        .with_balance(U256::from(1), 100)
        .build_and_execute(|| {
            let creator: AccountOf<Test> = U256::from(1);
            let crowdloan_id: CrowdloanId = 0;

            assert_err!(
                Crowdloan::refund(RuntimeOrigin::signed(creator), crowdloan_id),
                pallet_crowdloan::Error::<Test>::InvalidCrowdloanId
            );
        });
}

#[test]
fn test_refund_fails_if_crowdloan_has_not_ended() {
    TestState::default()
        .with_balance(U256::from(1), 100)
        .build_and_execute(|| {
            // create a crowdloan
            let creator: AccountOf<Test> = U256::from(1);
            let initial_deposit: BalanceOf<Test> = 50;
            let min_contribution: BalanceOf<Test> = 10;
            let cap: BalanceOf<Test> = 300;
            let end: BlockNumberFor<Test> = 50;
            assert_ok!(Crowdloan::create(
                RuntimeOrigin::signed(creator),
                initial_deposit,
                min_contribution,
                cap,
                end,
                Some(noop_call()),
                None,
            ));

            // run some blocks
            run_to_block(10);

            // try to refund
            let crowdloan_id: CrowdloanId = 0;
            assert_err!(
                Crowdloan::refund(RuntimeOrigin::signed(creator), crowdloan_id),
                pallet_crowdloan::Error::<Test>::ContributionPeriodNotEnded
            );
        });
}

#[test]
fn test_dissolve_succeeds() {
    TestState::default()
        .with_balance(U256::from(1), 100)
        .build_and_execute(|| {
            // create a crowdloan
            let creator: AccountOf<Test> = U256::from(1);
            let deposit: BalanceOf<Test> = 50;
            let min_contribution: BalanceOf<Test> = 10;
            let cap: BalanceOf<Test> = 100;
            let end: BlockNumberFor<Test> = 50;

            assert_ok!(Crowdloan::create(
                RuntimeOrigin::signed(creator),
                deposit,
                min_contribution,
                cap,
                end,
                Some(noop_call()),
                None,
            ));

            // run some blocks past end
            run_to_block(60);

            let crowdloan_id: CrowdloanId = 0;

            // ensure the contributor count is correct
            assert!(
                pallet_crowdloan::Crowdloans::<Test>::get(crowdloan_id)
                    .is_some_and(|c| c.contributors_count == 1)
            );

            // dissolve the crowdloan
            assert_ok!(Crowdloan::dissolve(
                RuntimeOrigin::signed(creator),
                crowdloan_id
            ));

            // ensure the crowdloan is removed from the crowdloans map
            assert!(pallet_crowdloan::Crowdloans::<Test>::get(crowdloan_id).is_none());

            // ensure the contributions are removed
            assert!(!pallet_crowdloan::Contributions::<Test>::contains_prefix(
                crowdloan_id
            ));

            // ensure the event is emitted
            assert_eq!(
                last_event(),
                pallet_crowdloan::Event::<Test>::Dissolved { crowdloan_id }.into()
            )
        });
}

#[test]
fn test_dissolve_fails_if_bad_origin() {
    TestState::default().build_and_execute(|| {
        let crowdloan_id: CrowdloanId = 0;

        assert_err!(
            Crowdloan::dissolve(RuntimeOrigin::none(), crowdloan_id),
            DispatchError::BadOrigin
        );

        assert_err!(
            Crowdloan::dissolve(RuntimeOrigin::root(), crowdloan_id),
            DispatchError::BadOrigin
        );
    });
}

#[test]
fn test_dissolve_fails_if_crowdloan_does_not_exist() {
    TestState::default().build_and_execute(|| {
        let crowdloan_id: CrowdloanId = 0;
        assert_err!(
            Crowdloan::dissolve(RuntimeOrigin::signed(U256::from(1)), crowdloan_id),
            pallet_crowdloan::Error::<Test>::InvalidCrowdloanId
        );
    });
}

#[test]
fn test_dissolve_fails_if_crowdloan_has_been_finalized() {
    TestState::default()
        .with_balance(U256::from(1), 100)
        .with_balance(U256::from(2), 100)
        .build_and_execute(|| {
            let creator: AccountOf<Test> = U256::from(1);
            let deposit: BalanceOf<Test> = 50;
            let min_contribution: BalanceOf<Test> = 10;
            let cap: BalanceOf<Test> = 100;
            let end: BlockNumberFor<Test> = 50;

            assert_ok!(Crowdloan::create(
                RuntimeOrigin::signed(creator),
                deposit,
                min_contribution,
                cap,
                end,
                Some(noop_call()),
                None,
            ));

            // run some blocks
            run_to_block(10);

            // some contribution
            let crowdloan_id: CrowdloanId = 0;
            let contributor: AccountOf<Test> = U256::from(2);
            let amount: BalanceOf<Test> = 50;

            assert_ok!(Crowdloan::contribute(
                RuntimeOrigin::signed(contributor),
                crowdloan_id,
                amount
            ));

            // run some more blocks past the end of the contribution period
            run_to_block(60);

            // finalize the crowdloan
            assert_ok!(Crowdloan::finalize(
                RuntimeOrigin::signed(creator),
                crowdloan_id
            ));

            // try dissolve the crowdloan
            assert_err!(
                Crowdloan::dissolve(RuntimeOrigin::signed(creator), crowdloan_id),
                pallet_crowdloan::Error::<Test>::AlreadyFinalized
            );
        });
}

#[test]
fn test_dissolve_fails_if_origin_is_not_creator() {
    TestState::default()
        .with_balance(U256::from(1), 100)
        .build_and_execute(|| {
            let creator: AccountOf<Test> = U256::from(1);
            let deposit: BalanceOf<Test> = 50;
            let min_contribution: BalanceOf<Test> = 10;
            let cap: BalanceOf<Test> = 100;
            let end: BlockNumberFor<Test> = 50;

            assert_ok!(Crowdloan::create(
                RuntimeOrigin::signed(creator),
                deposit,
                min_contribution,
                cap,
                end,
                Some(noop_call()),
                None,
            ));

            // run some blocks
            run_to_block(10);

            // some contribution
            let crowdloan_id: CrowdloanId = 0;

            // try dissolve the crowdloan
            assert_err!(
                Crowdloan::dissolve(RuntimeOrigin::signed(U256::from(2)), crowdloan_id),
                pallet_crowdloan::Error::<Test>::InvalidOrigin
            );
        });
}

#[test]
fn test_dissolve_fails_if_not_everyone_has_been_refunded() {
    TestState::default()
        .with_balance(U256::from(1), 100)
        .with_balance(U256::from(2), 100)
        .build_and_execute(|| {
            // create a crowdloan
            let creator: AccountOf<Test> = U256::from(1);
            let deposit: BalanceOf<Test> = 50;
            let min_contribution: BalanceOf<Test> = 10;
            let cap: BalanceOf<Test> = 100;
            let end: BlockNumberFor<Test> = 50;

            assert_ok!(Crowdloan::create(
                RuntimeOrigin::signed(creator),
                deposit,
                min_contribution,
                cap,
                end,
                Some(noop_call()),
                None,
            ));

            // run some blocks
            run_to_block(10);

            // some contribution
            let crowdloan_id: CrowdloanId = 0;
            let contributor: AccountOf<Test> = U256::from(2);
            let amount: BalanceOf<Test> = 50;
            assert_ok!(Crowdloan::contribute(
                RuntimeOrigin::signed(contributor),
                crowdloan_id,
                amount
            ));

            // run some blocks
            run_to_block(10);

            // try to dissolve the crowdloan
            let crowdloan_id = 0;
            assert_err!(
                Crowdloan::dissolve(RuntimeOrigin::signed(creator), crowdloan_id),
                pallet_crowdloan::Error::<Test>::NotReadyToDissolve
            );
        });
}

#[test]
fn test_update_min_contribution_succeeds() {
    TestState::default()
        .with_balance(U256::from(1), 100)
        .build_and_execute(|| {
            // create a crowdloan
            let creator: AccountOf<Test> = U256::from(1);
            let deposit: BalanceOf<Test> = 50;
            let min_contribution: BalanceOf<Test> = 10;
            let cap: BalanceOf<Test> = 100;
            let end: BlockNumberFor<Test> = 50;

            assert_ok!(Crowdloan::create(
                RuntimeOrigin::signed(creator),
                deposit,
                min_contribution,
                cap,
                end,
                Some(noop_call()),
                None,
            ));

            let crowdloan_id: CrowdloanId = 0;
            let new_min_contribution: BalanceOf<Test> = 20;

            // update the min contribution
            assert_ok!(Crowdloan::update_min_contribution(
                RuntimeOrigin::signed(creator),
                crowdloan_id,
                new_min_contribution
            ));

            // ensure the min contribution is updated
            assert!(
                pallet_crowdloan::Crowdloans::<Test>::get(crowdloan_id)
                    .is_some_and(|c| c.min_contribution == new_min_contribution)
            );
            // ensure the event is emitted
            assert_eq!(
                last_event(),
                pallet_crowdloan::Event::<Test>::MinContributionUpdated {
                    crowdloan_id,
                    new_min_contribution
                }
                .into()
            );
        });
}

#[test]
fn test_update_min_contribution_fails_if_bad_origin() {
    TestState::default().build_and_execute(|| {
        let crowdloan_id: CrowdloanId = 0;

        assert_err!(
            Crowdloan::update_min_contribution(RuntimeOrigin::none(), crowdloan_id, 20),
            DispatchError::BadOrigin
        );

        assert_err!(
            Crowdloan::update_min_contribution(RuntimeOrigin::root(), crowdloan_id, 20),
            DispatchError::BadOrigin
        );
    });
}

#[test]
fn test_update_min_contribution_fails_if_crowdloan_does_not_exist() {
    TestState::default().build_and_execute(|| {
        let crowdloan_id: CrowdloanId = 0;

        assert_err!(
            Crowdloan::update_min_contribution(
                RuntimeOrigin::signed(U256::from(1)),
                crowdloan_id,
                20
            ),
            pallet_crowdloan::Error::<Test>::InvalidCrowdloanId
        );
    });
}

#[test]
fn test_update_min_contribution_fails_if_crowdloan_has_been_finalized() {
    TestState::default()
        .with_balance(U256::from(1), 100)
        .with_balance(U256::from(2), 100)
        .build_and_execute(|| {
            let creator: AccountOf<Test> = U256::from(1);
            let deposit: BalanceOf<Test> = 50;
            let min_contribution: BalanceOf<Test> = 10;
            let cap: BalanceOf<Test> = 100;
            let end: BlockNumberFor<Test> = 50;

            assert_ok!(Crowdloan::create(
                RuntimeOrigin::signed(creator),
                deposit,
                min_contribution,
                cap,
                end,
                Some(noop_call()),
                None,
            ));

            // some contribution
            let crowdloan_id: CrowdloanId = 0;
            let contributor: AccountOf<Test> = U256::from(2);
            let amount: BalanceOf<Test> = 50;
            assert_ok!(Crowdloan::contribute(
                RuntimeOrigin::signed(contributor),
                crowdloan_id,
                amount
            ));

            // run some blocks
            run_to_block(50);

            // finalize the crowdloan
            let crowdloan_id: CrowdloanId = 0;
            assert_ok!(Crowdloan::finalize(
                RuntimeOrigin::signed(creator),
                crowdloan_id
            ));

            // try update the min contribution
            let new_min_contribution: BalanceOf<Test> = 20;
            assert_err!(
                Crowdloan::update_min_contribution(
                    RuntimeOrigin::signed(creator),
                    crowdloan_id,
                    new_min_contribution
                ),
                pallet_crowdloan::Error::<Test>::AlreadyFinalized
            );
        });
}

#[test]
fn test_update_min_contribution_fails_if_not_creator() {
    TestState::default()
        .with_balance(U256::from(1), 100)
        .with_balance(U256::from(2), 100)
        .build_and_execute(|| {
            let creator: AccountOf<Test> = U256::from(1);
            let deposit: BalanceOf<Test> = 50;
            let min_contribution: BalanceOf<Test> = 10;
            let cap: BalanceOf<Test> = 100;
            let end: BlockNumberFor<Test> = 50;

            assert_ok!(Crowdloan::create(
                RuntimeOrigin::signed(creator),
                deposit,
                min_contribution,
                cap,
                end,
                Some(noop_call()),
                None,
            ));

            let crowdloan_id: CrowdloanId = 0;
            let new_min_contribution: BalanceOf<Test> = 20;

            // try update the min contribution
            assert_err!(
                Crowdloan::update_min_contribution(
                    RuntimeOrigin::signed(U256::from(2)),
                    crowdloan_id,
                    new_min_contribution
                ),
                pallet_crowdloan::Error::<Test>::InvalidOrigin
            );
        });
}

#[test]
fn test_update_min_contribution_fails_if_new_min_contribution_is_too_low() {
    TestState::default()
        .with_balance(U256::from(1), 100)
        .build_and_execute(|| {
            let creator: AccountOf<Test> = U256::from(1);
            let deposit: BalanceOf<Test> = 50;
            let min_contribution: BalanceOf<Test> = 10;
            let cap: BalanceOf<Test> = 100;
            let end: BlockNumberFor<Test> = 50;

            assert_ok!(Crowdloan::create(
                RuntimeOrigin::signed(creator),
                deposit,
                min_contribution,
                cap,
                end,
                Some(noop_call()),
                None,
            ));

            let crowdloan_id: CrowdloanId = 0;
            let new_min_contribution: BalanceOf<Test> = 9;

            // try update the min contribution
            assert_err!(
                Crowdloan::update_min_contribution(
                    RuntimeOrigin::signed(creator),
                    crowdloan_id,
                    new_min_contribution
                ),
                pallet_crowdloan::Error::<Test>::MinimumContributionTooLow
            );
        });
}

#[test]
fn test_update_end_succeeds() {
    TestState::default()
        .with_balance(U256::from(1), 100)
        .build_and_execute(|| {
            let creator: AccountOf<Test> = U256::from(1);
            let deposit: BalanceOf<Test> = 50;
            let min_contribution: BalanceOf<Test> = 10;
            let cap: BalanceOf<Test> = 100;
            let end: BlockNumberFor<Test> = 50;

            assert_ok!(Crowdloan::create(
                RuntimeOrigin::signed(creator),
                deposit,
                min_contribution,
                cap,
                end,
                Some(noop_call()),
                None,
            ));

            let crowdloan_id: CrowdloanId = 0;
            let new_end: BlockNumberFor<Test> = 60;

            // update the end
            assert_ok!(Crowdloan::update_end(
                RuntimeOrigin::signed(creator),
                crowdloan_id,
                new_end
            ));

            // ensure the end is updated
            assert!(
                pallet_crowdloan::Crowdloans::<Test>::get(crowdloan_id)
                    .is_some_and(|c| c.end == new_end)
            );
            // ensure the event is emitted
            assert_eq!(
                last_event(),
                pallet_crowdloan::Event::<Test>::EndUpdated {
                    crowdloan_id,
                    new_end
                }
                .into()
            );
        });
}

#[test]
fn test_update_end_fails_if_bad_origin() {
    TestState::default().build_and_execute(|| {
        let crowdloan_id: CrowdloanId = 0;

        assert_err!(
            Crowdloan::update_end(RuntimeOrigin::none(), crowdloan_id, 60),
            DispatchError::BadOrigin
        );

        assert_err!(
            Crowdloan::update_end(RuntimeOrigin::root(), crowdloan_id, 60),
            DispatchError::BadOrigin
        );
    });
}

#[test]
fn test_update_end_fails_if_crowdloan_does_not_exist() {
    TestState::default().build_and_execute(|| {
        let crowdloan_id: CrowdloanId = 0;

        assert_err!(
            Crowdloan::update_end(RuntimeOrigin::signed(U256::from(1)), crowdloan_id, 60),
            pallet_crowdloan::Error::<Test>::InvalidCrowdloanId
        );
    });
}

#[test]
fn test_update_end_fails_if_crowdloan_has_been_finalized() {
    TestState::default()
        .with_balance(U256::from(1), 100)
        .with_balance(U256::from(2), 100)
        .build_and_execute(|| {
            let creator: AccountOf<Test> = U256::from(1);
            let deposit: BalanceOf<Test> = 50;
            let min_contribution: BalanceOf<Test> = 10;
            let cap: BalanceOf<Test> = 100;
            let end: BlockNumberFor<Test> = 50;

            assert_ok!(Crowdloan::create(
                RuntimeOrigin::signed(creator),
                deposit,
                min_contribution,
                cap,
                end,
                Some(noop_call()),
                None,
            ));

            let crowdloan_id: CrowdloanId = 0;

            // some contribution
            let contributor: AccountOf<Test> = U256::from(2);
            let amount: BalanceOf<Test> = 50;
            assert_ok!(Crowdloan::contribute(
                RuntimeOrigin::signed(contributor),
                crowdloan_id,
                amount
            ));

            // run some blocks
            run_to_block(60);

            // finalize the crowdloan
            assert_ok!(Crowdloan::finalize(
                RuntimeOrigin::signed(creator),
                crowdloan_id
            ));

            // try update the end
            let new_end: BlockNumberFor<Test> = 60;
            assert_err!(
                Crowdloan::update_end(RuntimeOrigin::signed(creator), crowdloan_id, new_end),
                pallet_crowdloan::Error::<Test>::AlreadyFinalized
            );
        });
}

#[test]
fn test_update_end_fails_if_not_creator() {
    TestState::default()
        .with_balance(U256::from(1), 100)
        .with_balance(U256::from(2), 100)
        .build_and_execute(|| {
            let creator: AccountOf<Test> = U256::from(1);
            let deposit: BalanceOf<Test> = 50;
            let min_contribution: BalanceOf<Test> = 10;
            let cap: BalanceOf<Test> = 100;
            let end: BlockNumberFor<Test> = 50;

            assert_ok!(Crowdloan::create(
                RuntimeOrigin::signed(creator),
                deposit,
                min_contribution,
                cap,
                end,
                Some(noop_call()),
                None,
            ));

            let crowdloan_id: CrowdloanId = 0;
            let new_end: BlockNumberFor<Test> = 60;

            // try update the end
            assert_err!(
                Crowdloan::update_end(RuntimeOrigin::signed(U256::from(2)), crowdloan_id, new_end),
                pallet_crowdloan::Error::<Test>::InvalidOrigin
            );
        });
}

#[test]
fn test_update_end_fails_if_new_end_is_in_past() {
    TestState::default()
        .with_block_number(50)
        .with_balance(U256::from(1), 100)
        .build_and_execute(|| {
            let creator: AccountOf<Test> = U256::from(1);
            let deposit: BalanceOf<Test> = 50;
            let min_contribution: BalanceOf<Test> = 10;
            let cap: BalanceOf<Test> = 100;
            let end: BlockNumberFor<Test> = 100;

            assert_ok!(Crowdloan::create(
                RuntimeOrigin::signed(creator),
                deposit,
                min_contribution,
                cap,
                end,
                Some(noop_call()),
                None,
            ));

            let crowdloan_id: CrowdloanId = 0;
            let new_end: BlockNumberFor<Test> = 40;

            // try update the end to a past block number
            assert_err!(
                Crowdloan::update_end(RuntimeOrigin::signed(creator), crowdloan_id, new_end),
                pallet_crowdloan::Error::<Test>::CannotEndInPast
            );
        });
}

#[test]
fn test_update_end_fails_if_block_duration_is_too_short() {
    TestState::default()
        .with_balance(U256::from(1), 100)
        .build_and_execute(|| {
            let creator: AccountOf<Test> = U256::from(1);
            let deposit: BalanceOf<Test> = 50;
            let min_contribution: BalanceOf<Test> = 10;
            let cap: BalanceOf<Test> = 100;
            let end: BlockNumberFor<Test> = 50;

            assert_ok!(Crowdloan::create(
                RuntimeOrigin::signed(creator),
                deposit,
                min_contribution,
                cap,
                end,
                Some(noop_call()),
                None,
            ));

            // run some blocks
            run_to_block(50);

            let crowdloan_id: CrowdloanId = 0;
            let new_end: BlockNumberFor<Test> = 51;

            // try update the end to a block number that is too long
            assert_err!(
                Crowdloan::update_end(RuntimeOrigin::signed(creator), crowdloan_id, new_end),
                pallet_crowdloan::Error::<Test>::BlockDurationTooShort
            );
        });
}

#[test]
fn test_update_end_fails_if_block_duration_is_too_long() {
    TestState::default()
        .with_balance(U256::from(1), 100)
        .build_and_execute(|| {
            let creator: AccountOf<Test> = U256::from(1);
            let deposit: BalanceOf<Test> = 50;
            let min_contribution: BalanceOf<Test> = 10;
            let cap: BalanceOf<Test> = 100;
            let end: BlockNumberFor<Test> = 50;

            assert_ok!(Crowdloan::create(
                RuntimeOrigin::signed(creator),
                deposit,
                min_contribution,
                cap,
                end,
                Some(noop_call()),
                None,
            ));

            let crowdloan_id: CrowdloanId = 0;
            let new_end: BlockNumberFor<Test> = 1000;

            // try update the end to a block number that is too long
            assert_err!(
                Crowdloan::update_end(RuntimeOrigin::signed(creator), crowdloan_id, new_end),
                pallet_crowdloan::Error::<Test>::BlockDurationTooLong
            );
        });
}

#[test]
fn test_update_cap_succeeds() {
    TestState::default()
        .with_balance(U256::from(1), 100)
        .build_and_execute(|| {
            let creator: AccountOf<Test> = U256::from(1);
            let deposit: BalanceOf<Test> = 50;
            let min_contribution: BalanceOf<Test> = 10;
            let cap: BalanceOf<Test> = 100;
            let end: BlockNumberFor<Test> = 50;

            assert_ok!(Crowdloan::create(
                RuntimeOrigin::signed(creator),
                deposit,
                min_contribution,
                cap,
                end,
                Some(noop_call()),
                None,
            ));

            // try update the cap
            let crowdloan_id: CrowdloanId = 0;
            let new_cap: BalanceOf<Test> = 200;
            assert_ok!(Crowdloan::update_cap(
                RuntimeOrigin::signed(creator),
                crowdloan_id,
                new_cap
            ));

            // ensure the cap is updated
            assert!(
                pallet_crowdloan::Crowdloans::<Test>::get(crowdloan_id)
                    .is_some_and(|c| c.cap == new_cap)
            );
            // ensure the event is emitted
            assert_eq!(
                last_event(),
                pallet_crowdloan::Event::<Test>::CapUpdated {
                    crowdloan_id,
                    new_cap
                }
                .into()
            );
        });
}

#[test]
fn test_update_cap_fails_if_bad_origin() {
    TestState::default().build_and_execute(|| {
        let crowdloan_id: CrowdloanId = 0;

        assert_err!(
            Crowdloan::update_cap(RuntimeOrigin::none(), crowdloan_id, 200),
            DispatchError::BadOrigin
        );

        assert_err!(
            Crowdloan::update_cap(RuntimeOrigin::root(), crowdloan_id, 200),
            DispatchError::BadOrigin
        );
    });
}

#[test]
fn test_update_cap_fails_if_crowdloan_does_not_exist() {
    TestState::default().build_and_execute(|| {
        let crowdloan_id: CrowdloanId = 0;

        assert_err!(
            Crowdloan::update_cap(RuntimeOrigin::signed(U256::from(1)), crowdloan_id, 200),
            pallet_crowdloan::Error::<Test>::InvalidCrowdloanId
        );
    });
}

#[test]
fn test_update_cap_fails_if_crowdloan_has_been_finalized() {
    TestState::default()
        .with_balance(U256::from(1), 100)
        .with_balance(U256::from(2), 100)
        .build_and_execute(|| {
            let creator: AccountOf<Test> = U256::from(1);
            let deposit: BalanceOf<Test> = 50;
            let min_contribution: BalanceOf<Test> = 10;
            let cap: BalanceOf<Test> = 100;
            let end: BlockNumberFor<Test> = 50;

            assert_ok!(Crowdloan::create(
                RuntimeOrigin::signed(creator),
                deposit,
                min_contribution,
                cap,
                end,
                Some(noop_call()),
                None,
            ));

            // some contribution
            let crowdloan_id: CrowdloanId = 0;
            let contributor: AccountOf<Test> = U256::from(2);
            let amount: BalanceOf<Test> = 50;
            assert_ok!(Crowdloan::contribute(
                RuntimeOrigin::signed(contributor),
                crowdloan_id,
                amount
            ));

            // run some blocks
            run_to_block(60);

            // finalize the crowdloan
            let crowdloan_id: CrowdloanId = 0;
            assert_ok!(Crowdloan::finalize(
                RuntimeOrigin::signed(creator),
                crowdloan_id
            ));

            // try update the cap
            let new_cap: BalanceOf<Test> = 200;
            assert_err!(
                Crowdloan::update_cap(RuntimeOrigin::signed(creator), crowdloan_id, new_cap),
                pallet_crowdloan::Error::<Test>::AlreadyFinalized
            );
        });
}

#[test]
fn test_update_cap_fails_if_not_creator() {
    TestState::default()
        .with_balance(U256::from(1), 100)
        .with_balance(U256::from(2), 100)
        .build_and_execute(|| {
            let creator: AccountOf<Test> = U256::from(1);
            let deposit: BalanceOf<Test> = 50;
            let min_contribution: BalanceOf<Test> = 10;
            let cap: BalanceOf<Test> = 100;
            let end: BlockNumberFor<Test> = 50;

            assert_ok!(Crowdloan::create(
                RuntimeOrigin::signed(creator),
                deposit,
                min_contribution,
                cap,
                end,
                Some(noop_call()),
                None,
            ));

            // try update the cap
            let crowdloan_id: CrowdloanId = 0;
            let new_cap: BalanceOf<Test> = 200;
            assert_err!(
                Crowdloan::update_cap(RuntimeOrigin::signed(U256::from(2)), crowdloan_id, new_cap),
                pallet_crowdloan::Error::<Test>::InvalidOrigin
            );
        });
}

#[test]
fn test_update_cap_fails_if_new_cap_is_too_low() {
    TestState::default()
        .with_balance(U256::from(1), 100)
        .build_and_execute(|| {
            let creator: AccountOf<Test> = U256::from(1);
            let deposit: BalanceOf<Test> = 50;
            let min_contribution: BalanceOf<Test> = 10;
            let cap: BalanceOf<Test> = 100;
            let end: BlockNumberFor<Test> = 50;

            assert_ok!(Crowdloan::create(
                RuntimeOrigin::signed(creator),
                deposit,
                min_contribution,
                cap,
                end,
                Some(noop_call()),
                None,
            ));

            // try update the cap
            let crowdloan_id: CrowdloanId = 0;
            let new_cap: BalanceOf<Test> = 49;
            assert_err!(
                Crowdloan::update_cap(RuntimeOrigin::signed(creator), crowdloan_id, new_cap),
                pallet_crowdloan::Error::<Test>::CapTooLow
            );
        });
}
