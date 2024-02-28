// This file is part of Aventus.
// Copyright (C) 2022 Aventus Network Services (UK) Ltd.

// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with this program. If not, see <https://www.gnu.org/licenses/>.

#![cfg(test)]
use crate::mock::{Balances, Call as MockCall, Event, *};
use crate::{self as token_manager, Call, *};
use codec::Encode;
use frame_support::{assert_err, assert_noop, assert_ok};
use hex_literal::hex;
use pallet_transaction_payment::ChargeTransactionPayment;
use sp_core::{sr25519, Pair};
use sp_runtime::{
    traits::{Hash, SignedExtension},
    transaction_validity::InvalidTransaction,
};

type AccountId = <TestRuntime as system::Config>::AccountId;
type Hashing = <TestRuntime as system::Config>::Hashing;

const DEFAULT_AMOUNT: u128 = 1_000_000;
const DEFAULT_NONCE: u64 = 0;
const NON_ZERO_NONCE: u64 = 100;

pub static TX_LEN: usize = 1;

pub fn default_key_pair() -> sr25519::Pair {
    return sr25519::Pair::from_seed(&[70u8; 32]);
}

fn default_sender() -> AccountId {
    return AccountId::decode(&mut default_key_pair().public().to_vec().as_slice()).unwrap();
}

fn default_receiver_account_id() -> AccountId {
    let receiver = H256(hex!(
        "0000000000000000000000000000000000000000000000000000000000000000"
    ));
    return AccountId::decode(&mut receiver.as_bytes()).expect("Valid account id");
}

fn default_relayer() -> AccountId {
    return AccountId::from_raw([10; 32]);
}

fn default_t1_recipient() -> H160 {
    return H160(hex!("2222222222222222222222222222222222222222"));
}

fn pay_gas_and_proxy_call(
    relayer: &AccountId,
    outer_call: &<TestRuntime as frame_system::Config>::Call,
    inner_call: Box<<TestRuntime as Config>::Call>,
) -> DispatchResult {
    // See: /primitives/runtime/src/traits.rs for more details
    <ChargeTransactionPayment<TestRuntime> as SignedExtension>::pre_dispatch(
        ChargeTransactionPayment::from(0), // we do not pay any tip
        relayer,
        outer_call,
        &info_from_weight(1),
        TX_LEN,
    )
    .map_err(|e| <&'static str>::from(e))?;

    return TokenManager::proxy(Origin::signed(*relayer), inner_call);
}

fn pay_gas_and_call_lower_directly(
    sender: &AccountId,
    token_id: <TestRuntime as Config>::TokenId,
    amount: <TestRuntime as Config>::TokenBalance,
    t1_recipient: H160,
    proof: Proof<Signature, AccountId>,
    call: &<TestRuntime as frame_system::Config>::Call,
) -> DispatchResultWithPostInfo {
    <ChargeTransactionPayment<TestRuntime> as SignedExtension>::pre_dispatch(
        ChargeTransactionPayment::from(0),
        sender,
        call,
        &info_from_weight(1),
        TX_LEN,
    )
    .map_err(|e| <&'static str>::from(e))?;

    return TokenManager::signed_lower(
        Origin::signed(*sender),
        proof,
        *sender,
        token_id,
        amount,
        t1_recipient,
    );
}

fn build_proof(
    signer: &AccountId,
    relayer: &AccountId,
    signature: Signature,
) -> Proof<Signature, AccountId> {
    return Proof {
        signer: *signer,
        relayer: *relayer,
        signature: signature,
    };
}

fn setup(sender: &AccountId, nonce: u64) {
    <TokenManager as Store>::Balances::insert((NON_AVT_TOKEN_ID, sender), 2 * DEFAULT_AMOUNT);
    <TokenManager as Store>::Nonces::insert(sender, nonce);
}

fn default_setup() {
    setup(&default_sender(), DEFAULT_NONCE);
}

fn create_proof_for_signed_lower_with_relayer(relayer: &AccountId) -> Proof<Signature, AccountId> {
    return create_proof_for_signed_lower(
        relayer,
        &default_sender(),
        NON_AVT_TOKEN_ID,
        DEFAULT_AMOUNT,
        DEFAULT_NONCE,
        &default_key_pair(),
        default_t1_recipient(),
    );
}

fn create_proof_for_signed_lower_with_nonce(nonce: u64) -> Proof<Signature, AccountId> {
    return create_proof_for_signed_lower(
        &default_relayer(),
        &default_sender(),
        NON_AVT_TOKEN_ID,
        DEFAULT_AMOUNT,
        nonce,
        &default_key_pair(),
        default_t1_recipient(),
    );
}

fn create_default_proof_for_signed_lower() -> Proof<Signature, AccountId> {
    return create_proof_for_signed_lower(
        &default_relayer(),
        &default_sender(),
        NON_AVT_TOKEN_ID,
        DEFAULT_AMOUNT,
        DEFAULT_NONCE,
        &default_key_pair(),
        default_t1_recipient(),
    );
}

fn create_proof_for_signed_lower(
    relayer: &AccountId,
    from: &AccountId,
    token_id: H160,
    amount: u128,
    nonce: u64,
    keys: &sr25519::Pair,
    t1_recipient: H160,
) -> Proof<Signature, AccountId> {
    let context = SIGNED_LOWER_CONTEXT;
    let data_to_sign = (
        context,
        relayer,
        from,
        token_id,
        amount,
        t1_recipient,
        nonce,
    );
    let signature = sign(&keys, &data_to_sign.encode());

    return build_proof(from, relayer, signature);
}

fn check_proxy_lower_default_call_succeed(call: Box<<TestRuntime as Config>::Call>) {
    let call_hash = Hashing::hash_of(&call);

    assert_ok!(TokenManager::proxy(Origin::signed(default_relayer()), call));
    assert_eq!(System::events().len(), 2);
    assert!(System::events().iter().any(|a| a.event
        == Event::token_manager(crate::Event::<TestRuntime>::CallDispatched(
            default_relayer(),
            call_hash
        ))));

    assert!(System::events().iter().any(|a| a.event
        == Event::token_manager(crate::Event::<TestRuntime>::TokenLowered(
            NON_AVT_TOKEN_ID,
            default_sender(),
            default_receiver_account_id(),
            DEFAULT_AMOUNT,
            default_t1_recipient()
        ))));
}

struct Context;

impl Context {
    pub fn default() -> (sr25519::Pair, AccountId, AccountId, AccountId, H160) {
        (
            default_key_pair(),
            default_sender(),
            default_relayer(),
            default_receiver_account_id(),
            default_t1_recipient(),
        )
    }
}

mod proxy_signed_lower {
    use super::*;

    mod succeeds_implies_that {
        use super::*;

        #[test]
        fn lower_amount_is_deducted_from_sender_balance() {
            let mut ext = ExtBuilder::build_default().as_externality();
            ext.execute_with(|| {
                let (_, sender, relayer, _, t1_recipient) = Context::default();

                setup(&sender, NON_ZERO_NONCE);
                assert_eq!(
                    <TokenManager as Store>::Balances::get((NON_AVT_TOKEN_ID, sender)),
                    2 * DEFAULT_AMOUNT
                );

                let proof = create_proof_for_signed_lower_with_nonce(NON_ZERO_NONCE);

                let call = Box::new(MockCall::TokenManager(
                    super::Call::<TestRuntime>::signed_lower(
                        proof,
                        sender,
                        NON_AVT_TOKEN_ID,
                        DEFAULT_AMOUNT,
                        t1_recipient,
                    ),
                ));

                assert_ok!(TokenManager::proxy(Origin::signed(relayer), call.clone()));

                assert_eq!(
                    <TokenManager as Store>::Balances::get((NON_AVT_TOKEN_ID, sender)),
                    DEFAULT_AMOUNT
                );
            });
        }

        #[test]
        fn events_are_emitted() {
            let mut ext = ExtBuilder::build_default().as_externality();
            ext.execute_with(|| {
                let (_, sender, relayer, recipient_account_id, t1_recipient) = Context::default();

                setup(&sender, NON_ZERO_NONCE);
                let proof = create_proof_for_signed_lower_with_nonce(NON_ZERO_NONCE);

                let call = Box::new(MockCall::TokenManager(
                    super::Call::<TestRuntime>::signed_lower(
                        proof,
                        sender,
                        NON_AVT_TOKEN_ID,
                        DEFAULT_AMOUNT,
                        t1_recipient,
                    ),
                ));

                assert_eq!(System::events().len(), 0);
                assert_ok!(TokenManager::proxy(Origin::signed(relayer), call.clone()));

                let call_hash = Hashing::hash_of(&call);
                assert!(System::events().iter().any(|a| a.event
                    == Event::token_manager(crate::Event::<TestRuntime>::CallDispatched(
                        relayer, call_hash
                    ))));

                // In order to validate that the proxied call has been dispatched we need any proof that lower was called.
                // In this case we will check that the Lowered signal was emitted.
                assert!(System::events().iter().any(|a| a.event
                    == Event::token_manager(crate::Event::<TestRuntime>::TokenLowered(
                        NON_AVT_TOKEN_ID,
                        sender,
                        recipient_account_id,
                        DEFAULT_AMOUNT,
                        t1_recipient
                    ))));
            });
        }
    }

    #[test]
    fn succeeds_with_nonce_zero() {
        let mut ext = ExtBuilder::build_default().as_externality();
        ext.execute_with(|| {
            let (_, sender, relayer, recipient_account_id, t1_recipient) = Context::default();

            default_setup();
            let proof = create_default_proof_for_signed_lower();

            let call = Box::new(MockCall::TokenManager(
                super::Call::<TestRuntime>::signed_lower(
                    proof,
                    sender,
                    NON_AVT_TOKEN_ID,
                    DEFAULT_AMOUNT,
                    t1_recipient,
                ),
            ));
            let call_hash = Hashing::hash_of(&call);

            assert_eq!(System::events().len(), 0);
            assert_ok!(TokenManager::proxy(Origin::signed(relayer), call));

            assert_eq!(
                <TokenManager as Store>::Balances::get((NON_AVT_TOKEN_ID, sender)),
                DEFAULT_AMOUNT
            );

            assert!(System::events().iter().any(|a| a.event
                == Event::token_manager(crate::Event::<TestRuntime>::CallDispatched(
                    relayer, call_hash
                ))));
            assert!(System::events().iter().any(|a| a.event
                == Event::token_manager(crate::Event::<TestRuntime>::TokenLowered(
                    NON_AVT_TOKEN_ID,
                    sender,
                    recipient_account_id,
                    DEFAULT_AMOUNT,
                    t1_recipient
                ))));
        });
    }

    mod fails_with {
        use super::*;

        #[test]
        fn mismatching_proof_nonce() {
            let mut ext = ExtBuilder::build_default().as_externality();
            ext.execute_with(|| {
                let (_, sender, relayer, _, t1_recipient) = Context::default();
                let bad_nonces = [0, 99, 101];
                setup(&sender, NON_ZERO_NONCE);

                for bad_nonce in bad_nonces.iter() {
                    let proof = create_proof_for_signed_lower_with_nonce(*bad_nonce);
                    let call = Box::new(MockCall::TokenManager(
                        super::Call::<TestRuntime>::signed_lower(
                            proof,
                            sender,
                            NON_AVT_TOKEN_ID,
                            DEFAULT_AMOUNT,
                            t1_recipient,
                        ),
                    ));

                    assert_err!(
                        TokenManager::proxy(Origin::signed(relayer), call),
                        Error::<TestRuntime>::UnauthorizedSignedLowerTransaction
                    );

                    assert_eq!(System::events().len(), 0);
                }
            });
        }

        #[test]
        fn mismatched_proof_other_amount() {
            let mut ext = ExtBuilder::build_default().as_externality();
            ext.execute_with(|| {
                let (_, sender, relayer, _, t1_recipient) = Context::default();

                let mismatching_amount: u128 = 100;

                default_setup();
                let proof = create_default_proof_for_signed_lower();

                let call = Box::new(MockCall::TokenManager(
                    super::Call::<TestRuntime>::signed_lower(
                        proof.clone(),
                        sender,
                        NON_AVT_TOKEN_ID,
                        mismatching_amount,
                        t1_recipient,
                    ),
                ));

                assert_eq!(System::events().len(), 0);
                assert_err!(
                    TokenManager::proxy(Origin::signed(relayer), call),
                    Error::<TestRuntime>::UnauthorizedSignedLowerTransaction
                );

                // Show that it works with the correct input
                let proof = create_default_proof_for_signed_lower();
                check_proxy_lower_default_call_succeed(Box::new(MockCall::TokenManager(
                    super::Call::<TestRuntime>::signed_lower(
                        proof,
                        sender,
                        NON_AVT_TOKEN_ID,
                        DEFAULT_AMOUNT,
                        t1_recipient,
                    ),
                )));
            });
        }

        #[test]
        fn mismatched_proof_other_keys() {
            let mut ext = ExtBuilder::build_default().as_externality();
            ext.execute_with(|| {
                let (_, sender, relayer, _, t1_recipient) = Context::default();

                let other_sender_keys = sr25519::Pair::from_entropy(&[2u8; 32], None).0;

                default_setup();
                let mismatching_proof = create_proof_for_signed_lower(
                    &relayer,
                    &sender,
                    NON_AVT_TOKEN_ID,
                    DEFAULT_AMOUNT,
                    DEFAULT_NONCE,
                    &other_sender_keys,
                    t1_recipient,
                );

                let call = Box::new(MockCall::TokenManager(
                    super::Call::<TestRuntime>::signed_lower(
                        mismatching_proof,
                        sender,
                        NON_AVT_TOKEN_ID,
                        DEFAULT_AMOUNT,
                        t1_recipient,
                    ),
                ));

                assert_err!(
                    TokenManager::proxy(Origin::signed(relayer), call),
                    Error::<TestRuntime>::UnauthorizedSignedLowerTransaction
                );
                assert_eq!(System::events().len(), 0);

                // Show that it works with the correct input
                let proof = create_default_proof_for_signed_lower();
                check_proxy_lower_default_call_succeed(Box::new(MockCall::TokenManager(
                    super::Call::<TestRuntime>::signed_lower(
                        proof,
                        sender,
                        NON_AVT_TOKEN_ID,
                        DEFAULT_AMOUNT,
                        t1_recipient,
                    ),
                )));
            });
        }

        #[test]
        fn mismatched_proof_other_sender() {
            let mut ext = ExtBuilder::build_default().as_externality();
            ext.execute_with(|| {
                let (sender_keys, sender, relayer, _, t1_recipient) = Context::default();

                let other_sender_account_id = AccountId::from_raw([55; 32]);

                default_setup();
                let mismatching_proof = create_proof_for_signed_lower(
                    &relayer,
                    &other_sender_account_id,
                    NON_AVT_TOKEN_ID,
                    DEFAULT_AMOUNT,
                    DEFAULT_NONCE,
                    &sender_keys,
                    t1_recipient,
                );

                let call = Box::new(MockCall::TokenManager(
                    super::Call::<TestRuntime>::signed_lower(
                        mismatching_proof,
                        sender,
                        NON_AVT_TOKEN_ID,
                        DEFAULT_AMOUNT,
                        t1_recipient,
                    ),
                ));

                assert_err!(
                    TokenManager::proxy(Origin::signed(relayer), call),
                    Error::<TestRuntime>::SenderNotValid
                );
                assert_eq!(System::events().len(), 0);

                // Show that it works with the correct input
                let proof = create_default_proof_for_signed_lower();
                check_proxy_lower_default_call_succeed(Box::new(MockCall::TokenManager(
                    super::Call::<TestRuntime>::signed_lower(
                        proof,
                        sender,
                        NON_AVT_TOKEN_ID,
                        DEFAULT_AMOUNT,
                        t1_recipient,
                    ),
                )));
            });
        }

        #[test]
        fn mismatched_proof_other_relayer() {
            let mut ext = ExtBuilder::build_default().as_externality();
            ext.execute_with(|| {
                let (_, sender, relayer, recipient_account_id, t1_recipient) = Context::default();

                let other_relayer_account_id = recipient_account_id.clone();

                default_setup();
                let mismatching_proof =
                    create_proof_for_signed_lower_with_relayer(&other_relayer_account_id);
                let call = Box::new(MockCall::TokenManager(
                    super::Call::<TestRuntime>::signed_lower(
                        mismatching_proof,
                        sender,
                        NON_AVT_TOKEN_ID,
                        DEFAULT_AMOUNT,
                        t1_recipient,
                    ),
                ));

                assert_err!(
                    TokenManager::proxy(Origin::signed(relayer), call.clone()),
                    Error::<TestRuntime>::UnauthorizedProxyTransaction
                );
                assert_eq!(System::events().len(), 0);

                // Show that it works with the correct input
                let proof = create_default_proof_for_signed_lower();
                check_proxy_lower_default_call_succeed(Box::new(MockCall::TokenManager(
                    super::Call::<TestRuntime>::signed_lower(
                        proof,
                        sender,
                        NON_AVT_TOKEN_ID,
                        DEFAULT_AMOUNT,
                        t1_recipient,
                    ),
                )));
            });
        }

        #[test]
        fn mismatched_proof_other_token_id() {
            let mut ext = ExtBuilder::build_default().as_externality();
            ext.execute_with(|| {
                let (sender_keys, sender, relayer, _, t1_recipient) = Context::default();
                let other_token_id = NON_AVT_TOKEN_ID_2;

                default_setup();
                let mismatching_proof = create_proof_for_signed_lower(
                    &relayer,
                    &sender,
                    other_token_id,
                    DEFAULT_AMOUNT,
                    DEFAULT_NONCE,
                    &sender_keys,
                    t1_recipient,
                );

                let call = Box::new(MockCall::TokenManager(
                    super::Call::<TestRuntime>::signed_lower(
                        mismatching_proof,
                        sender,
                        NON_AVT_TOKEN_ID,
                        DEFAULT_AMOUNT,
                        t1_recipient,
                    ),
                ));

                assert_err!(
                    TokenManager::proxy(Origin::signed(relayer), call),
                    Error::<TestRuntime>::UnauthorizedSignedLowerTransaction
                );
                assert_eq!(System::events().len(), 0);

                // Show that it works with the correct input
                let proof = create_default_proof_for_signed_lower();
                check_proxy_lower_default_call_succeed(Box::new(MockCall::TokenManager(
                    super::Call::<TestRuntime>::signed_lower(
                        proof,
                        sender,
                        NON_AVT_TOKEN_ID,
                        DEFAULT_AMOUNT,
                        t1_recipient,
                    ),
                )));
            });
        }

        #[test]
        fn mismatched_proof_other_context() {
            let mut ext = ExtBuilder::build_default().as_externality();
            ext.execute_with(|| {
                let (sender_keys, sender, relayer, _, t1_recipient) = Context::default();

                let other_context: &'static [u8] = b"authorizati0n for tr4nsfer op3ration";

                default_setup();
                let data_to_sign = (
                    other_context,
                    relayer,
                    sender,
                    NON_AVT_TOKEN_ID,
                    DEFAULT_AMOUNT,
                    DEFAULT_NONCE,
                    t1_recipient,
                );
                let alternative_signature = sign(&sender_keys, &data_to_sign.encode());

                let mismatching_proof = Proof {
                    signer: sender,
                    relayer: relayer,
                    signature: alternative_signature,
                };

                let call = Box::new(MockCall::TokenManager(
                    super::Call::<TestRuntime>::signed_lower(
                        mismatching_proof,
                        sender,
                        NON_AVT_TOKEN_ID,
                        DEFAULT_AMOUNT,
                        t1_recipient,
                    ),
                ));
                assert_err!(
                    TokenManager::proxy(Origin::signed(relayer), call),
                    Error::<TestRuntime>::UnauthorizedSignedLowerTransaction
                );

                assert_eq!(System::events().len(), 0);

                // Show that it works with the correct input
                let proof = create_default_proof_for_signed_lower();
                check_proxy_lower_default_call_succeed(Box::new(MockCall::TokenManager(
                    super::Call::<TestRuntime>::signed_lower(
                        proof,
                        sender,
                        NON_AVT_TOKEN_ID,
                        DEFAULT_AMOUNT,
                        t1_recipient,
                    ),
                )));
            });
        }
    }
}

mod signed_lower {
    use super::*;

    mod succeeds_implies_that {
        use super::*;

        #[test]
        fn lower_amount_is_deducted_from_sender_balance() {
            let mut ext = ExtBuilder::build_default().as_externality();
            ext.execute_with(|| {
                let (_, sender, _, _, t1_recipient) = Context::default();
                setup(&sender, NON_ZERO_NONCE);
                assert_eq!(
                    <TokenManager as Store>::Balances::get((NON_AVT_TOKEN_ID, sender)),
                    2 * DEFAULT_AMOUNT
                );

                let proof = create_proof_for_signed_lower_with_nonce(NON_ZERO_NONCE);

                assert_ok!(TokenManager::signed_lower(
                    Origin::signed(sender),
                    proof,
                    sender,
                    NON_AVT_TOKEN_ID,
                    DEFAULT_AMOUNT,
                    t1_recipient
                ));

                assert_eq!(
                    <TokenManager as Store>::Balances::get((NON_AVT_TOKEN_ID, sender)),
                    DEFAULT_AMOUNT
                );
            });
        }

        #[test]
        fn event_is_emitted() {
            let mut ext = ExtBuilder::build_default().as_externality();
            ext.execute_with(|| {
                let (_, sender, _, recipient_account_id, t1_recipient) = Context::default();
                setup(&sender, NON_ZERO_NONCE);
                let proof = create_proof_for_signed_lower_with_nonce(NON_ZERO_NONCE);

                assert_eq!(System::events().len(), 0);

                assert_ok!(TokenManager::signed_lower(
                    Origin::signed(sender),
                    proof,
                    sender,
                    NON_AVT_TOKEN_ID,
                    DEFAULT_AMOUNT,
                    t1_recipient
                ));

                assert!(System::events().iter().any(|a| a.event
                    == Event::token_manager(crate::Event::<TestRuntime>::TokenLowered(
                        NON_AVT_TOKEN_ID,
                        sender,
                        recipient_account_id,
                        DEFAULT_AMOUNT,
                        t1_recipient
                    ))));
            });
        }
    }
}

mod get_proof {
    use super::*;

    #[test]
    fn succeeds_for_valid_signed_lower_call() {
        let mut ext = ExtBuilder::build_default().as_externality();
        ext.execute_with(|| {
            let sender = default_sender();
            let t1_recipient = default_t1_recipient();

            let proof = create_default_proof_for_signed_lower();
            let call = Box::new(MockCall::TokenManager(
                super::Call::<TestRuntime>::signed_lower(
                    proof.clone(),
                    sender,
                    NON_AVT_TOKEN_ID,
                    DEFAULT_AMOUNT,
                    t1_recipient,
                ),
            ));

            let result = TokenManager::get_proof(&call);
            assert!(result.is_ok());
            assert_eq!(result.unwrap(), proof);
        });
    }

    #[test]
    fn fails_for_invalid_calls() {
        let mut ext = ExtBuilder::build_default().as_externality();
        ext.execute_with(|| {
            let invalid_call = MockCall::System(frame_system::Call::remark(vec![]));

            assert!(matches!(
                TokenManager::get_proof(&invalid_call),
                Err(Error::<TestRuntime>::TransactionNotSupported)
            ));
        });
    }
}

mod fees {
    use super::*;

    mod are_paid_correctly {
        use super::*;

        #[test]
        // Ensure that the AVT gas fees are paid by the relayer
        fn when_relayer_with_enough_avt_proxy_a_signed_lower_call() {
            let mut ext = ExtBuilder::build_default().with_balances().as_externality();

            ext.execute_with(|| {
                let (_, sender, _, recipient_account_id, t1_recipient) = Context::default();
                let relayer = account_id_with_100_avt();

                default_setup();
                let proof = create_proof_for_signed_lower_with_relayer(&relayer);

                assert_eq!(Balances::free_balance(relayer), AMOUNT_100_TOKEN);
                assert_eq!(Balances::free_balance(sender), 0);
                assert_eq!(
                    <TokenManager as Store>::Balances::get((NON_AVT_TOKEN_ID, sender)),
                    2 * DEFAULT_AMOUNT
                );

                // Prepare the calls
                let inner_call = Box::new(MockCall::TokenManager(
                    super::Call::<TestRuntime>::signed_lower(
                        proof.clone(),
                        sender,
                        NON_AVT_TOKEN_ID,
                        DEFAULT_AMOUNT,
                        t1_recipient,
                    ),
                ));
                let outer_call =
                    &MockCall::TokenManager(token_manager::Call::proxy(inner_call.clone()));

                // Pay fees and submit the transaction
                assert_ok!(pay_gas_and_proxy_call(
                    &relayer,
                    outer_call,
                    inner_call.clone()
                ));

                // Check the effects of the transaction
                let call_hash = Hashing::hash_of(&inner_call);
                assert!(System::events().iter().any(|a| a.event
                    == Event::token_manager(crate::Event::<TestRuntime>::CallDispatched(
                        relayer, call_hash
                    ))));

                assert!(System::events().iter().any(|a| a.event
                    == Event::token_manager(crate::Event::<TestRuntime>::TokenLowered(
                        NON_AVT_TOKEN_ID,
                        sender,
                        recipient_account_id,
                        DEFAULT_AMOUNT,
                        t1_recipient
                    ))));

                let fee: u128 = (BASE_FEE + TX_LEN as u64) as u128;
                assert_eq!(Balances::free_balance(relayer), AMOUNT_100_TOKEN - fee);
                assert_eq!(Balances::free_balance(sender), 0);
                assert_eq!(
                    <TokenManager as Store>::Balances::get((NON_AVT_TOKEN_ID, sender)),
                    DEFAULT_AMOUNT
                );
            });
        }

        #[test]
        // Ensure that regular call's gas fees are paid by the sender
        fn when_sender_with_enough_avt_submit_a_signed_lower_call() {
            let mut ext = ExtBuilder::build_default().with_balances().as_externality();

            ext.execute_with(|| {
                let (_, _, _, recipient_account_id, t1_recipient) = Context::default();
                let sender_keys = key_pair_for_account_with_100_avt();
                let sender = get_account_id(&sender_keys);

                setup(&sender, DEFAULT_NONCE);
                let proof = create_proof_for_signed_lower(
                    &sender,
                    &sender,
                    NON_AVT_TOKEN_ID,
                    DEFAULT_AMOUNT,
                    DEFAULT_NONCE,
                    &sender_keys,
                    t1_recipient,
                );

                assert_eq!(Balances::free_balance(sender), AMOUNT_100_TOKEN);
                assert_eq!(
                    <TokenManager as Store>::Balances::get((NON_AVT_TOKEN_ID, sender)),
                    2 * DEFAULT_AMOUNT
                );
                assert_eq!(System::events().len(), 0);

                let call = &MockCall::TokenManager(token_manager::Call::signed_lower(
                    proof.clone(),
                    sender,
                    NON_AVT_TOKEN_ID,
                    DEFAULT_AMOUNT,
                    t1_recipient,
                ));
                assert_ok!(pay_gas_and_call_lower_directly(
                    &sender,
                    NON_AVT_TOKEN_ID,
                    DEFAULT_AMOUNT,
                    t1_recipient,
                    proof,
                    call
                ));

                let fee: u128 = (BASE_FEE + TX_LEN as u64) as u128;
                assert_eq!(System::events().len(), 1);
                assert_eq!(Balances::free_balance(sender), AMOUNT_100_TOKEN - fee);
                assert_eq!(
                    <TokenManager as Store>::Balances::get((NON_AVT_TOKEN_ID, sender)),
                    DEFAULT_AMOUNT
                );
                assert!(System::events().iter().any(|a| a.event
                    == Event::token_manager(crate::Event::<TestRuntime>::TokenLowered(
                        NON_AVT_TOKEN_ID,
                        sender,
                        recipient_account_id,
                        DEFAULT_AMOUNT,
                        t1_recipient
                    ))));
            });
        }
    }

    mod payment_fails {
        use super::*;

        #[test]
        // Relayer has insufficient funds to send transaction
        fn when_relayer_with_insufficient_avt_proxy_a_signed_lower_call() {
            let mut ext = ExtBuilder::build_default().with_balances().as_externality();

            ext.execute_with(|| {
                let (_, sender, relayer, _, t1_recipient) = Context::default();

                default_setup();
                let proof = create_default_proof_for_signed_lower();

                assert_eq!(Balances::free_balance(relayer), 0);
                assert_eq!(
                    <TokenManager as Store>::Balances::get((NON_AVT_TOKEN_ID, sender)),
                    2 * DEFAULT_AMOUNT
                );

                // Prepare the calls
                let inner_call = Box::new(MockCall::TokenManager(
                    super::Call::<TestRuntime>::signed_lower(
                        proof.clone(),
                        sender,
                        NON_AVT_TOKEN_ID,
                        DEFAULT_AMOUNT,
                        t1_recipient,
                    ),
                ));
                let outer_call =
                    &MockCall::TokenManager(token_manager::Call::proxy(inner_call.clone()));

                // Pay fees and submit the transaction.
                // Gas fee for this tx is (BASE_FEE + TX_LEN): 10 + 1 = 11 AVT
                assert_noop!(
                    pay_gas_and_proxy_call(&relayer, outer_call, inner_call),
                    <&str>::from(InvalidTransaction::Payment)
                );
                assert_eq!(System::events().len(), 0);
            });
        }

        #[test]
        // Ensure that regular call's gas fees are paid by the sender
        fn when_sender_with_insufficient_avt_submit_a_signed_lower_call() {
            let mut ext = ExtBuilder::build_default().with_balances().as_externality();

            ext.execute_with(|| {
                let (_, sender, _, _, t1_recipient) = Context::default();

                default_setup();
                let proof = create_default_proof_for_signed_lower();

                assert_eq!(Balances::free_balance(sender), 0);
                assert_eq!(
                    <TokenManager as Store>::Balances::get((NON_AVT_TOKEN_ID, sender)),
                    2 * DEFAULT_AMOUNT
                );

                let call = &MockCall::TokenManager(token_manager::Call::signed_lower(
                    proof.clone(),
                    sender,
                    NON_AVT_TOKEN_ID,
                    DEFAULT_AMOUNT,
                    t1_recipient,
                ));
                assert_noop!(
                    pay_gas_and_call_lower_directly(
                        &sender,
                        NON_AVT_TOKEN_ID,
                        DEFAULT_AMOUNT,
                        t1_recipient,
                        proof,
                        call
                    ),
                    <&str>::from(InvalidTransaction::Payment)
                );
                assert_eq!(System::events().len(), 0);
            });
        }
    }
}
