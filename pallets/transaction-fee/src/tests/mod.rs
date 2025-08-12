#![allow(clippy::indexing_slicing, clippy::unwrap_used)]
use crate::TransactionSource;
use frame_support::assert_ok;
use frame_support::dispatch::GetDispatchInfo;
use pallet_subtensor_swap::AlphaSqrtPrice;
use sp_runtime::{
    traits::{DispatchTransaction, TransactionExtension, TxBaseImplication},
    transaction_validity::{InvalidTransaction, TransactionValidityError},
};
use substrate_fixed::types::U64F64;
use subtensor_runtime_common::AlphaCurrency;

use mock::*;
mod mock;

// cargo test --package subtensor-transaction-fee --lib -- tests::test_remove_stake_fees_tao --exact --show-output
#[test]
fn test_remove_stake_fees_tao() {
    new_test_ext().execute_with(|| {
        let stake_amount = TAO;
        let unstake_amount = AlphaCurrency::from(TAO / 50);
        let sn = setup_subnets(1, 1);
        setup_stake(
            sn.subnets[0].netuid,
            &sn.coldkey,
            &sn.hotkeys[0],
            stake_amount,
        );
        SubtensorModule::add_balance_to_coldkey_account(&sn.coldkey, TAO);

        // Simulate stake removal to get how much TAO should we get for unstaked Alpha
        let (expected_unstaked_tao, _swap_fee) =
            mock::swap_alpha_to_tao(sn.subnets[0].netuid, unstake_amount);

        // Remove stake
        let balance_before = Balances::free_balance(sn.coldkey);
        let alpha_before = SubtensorModule::get_stake_for_hotkey_and_coldkey_on_subnet(
            &sn.hotkeys[0],
            &sn.coldkey,
            sn.subnets[0].netuid,
        );
        let call = RuntimeCall::SubtensorModule(pallet_subtensor::Call::remove_stake {
            hotkey: sn.hotkeys[0],
            netuid: sn.subnets[0].netuid,
            amount_unstaked: unstake_amount,
        });

        // Dispatch the extrinsic with ChargeTransactionPayment extension
        let info = call.get_dispatch_info();
        let ext = pallet_transaction_payment::ChargeTransactionPayment::<Test>::from(0);
        assert_ok!(ext.dispatch_transaction(
            RuntimeOrigin::signed(sn.coldkey).into(),
            call,
            &info,
            0,
            0,
        ));

        let final_balance = Balances::free_balance(sn.coldkey);
        let alpha_after = SubtensorModule::get_stake_for_hotkey_and_coldkey_on_subnet(
            &sn.hotkeys[0],
            &sn.coldkey,
            sn.subnets[0].netuid,
        );

        let actual_tao_fee = balance_before + expected_unstaked_tao - final_balance;
        let actual_alpha_fee = alpha_before - alpha_after - unstake_amount;

        // Remove stake extrinsic should pay fees in TAO because ck has sufficient TAO balance
        assert!(actual_tao_fee > 0);
        assert_eq!(actual_alpha_fee, AlphaCurrency::from(0));
    });
}

// cargo test --package subtensor-transaction-fee --lib -- tests::test_remove_stake_fees_alpha --exact --show-output
#[test]
fn test_remove_stake_fees_alpha() {
    new_test_ext().execute_with(|| {
        let stake_amount = TAO;
        let unstake_amount = AlphaCurrency::from(TAO / 50);
        let sn = setup_subnets(1, 1);
        setup_stake(
            sn.subnets[0].netuid,
            &sn.coldkey,
            &sn.hotkeys[0],
            stake_amount,
        );

        // Simulate stake removal to get how much TAO should we get for unstaked Alpha
        let (expected_unstaked_tao, _swap_fee) =
            mock::swap_alpha_to_tao(sn.subnets[0].netuid, unstake_amount);

        // Forse-set signer balance to ED
        let current_balance = Balances::free_balance(sn.coldkey);
        let _ = SubtensorModule::remove_balance_from_coldkey_account(
            &sn.coldkey,
            current_balance - ExistentialDeposit::get(),
        );

        // Remove stake
        let balance_before = Balances::free_balance(sn.coldkey);
        let alpha_before = SubtensorModule::get_stake_for_hotkey_and_coldkey_on_subnet(
            &sn.hotkeys[0],
            &sn.coldkey,
            sn.subnets[0].netuid,
        );
        let call = RuntimeCall::SubtensorModule(pallet_subtensor::Call::remove_stake {
            hotkey: sn.hotkeys[0],
            netuid: sn.subnets[0].netuid,
            amount_unstaked: unstake_amount,
        });

        // Dispatch the extrinsic with ChargeTransactionPayment extension
        let info = call.get_dispatch_info();
        let ext = pallet_transaction_payment::ChargeTransactionPayment::<Test>::from(0);
        assert_ok!(ext.dispatch_transaction(
            RuntimeOrigin::signed(sn.coldkey).into(),
            call,
            &info,
            0,
            0,
        ));

        let final_balance = Balances::free_balance(sn.coldkey);
        let alpha_after = SubtensorModule::get_stake_for_hotkey_and_coldkey_on_subnet(
            &sn.hotkeys[0],
            &sn.coldkey,
            sn.subnets[0].netuid,
        );

        let actual_tao_fee = balance_before + expected_unstaked_tao - final_balance;
        let actual_alpha_fee = alpha_before - alpha_after - unstake_amount;

        // Remove stake extrinsic should pay fees in Alpha
        assert_eq!(actual_tao_fee, 0);
        assert!(actual_alpha_fee > 0.into());
    });
}

// Test that unstaking on root with no free balance results in charging fees from
// staked amount
//
// cargo test --package subtensor-transaction-fee --lib -- tests::test_remove_stake_root --exact --show-output
#[test]
fn test_remove_stake_root() {
    new_test_ext().execute_with(|| {
        let stake_amount = TAO;
        let unstake_amount = TAO / 10;
        let netuid = NetUid::from(0);
        let coldkey = U256::from(100000);
        let hotkey = U256::from(100001);

        // Root stake
        add_network(netuid, 10);
        pallet_subtensor::Owner::<Test>::insert(hotkey, coldkey);
        pallet_subtensor::SubtokenEnabled::<Test>::insert(NetUid::from(0), true);
        setup_stake(netuid, &coldkey, &hotkey, stake_amount);

        // Forse-set signer balance to ED
        let current_balance = Balances::free_balance(coldkey);
        let _ = SubtensorModule::remove_balance_from_coldkey_account(
            &coldkey,
            current_balance - ExistentialDeposit::get(),
        );

        // Remove stake
        let balance_before = Balances::free_balance(coldkey);
        let call = RuntimeCall::SubtensorModule(pallet_subtensor::Call::remove_stake {
            hotkey,
            netuid,
            amount_unstaked: unstake_amount.into(),
        });

        // Dispatch the extrinsic with ChargeTransactionPayment extension
        let info = call.get_dispatch_info();
        let ext = pallet_transaction_payment::ChargeTransactionPayment::<Test>::from(0);
        assert_ok!(ext.dispatch_transaction(
            RuntimeOrigin::signed(coldkey).into(),
            call,
            &info,
            0,
            0,
        ));

        let final_balance = Balances::free_balance(coldkey);
        let alpha_after =
            SubtensorModule::get_stake_for_hotkey_and_coldkey_on_subnet(&hotkey, &coldkey, netuid);

        let actual_tao_fee = balance_before + unstake_amount - final_balance;
        let actual_alpha_fee =
            AlphaCurrency::from(stake_amount) - alpha_after - unstake_amount.into();

        // Remove stake extrinsic should pay fees in Alpha (withdrawn from staked TAO)
        assert_eq!(actual_tao_fee, 0);
        assert!(actual_alpha_fee > 0.into());
    });
}

// Test that unstaking 100% of stake on root is possible with no free balance
//
// cargo test --package subtensor-transaction-fee --lib -- tests::test_remove_stake_completely_root --exact --show-output
#[test]
fn test_remove_stake_completely_root() {
    new_test_ext().execute_with(|| {
        let stake_amount = TAO;
        let unstake_amount = TAO;
        let netuid = NetUid::from(0);
        let coldkey = U256::from(100000);
        let hotkey = U256::from(100001);

        // Root stake
        add_network(netuid, 10);
        pallet_subtensor::Owner::<Test>::insert(hotkey, coldkey);
        pallet_subtensor::SubtokenEnabled::<Test>::insert(NetUid::from(0), true);
        setup_stake(netuid, &coldkey, &hotkey, stake_amount);

        // Forse-set signer balance to ED
        let current_balance = Balances::free_balance(coldkey);
        let _ = SubtensorModule::remove_balance_from_coldkey_account(
            &coldkey,
            current_balance - ExistentialDeposit::get(),
        );

        // Remove stake
        let balance_before = Balances::free_balance(coldkey);
        let call = RuntimeCall::SubtensorModule(pallet_subtensor::Call::remove_stake {
            hotkey,
            netuid,
            amount_unstaked: unstake_amount.into(),
        });

        // Dispatch the extrinsic with ChargeTransactionPayment extension
        let info = call.get_dispatch_info();
        let ext = pallet_transaction_payment::ChargeTransactionPayment::<Test>::from(0);
        assert_ok!(ext.dispatch_transaction(
            RuntimeOrigin::signed(coldkey).into(),
            call,
            &info,
            0,
            0,
        ));

        let final_balance = Balances::free_balance(coldkey);
        let alpha_after =
            SubtensorModule::get_stake_for_hotkey_and_coldkey_on_subnet(&hotkey, &coldkey, netuid);

        assert_eq!(alpha_after, 0.into());
        assert!(final_balance > balance_before);
    });
}

// cargo test --package subtensor-transaction-fee --lib -- tests::test_remove_stake_completely_fees_alpha --exact --show-output
#[test]
fn test_remove_stake_completely_fees_alpha() {
    new_test_ext().execute_with(|| {
        let stake_amount = TAO;
        let sn = setup_subnets(1, 1);
        setup_stake(
            sn.subnets[0].netuid,
            &sn.coldkey,
            &sn.hotkeys[0],
            stake_amount,
        );

        // Simulate stake removal to get how much TAO should we get for unstaked Alpha
        let unstake_amount = SubtensorModule::get_stake_for_hotkey_and_coldkey_on_subnet(
            &sn.hotkeys[0],
            &sn.coldkey,
            sn.subnets[0].netuid,
        );
        let (expected_unstaked_tao, _swap_fee) =
            mock::swap_alpha_to_tao(sn.subnets[0].netuid, unstake_amount);

        // Forse-set signer balance to ED
        let current_balance = Balances::free_balance(sn.coldkey);
        let _ = SubtensorModule::remove_balance_from_coldkey_account(
            &sn.coldkey,
            current_balance - ExistentialDeposit::get(),
        );

        // Remove stake
        let balance_before = Balances::free_balance(sn.coldkey);
        let call = RuntimeCall::SubtensorModule(pallet_subtensor::Call::remove_stake {
            hotkey: sn.hotkeys[0],
            netuid: sn.subnets[0].netuid,
            amount_unstaked: unstake_amount,
        });

        // Dispatch the extrinsic with ChargeTransactionPayment extension
        let info = call.get_dispatch_info();
        let ext = pallet_transaction_payment::ChargeTransactionPayment::<Test>::from(0);
        assert_ok!(ext.dispatch_transaction(
            RuntimeOrigin::signed(sn.coldkey).into(),
            call,
            &info,
            0,
            0,
        ));

        let final_balance = Balances::free_balance(sn.coldkey);
        let alpha_after = SubtensorModule::get_stake_for_hotkey_and_coldkey_on_subnet(
            &sn.hotkeys[0],
            &sn.coldkey,
            sn.subnets[0].netuid,
        );

        // Effectively, the fee is paid in TAO in this case because user receives less TAO,
        // and all Alpha is gone, and it is not measurable in Alpha
        let actual_fee = balance_before + expected_unstaked_tao - final_balance;
        assert_eq!(alpha_after, 0.into());
        assert!(actual_fee > 0);
    });
}

// Validation should fail if both TAO and Alpha balance are lower than tx fees,
// so that transaction is not included in the block
#[test]
fn test_remove_stake_not_enough_balance_for_fees() {
    new_test_ext().execute_with(|| {
        let stake_amount = TAO;
        let sn = setup_subnets(1, 1);
        setup_stake(
            sn.subnets[0].netuid,
            &sn.coldkey,
            &sn.hotkeys[0],
            stake_amount,
        );

        // Simulate stake removal to get how much TAO should we get for unstaked Alpha
        let current_stake = SubtensorModule::get_stake_for_hotkey_and_coldkey_on_subnet(
            &sn.hotkeys[0],
            &sn.coldkey,
            sn.subnets[0].netuid,
        );

        // Forse-set signer balance to ED
        let current_balance = Balances::free_balance(sn.coldkey);
        let _ = SubtensorModule::remove_balance_from_coldkey_account(
            &sn.coldkey,
            current_balance - ExistentialDeposit::get(),
        );

        // For-set Alpha balance to low
        let new_current_stake = AlphaCurrency::from(1_000);
        SubtensorModule::decrease_stake_for_hotkey_and_coldkey_on_subnet(
            &sn.hotkeys[0],
            &sn.coldkey,
            sn.subnets[0].netuid,
            current_stake - new_current_stake,
        );

        // Remove stake
        let call = RuntimeCall::SubtensorModule(pallet_subtensor::Call::remove_stake {
            hotkey: sn.hotkeys[0],
            netuid: sn.subnets[0].netuid,
            amount_unstaked: new_current_stake,
        });

        // Dispatch the extrinsic with ChargeTransactionPayment extension
        let info = call.get_dispatch_info();
        let ext = pallet_transaction_payment::ChargeTransactionPayment::<Test>::from(0);
        let result = ext.validate(
            RuntimeOrigin::signed(sn.coldkey).into(),
            &call.clone(),
            &info,
            10,
            (),
            &TxBaseImplication(()),
            TransactionSource::External,
        );

        assert_eq!(
            result.unwrap_err(),
            TransactionValidityError::Invalid(InvalidTransaction::Payment)
        );
    });
}

// No TAO balance, Alpha fees. If Alpha price is high, it is enough to pay fees, but when Alpha price
// is low, the validation fails
//
// cargo test --package subtensor-transaction-fee --lib -- tests::test_remove_stake_edge_alpha --exact --show-output
#[test]
fn test_remove_stake_edge_alpha() {
    new_test_ext().execute_with(|| {
        let stake_amount = TAO;
        let sn = setup_subnets(1, 1);
        setup_stake(
            sn.subnets[0].netuid,
            &sn.coldkey,
            &sn.hotkeys[0],
            stake_amount,
        );

        // Simulate stake removal to get how much TAO should we get for unstaked Alpha
        let current_stake = SubtensorModule::get_stake_for_hotkey_and_coldkey_on_subnet(
            &sn.hotkeys[0],
            &sn.coldkey,
            sn.subnets[0].netuid,
        );

        // Forse-set signer balance to ED
        let current_balance = Balances::free_balance(sn.coldkey);
        let _ = SubtensorModule::remove_balance_from_coldkey_account(
            &sn.coldkey,
            current_balance - ExistentialDeposit::get(),
        );

        // For-set Alpha balance to low, but enough to pay tx fees at the current Alpha price
        let new_current_stake = AlphaCurrency::from(1_000_000);
        SubtensorModule::decrease_stake_for_hotkey_and_coldkey_on_subnet(
            &sn.hotkeys[0],
            &sn.coldkey,
            sn.subnets[0].netuid,
            current_stake - new_current_stake,
        );

        // Remove stake
        let call = RuntimeCall::SubtensorModule(pallet_subtensor::Call::remove_stake {
            hotkey: sn.hotkeys[0],
            netuid: sn.subnets[0].netuid,
            amount_unstaked: new_current_stake,
        });

        // Dispatch the extrinsic with ChargeTransactionPayment extension
        let info = call.get_dispatch_info();
        let ext = pallet_transaction_payment::ChargeTransactionPayment::<Test>::from(0);
        let result = ext.validate(
            RuntimeOrigin::signed(sn.coldkey).into(),
            &call.clone(),
            &info,
            10,
            (),
            &TxBaseImplication(()),
            TransactionSource::External,
        );

        // Ok - Validation passed
        assert_ok!(result);

        // Lower Alpha price to 0.0001 so that there is not enough alpha to cover tx fees
        AlphaSqrtPrice::<Test>::insert(sn.subnets[0].netuid, U64F64::from_num(0.01));
        let result_low_alpha_price = ext.validate(
            RuntimeOrigin::signed(sn.coldkey).into(),
            &call.clone(),
            &info,
            10,
            (),
            &TxBaseImplication(()),
            TransactionSource::External,
        );
        assert_eq!(
            result_low_alpha_price.unwrap_err(),
            TransactionValidityError::Invalid(InvalidTransaction::Payment)
        );
    });
}

// Validation passes, but transaction fails => TAO fees are paid
//
// cargo test --package subtensor-transaction-fee --lib -- tests::test_remove_stake_failing_transaction_tao_fees --exact --show-output
#[test]
fn test_remove_stake_failing_transaction_tao_fees() {
    new_test_ext().execute_with(|| {
        let stake_amount = TAO;
        let unstake_amount = AlphaCurrency::from(TAO / 50);
        let sn = setup_subnets(1, 1);
        setup_stake(
            sn.subnets[0].netuid,
            &sn.coldkey,
            &sn.hotkeys[0],
            stake_amount,
        );
        SubtensorModule::add_balance_to_coldkey_account(&sn.coldkey, TAO);

        // Make unstaking fail by reducing liquidity to critical
        SubnetTAO::<Test>::insert(sn.subnets[0].netuid, TaoCurrency::from(1));

        // Remove stake
        let balance_before = Balances::free_balance(sn.coldkey);
        let alpha_before = SubtensorModule::get_stake_for_hotkey_and_coldkey_on_subnet(
            &sn.hotkeys[0],
            &sn.coldkey,
            sn.subnets[0].netuid,
        );
        let call = RuntimeCall::SubtensorModule(pallet_subtensor::Call::remove_stake {
            hotkey: sn.hotkeys[0],
            netuid: sn.subnets[0].netuid,
            amount_unstaked: unstake_amount,
        });

        // Dispatch the extrinsic with ChargeTransactionPayment extension
        let info = call.get_dispatch_info();
        let ext = pallet_transaction_payment::ChargeTransactionPayment::<Test>::from(0);
        assert_ok!(ext.dispatch_transaction(
            RuntimeOrigin::signed(sn.coldkey).into(),
            call,
            &info,
            0,
            0,
        ));

        let final_balance = Balances::free_balance(sn.coldkey);
        let alpha_after = SubtensorModule::get_stake_for_hotkey_and_coldkey_on_subnet(
            &sn.hotkeys[0],
            &sn.coldkey,
            sn.subnets[0].netuid,
        );

        let actual_tao_fee = balance_before - final_balance;

        // Remove stake extrinsic should pay fees in TAO because ck has sufficient TAO balance
        assert!(actual_tao_fee > 0);
        assert_eq!(alpha_before, alpha_after);
    });
}

// Validation passes, but transaction fails => Alpha fees are paid
//
// cargo test --package subtensor-transaction-fee --lib -- tests::test_remove_stake_failing_transaction_alpha_fees --exact --show-output
#[test]
fn test_remove_stake_failing_transaction_alpha_fees() {
    new_test_ext().execute_with(|| {
        let stake_amount = TAO;
        let unstake_amount = AlphaCurrency::from(TAO / 50);
        let sn = setup_subnets(1, 1);
        setup_stake(
            sn.subnets[0].netuid,
            &sn.coldkey,
            &sn.hotkeys[0],
            stake_amount,
        );

        // Make unstaking fail by reducing liquidity to critical
        SubnetTAO::<Test>::insert(sn.subnets[0].netuid, TaoCurrency::from(1));

        // Forse-set signer balance to ED
        let current_balance = Balances::free_balance(sn.coldkey);
        let _ = SubtensorModule::remove_balance_from_coldkey_account(
            &sn.coldkey,
            current_balance - ExistentialDeposit::get(),
        );

        // Remove stake
        let balance_before = Balances::free_balance(sn.coldkey);
        let alpha_before = SubtensorModule::get_stake_for_hotkey_and_coldkey_on_subnet(
            &sn.hotkeys[0],
            &sn.coldkey,
            sn.subnets[0].netuid,
        );
        let call = RuntimeCall::SubtensorModule(pallet_subtensor::Call::remove_stake {
            hotkey: sn.hotkeys[0],
            netuid: sn.subnets[0].netuid,
            amount_unstaked: unstake_amount,
        });

        // Dispatch the extrinsic with ChargeTransactionPayment extension
        let info = call.get_dispatch_info();
        let ext = pallet_transaction_payment::ChargeTransactionPayment::<Test>::from(0);
        assert_ok!(ext.dispatch_transaction(
            RuntimeOrigin::signed(sn.coldkey).into(),
            call,
            &info,
            0,
            0,
        ));

        let final_balance = Balances::free_balance(sn.coldkey);
        let alpha_after = SubtensorModule::get_stake_for_hotkey_and_coldkey_on_subnet(
            &sn.hotkeys[0],
            &sn.coldkey,
            sn.subnets[0].netuid,
        );

        let actual_tao_fee = balance_before - final_balance;
        let actual_alpha_fee = alpha_before - alpha_after;

        // Remove stake extrinsic should pay fees in Alpha
        assert_eq!(actual_tao_fee, 0);
        assert!(actual_alpha_fee > 0.into());
        assert!(actual_alpha_fee < unstake_amount);
    });
}

// cargo test --package subtensor-transaction-fee --lib -- tests::test_remove_stake_limit_fees_alpha --exact --show-output
#[test]
fn test_remove_stake_limit_fees_alpha() {
    new_test_ext().execute_with(|| {
        let stake_amount = TAO;
        let unstake_amount = AlphaCurrency::from(TAO / 50);
        let sn = setup_subnets(1, 1);
        setup_stake(
            sn.subnets[0].netuid,
            &sn.coldkey,
            &sn.hotkeys[0],
            stake_amount,
        );

        // Simulate stake removal to get how much TAO should we get for unstaked Alpha
        let (expected_unstaked_tao, _swap_fee) =
            mock::swap_alpha_to_tao(sn.subnets[0].netuid, unstake_amount);

        // Forse-set signer balance to ED
        let current_balance = Balances::free_balance(sn.coldkey);
        let _ = SubtensorModule::remove_balance_from_coldkey_account(
            &sn.coldkey,
            current_balance - ExistentialDeposit::get(),
        );

        // Remove stake limit
        let balance_before = Balances::free_balance(sn.coldkey);
        let alpha_before = SubtensorModule::get_stake_for_hotkey_and_coldkey_on_subnet(
            &sn.hotkeys[0],
            &sn.coldkey,
            sn.subnets[0].netuid,
        );
        let call = RuntimeCall::SubtensorModule(pallet_subtensor::Call::remove_stake_limit {
            hotkey: sn.hotkeys[0],
            netuid: sn.subnets[0].netuid,
            amount_unstaked: unstake_amount,
            limit_price: 1_000.into(),
            allow_partial: false,
        });

        // Dispatch the extrinsic with ChargeTransactionPayment extension
        let info = call.get_dispatch_info();
        let ext = pallet_transaction_payment::ChargeTransactionPayment::<Test>::from(0);
        assert_ok!(ext.dispatch_transaction(
            RuntimeOrigin::signed(sn.coldkey).into(),
            call,
            &info,
            0,
            0,
        ));

        let final_balance = Balances::free_balance(sn.coldkey);
        let alpha_after = SubtensorModule::get_stake_for_hotkey_and_coldkey_on_subnet(
            &sn.hotkeys[0],
            &sn.coldkey,
            sn.subnets[0].netuid,
        );

        let actual_tao_fee = balance_before + expected_unstaked_tao - final_balance;
        let actual_alpha_fee = alpha_before - alpha_after - unstake_amount;

        // Remove stake extrinsic should pay fees in Alpha
        assert_eq!(actual_tao_fee, 0);
        assert!(actual_alpha_fee > 0.into());
    });
}

// cargo test --package subtensor-transaction-fee --lib -- tests::test_unstake_all_fees_alpha --exact --show-output
#[test]
fn test_unstake_all_fees_alpha() {
    new_test_ext().execute_with(|| {
        let stake_amount = TAO;
        let sn = setup_subnets(10, 1);
        let coldkey = U256::from(100000);
        for i in 0..10 {
            setup_stake(sn.subnets[i].netuid, &coldkey, &sn.hotkeys[0], stake_amount);
        }

        // Root stake
        add_network(NetUid::from(0), 10);
        pallet_subtensor::SubtokenEnabled::<Test>::insert(NetUid::from(0), true);
        setup_stake(0.into(), &coldkey, &sn.hotkeys[0], stake_amount);

        // Simulate stake removal to get how much TAO should we get for unstaked Alpha
        let mut expected_unstaked_tao = 0;
        for i in 0..10 {
            let unstake_amount = SubtensorModule::get_stake_for_hotkey_and_coldkey_on_subnet(
                &sn.hotkeys[0],
                &coldkey,
                sn.subnets[i].netuid,
            );

            let (tao, _swap_fee) = mock::swap_alpha_to_tao(sn.subnets[i].netuid, unstake_amount);
            expected_unstaked_tao += tao;
        }

        // Forse-set signer balance to ED
        let current_balance = Balances::free_balance(coldkey);
        let _ = SubtensorModule::remove_balance_from_coldkey_account(
            &coldkey,
            current_balance - ExistentialDeposit::get(),
        );

        // Unstake all
        let balance_before = Balances::free_balance(sn.coldkey);
        let call = RuntimeCall::SubtensorModule(pallet_subtensor::Call::unstake_all {
            hotkey: sn.hotkeys[0],
        });

        // Dispatch the extrinsic with ChargeTransactionPayment extension
        let info = call.get_dispatch_info();
        let ext = pallet_transaction_payment::ChargeTransactionPayment::<Test>::from(0);
        assert_ok!(ext.dispatch_transaction(
            RuntimeOrigin::signed(coldkey).into(),
            call,
            &info,
            0,
            0,
        ));

        let final_balance = Balances::free_balance(sn.coldkey);

        // Effectively, the fee is paid in TAO in this case because user receives less TAO,
        // and all Alpha is gone, and it is not measurable in Alpha
        let actual_fee = balance_before + expected_unstaked_tao - final_balance;
        assert!(actual_fee > 0);

        // Check that all subnets got unstaked
        for i in 0..10 {
            let alpha_after = SubtensorModule::get_stake_for_hotkey_and_coldkey_on_subnet(
                &sn.hotkeys[0],
                &sn.coldkey,
                sn.subnets[i].netuid,
            );
            assert_eq!(alpha_after, 0.into());
        }
    });
}

// cargo test --package subtensor-transaction-fee --lib -- tests::test_unstake_all_alpha_fees_alpha --exact --show-output
#[test]
fn test_unstake_all_alpha_fees_alpha() {
    new_test_ext().execute_with(|| {
        let stake_amount = TAO;
        let sn = setup_subnets(10, 1);
        let coldkey = U256::from(100000);
        for i in 0..10 {
            setup_stake(sn.subnets[i].netuid, &coldkey, &sn.hotkeys[0], stake_amount);
        }

        // Simulate stake removal to get how much TAO should we get for unstaked Alpha
        let mut expected_unstaked_tao = 0;
        for i in 0..10 {
            let unstake_amount = SubtensorModule::get_stake_for_hotkey_and_coldkey_on_subnet(
                &sn.hotkeys[0],
                &coldkey,
                sn.subnets[i].netuid,
            );

            let (tao, _swap_fee) = mock::swap_alpha_to_tao(sn.subnets[i].netuid, unstake_amount);
            expected_unstaked_tao += tao;
        }

        // Forse-set signer balance to ED
        let current_balance = Balances::free_balance(coldkey);
        let _ = SubtensorModule::remove_balance_from_coldkey_account(
            &coldkey,
            current_balance - ExistentialDeposit::get(),
        );

        // Unstake all
        let balance_before = Balances::free_balance(sn.coldkey);
        let call = RuntimeCall::SubtensorModule(pallet_subtensor::Call::unstake_all_alpha {
            hotkey: sn.hotkeys[0],
        });

        // Dispatch the extrinsic with ChargeTransactionPayment extension
        let info = call.get_dispatch_info();
        let ext = pallet_transaction_payment::ChargeTransactionPayment::<Test>::from(0);
        assert_ok!(ext.dispatch_transaction(
            RuntimeOrigin::signed(coldkey).into(),
            call,
            &info,
            0,
            0,
        ));

        let final_balance = Balances::free_balance(sn.coldkey);

        // Effectively, the fee is paid in TAO in this case because user receives less TAO,
        // and all Alpha is gone, and it is not measurable in Alpha
        let actual_fee = balance_before + expected_unstaked_tao - final_balance;
        assert!(actual_fee > 0);

        // Check that all subnets got unstaked
        for i in 0..10 {
            let alpha_after = SubtensorModule::get_stake_for_hotkey_and_coldkey_on_subnet(
                &sn.hotkeys[0],
                &sn.coldkey,
                sn.subnets[i].netuid,
            );
            assert_eq!(alpha_after, 0.into());
        }
    });
}

// cargo test --package subtensor-transaction-fee --lib -- tests::test_move_stake_fees_alpha --exact --show-output
#[test]
fn test_move_stake_fees_alpha() {
    new_test_ext().execute_with(|| {
        let stake_amount = TAO;
        let unstake_amount = AlphaCurrency::from(TAO / 50);
        let sn = setup_subnets(2, 2);
        setup_stake(
            sn.subnets[0].netuid,
            &sn.coldkey,
            &sn.hotkeys[0],
            stake_amount,
        );

        // Forse-set signer balance to ED
        let current_balance = Balances::free_balance(sn.coldkey);
        let _ = SubtensorModule::remove_balance_from_coldkey_account(
            &sn.coldkey,
            current_balance - ExistentialDeposit::get(),
        );

        // Move stake
        let balance_before = Balances::free_balance(sn.coldkey);
        let alpha_before = SubtensorModule::get_stake_for_hotkey_and_coldkey_on_subnet(
            &sn.hotkeys[0],
            &sn.coldkey,
            sn.subnets[0].netuid,
        );
        let call = RuntimeCall::SubtensorModule(pallet_subtensor::Call::move_stake {
            origin_hotkey: sn.hotkeys[0],
            destination_hotkey: sn.hotkeys[1],
            origin_netuid: sn.subnets[0].netuid,
            destination_netuid: sn.subnets[1].netuid,
            alpha_amount: unstake_amount,
        });

        // Dispatch the extrinsic with ChargeTransactionPayment extension
        let info = call.get_dispatch_info();
        let ext = pallet_transaction_payment::ChargeTransactionPayment::<Test>::from(0);
        assert_ok!(ext.dispatch_transaction(
            RuntimeOrigin::signed(sn.coldkey).into(),
            call,
            &info,
            0,
            0,
        ));

        let final_balance = Balances::free_balance(sn.coldkey);
        let alpha_after_0 = SubtensorModule::get_stake_for_hotkey_and_coldkey_on_subnet(
            &sn.hotkeys[0],
            &sn.coldkey,
            sn.subnets[0].netuid,
        );

        // Ensure stake was moved
        let alpha_after_1 = SubtensorModule::get_stake_for_hotkey_and_coldkey_on_subnet(
            &sn.hotkeys[1],
            &sn.coldkey,
            sn.subnets[1].netuid,
        );
        assert!(alpha_after_1 > 0.into());

        let actual_tao_fee = balance_before - final_balance;
        let actual_alpha_fee = alpha_before - alpha_after_0 - unstake_amount;

        // Extrinsic should pay fees in Alpha
        assert_eq!(actual_tao_fee, 0);
        assert!(actual_alpha_fee > 0.into());
    });
}

// cargo test --package subtensor-transaction-fee --lib -- tests::test_transfer_stake_fees_alpha --exact --show-output
#[test]
fn test_transfer_stake_fees_alpha() {
    new_test_ext().execute_with(|| {
        let destination_coldkey = U256::from(100000);
        let stake_amount = TAO;
        let unstake_amount = AlphaCurrency::from(TAO / 50);
        let sn = setup_subnets(2, 2);
        setup_stake(
            sn.subnets[0].netuid,
            &sn.coldkey,
            &sn.hotkeys[0],
            stake_amount,
        );

        // Forse-set signer balance to ED
        let current_balance = Balances::free_balance(sn.coldkey);
        let _ = SubtensorModule::remove_balance_from_coldkey_account(
            &sn.coldkey,
            current_balance - ExistentialDeposit::get(),
        );

        // Transfer stake
        let balance_before = Balances::free_balance(sn.coldkey);
        let alpha_before = SubtensorModule::get_stake_for_hotkey_and_coldkey_on_subnet(
            &sn.hotkeys[0],
            &sn.coldkey,
            sn.subnets[0].netuid,
        );
        let call = RuntimeCall::SubtensorModule(pallet_subtensor::Call::transfer_stake {
            destination_coldkey,
            hotkey: sn.hotkeys[0],
            origin_netuid: sn.subnets[0].netuid,
            destination_netuid: sn.subnets[1].netuid,
            alpha_amount: unstake_amount,
        });

        // Dispatch the extrinsic with ChargeTransactionPayment extension
        let info = call.get_dispatch_info();
        let ext = pallet_transaction_payment::ChargeTransactionPayment::<Test>::from(0);
        assert_ok!(ext.dispatch_transaction(
            RuntimeOrigin::signed(sn.coldkey).into(),
            call,
            &info,
            0,
            0,
        ));

        let final_balance = Balances::free_balance(sn.coldkey);
        let alpha_after_0 = SubtensorModule::get_stake_for_hotkey_and_coldkey_on_subnet(
            &sn.hotkeys[0],
            &sn.coldkey,
            sn.subnets[0].netuid,
        );

        // Ensure stake was transferred
        let alpha_after_1 = SubtensorModule::get_stake_for_hotkey_and_coldkey_on_subnet(
            &sn.hotkeys[0],
            &destination_coldkey,
            sn.subnets[1].netuid,
        );
        assert!(alpha_after_1 > 0.into());

        let actual_tao_fee = balance_before - final_balance;
        let actual_alpha_fee = alpha_before - alpha_after_0 - unstake_amount;

        // Extrinsic should pay fees in Alpha
        assert_eq!(actual_tao_fee, 0);
        assert!(actual_alpha_fee > 0.into());
    });
}

// cargo test --package subtensor-transaction-fee --lib -- tests::test_swap_stake_fees_alpha --exact --show-output
#[test]
fn test_swap_stake_fees_alpha() {
    new_test_ext().execute_with(|| {
        let stake_amount = TAO;
        let unstake_amount = AlphaCurrency::from(TAO / 50);
        let sn = setup_subnets(2, 2);
        setup_stake(
            sn.subnets[0].netuid,
            &sn.coldkey,
            &sn.hotkeys[0],
            stake_amount,
        );

        // Forse-set signer balance to ED
        let current_balance = Balances::free_balance(sn.coldkey);
        let _ = SubtensorModule::remove_balance_from_coldkey_account(
            &sn.coldkey,
            current_balance - ExistentialDeposit::get(),
        );

        // Swap stake
        let balance_before = Balances::free_balance(sn.coldkey);
        let alpha_before = SubtensorModule::get_stake_for_hotkey_and_coldkey_on_subnet(
            &sn.hotkeys[0],
            &sn.coldkey,
            sn.subnets[0].netuid,
        );
        let call = RuntimeCall::SubtensorModule(pallet_subtensor::Call::swap_stake {
            hotkey: sn.hotkeys[0],
            origin_netuid: sn.subnets[0].netuid,
            destination_netuid: sn.subnets[1].netuid,
            alpha_amount: unstake_amount,
        });

        // Dispatch the extrinsic with ChargeTransactionPayment extension
        let info = call.get_dispatch_info();
        let ext = pallet_transaction_payment::ChargeTransactionPayment::<Test>::from(0);
        assert_ok!(ext.dispatch_transaction(
            RuntimeOrigin::signed(sn.coldkey).into(),
            call,
            &info,
            0,
            0,
        ));

        let final_balance = Balances::free_balance(sn.coldkey);
        let alpha_after_0 = SubtensorModule::get_stake_for_hotkey_and_coldkey_on_subnet(
            &sn.hotkeys[0],
            &sn.coldkey,
            sn.subnets[0].netuid,
        );

        // Ensure stake was transferred
        let alpha_after_1 = SubtensorModule::get_stake_for_hotkey_and_coldkey_on_subnet(
            &sn.hotkeys[0],
            &sn.coldkey,
            sn.subnets[1].netuid,
        );
        assert!(alpha_after_1 > 0.into());

        let actual_tao_fee = balance_before - final_balance;
        let actual_alpha_fee = alpha_before - alpha_after_0 - unstake_amount;

        // Extrinsic should pay fees in Alpha
        assert_eq!(actual_tao_fee, 0);
        assert!(actual_alpha_fee > 0.into());
    });
}

// cargo test --package subtensor-transaction-fee --lib -- tests::test_swap_stake_limit_fees_alpha --exact --show-output
#[test]
fn test_swap_stake_limit_fees_alpha() {
    new_test_ext().execute_with(|| {
        let stake_amount = TAO;
        let unstake_amount = AlphaCurrency::from(TAO / 50);
        let sn = setup_subnets(2, 2);
        setup_stake(
            sn.subnets[0].netuid,
            &sn.coldkey,
            &sn.hotkeys[0],
            stake_amount,
        );

        // Forse-set signer balance to ED
        let current_balance = Balances::free_balance(sn.coldkey);
        let _ = SubtensorModule::remove_balance_from_coldkey_account(
            &sn.coldkey,
            current_balance - ExistentialDeposit::get(),
        );

        // Swap stake limit
        let balance_before = Balances::free_balance(sn.coldkey);
        let alpha_before = SubtensorModule::get_stake_for_hotkey_and_coldkey_on_subnet(
            &sn.hotkeys[0],
            &sn.coldkey,
            sn.subnets[0].netuid,
        );
        let call = RuntimeCall::SubtensorModule(pallet_subtensor::Call::swap_stake_limit {
            hotkey: sn.hotkeys[0],
            origin_netuid: sn.subnets[0].netuid,
            destination_netuid: sn.subnets[1].netuid,
            alpha_amount: unstake_amount,
            limit_price: 1_000.into(),
            allow_partial: false,
        });

        // Dispatch the extrinsic with ChargeTransactionPayment extension
        let info = call.get_dispatch_info();
        let ext = pallet_transaction_payment::ChargeTransactionPayment::<Test>::from(0);
        assert_ok!(ext.dispatch_transaction(
            RuntimeOrigin::signed(sn.coldkey).into(),
            call,
            &info,
            0,
            0,
        ));

        let final_balance = Balances::free_balance(sn.coldkey);
        let alpha_after_0 = SubtensorModule::get_stake_for_hotkey_and_coldkey_on_subnet(
            &sn.hotkeys[0],
            &sn.coldkey,
            sn.subnets[0].netuid,
        );

        // Ensure stake was transferred
        let alpha_after_1 = SubtensorModule::get_stake_for_hotkey_and_coldkey_on_subnet(
            &sn.hotkeys[0],
            &sn.coldkey,
            sn.subnets[1].netuid,
        );
        assert!(alpha_after_1 > 0.into());

        let actual_tao_fee = balance_before - final_balance;
        let actual_alpha_fee = alpha_before - alpha_after_0 - unstake_amount;

        // Extrinsic should pay fees in Alpha
        assert_eq!(actual_tao_fee, 0);
        assert!(actual_alpha_fee > 0.into());
    });
}

// cargo test --package subtensor-transaction-fee --lib -- tests::test_burn_alpha_fees_alpha --exact --show-output
#[test]
fn test_burn_alpha_fees_alpha() {
    new_test_ext().execute_with(|| {
        let stake_amount = TAO;
        let alpha_amount = AlphaCurrency::from(TAO / 50);
        let sn = setup_subnets(1, 1);
        setup_stake(
            sn.subnets[0].netuid,
            &sn.coldkey,
            &sn.hotkeys[0],
            stake_amount,
        );

        // Forse-set signer balance to ED
        let current_balance = Balances::free_balance(sn.coldkey);
        let _ = SubtensorModule::remove_balance_from_coldkey_account(
            &sn.coldkey,
            current_balance - ExistentialDeposit::get(),
        );

        // Burn alpha
        let balance_before = Balances::free_balance(sn.coldkey);
        let alpha_before = SubtensorModule::get_stake_for_hotkey_and_coldkey_on_subnet(
            &sn.hotkeys[0],
            &sn.coldkey,
            sn.subnets[0].netuid,
        );
        let call = RuntimeCall::SubtensorModule(pallet_subtensor::Call::burn_alpha {
            hotkey: sn.hotkeys[0],
            amount: alpha_amount,
            netuid: sn.subnets[0].netuid,
        });

        // Dispatch the extrinsic with ChargeTransactionPayment extension
        let info = call.get_dispatch_info();
        let ext = pallet_transaction_payment::ChargeTransactionPayment::<Test>::from(0);
        assert_ok!(ext.dispatch_transaction(
            RuntimeOrigin::signed(sn.coldkey).into(),
            call,
            &info,
            0,
            0,
        ));

        let final_balance = Balances::free_balance(sn.coldkey);
        let alpha_after_0 = SubtensorModule::get_stake_for_hotkey_and_coldkey_on_subnet(
            &sn.hotkeys[0],
            &sn.coldkey,
            sn.subnets[0].netuid,
        );

        let actual_tao_fee = balance_before - final_balance;
        let actual_alpha_fee = alpha_before - alpha_after_0 - alpha_amount;

        // Extrinsic should pay fees in Alpha
        assert_eq!(actual_tao_fee, 0);
        assert!(actual_alpha_fee > 0.into());
    });
}

// cargo test --package subtensor-transaction-fee --lib -- tests::test_recycle_alpha_fees_alpha --exact --show-output
#[test]
fn test_recycle_alpha_fees_alpha() {
    new_test_ext().execute_with(|| {
        let stake_amount = TAO;
        let alpha_amount = AlphaCurrency::from(TAO / 50);
        let sn = setup_subnets(1, 1);
        setup_stake(
            sn.subnets[0].netuid,
            &sn.coldkey,
            &sn.hotkeys[0],
            stake_amount,
        );

        // Forse-set signer balance to ED
        let current_balance = Balances::free_balance(sn.coldkey);
        let _ = SubtensorModule::remove_balance_from_coldkey_account(
            &sn.coldkey,
            current_balance - ExistentialDeposit::get(),
        );

        // Recycle alpha
        let balance_before = Balances::free_balance(sn.coldkey);
        let alpha_before = SubtensorModule::get_stake_for_hotkey_and_coldkey_on_subnet(
            &sn.hotkeys[0],
            &sn.coldkey,
            sn.subnets[0].netuid,
        );
        let call = RuntimeCall::SubtensorModule(pallet_subtensor::Call::recycle_alpha {
            hotkey: sn.hotkeys[0],
            amount: alpha_amount,
            netuid: sn.subnets[0].netuid,
        });

        // Dispatch the extrinsic with ChargeTransactionPayment extension
        let info = call.get_dispatch_info();
        let ext = pallet_transaction_payment::ChargeTransactionPayment::<Test>::from(0);
        assert_ok!(ext.dispatch_transaction(
            RuntimeOrigin::signed(sn.coldkey).into(),
            call,
            &info,
            0,
            0,
        ));

        let final_balance = Balances::free_balance(sn.coldkey);
        let alpha_after_0 = SubtensorModule::get_stake_for_hotkey_and_coldkey_on_subnet(
            &sn.hotkeys[0],
            &sn.coldkey,
            sn.subnets[0].netuid,
        );

        let actual_tao_fee = balance_before - final_balance;
        let actual_alpha_fee = alpha_before - alpha_after_0 - alpha_amount;

        // Extrinsic should pay fees in Alpha
        assert_eq!(actual_tao_fee, 0);
        assert!(actual_alpha_fee > 0.into());
    });
}
