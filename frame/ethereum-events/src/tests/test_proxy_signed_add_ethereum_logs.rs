// Copyright 2021 Aventus (UK) Ltd.

#![cfg(test)]

use crate::mock::{Call as MockCall, *};
use crate::*;
use frame_support::{assert_ok, assert_noop};
use frame_system::RawOrigin;
use sp_core::{sr25519::Pair};
use sp_core::hash::H256;
use sp_avn_common::event_types::{
    EthEventId,
    ValidEvents,
};
use sp_runtime::DispatchError::BadOrigin;

mod proxy_signed_add_ethereum_log {
    use super::*;

    struct Context {
        origin: Origin,
        relayer: AccountId,
        signer: AccountId,
        tx_hash: H256,
        event_type: ValidEvents,
        current_block: BlockNumber,
        current_ingress_counter: IngressCounter,
        expected_ingress_counter: IngressCounter,
        signer_key_pair: Pair,
    }

    impl Default for Context {
        fn default() -> Self {
            let relayer = TestAccount::new([0u8; 32]).account_id();
            let signer = TestAccount::new([10u8; 32]);

            Context {
                origin: Origin::signed(relayer),
                relayer,
                signer: signer.account_id(),
                tx_hash: H256::from([5u8;32]),
                event_type: ValidEvents::Lifted,
                current_block: 1,
                current_ingress_counter: EthereumEvents::ingress_counter(),
                expected_ingress_counter: EthereumEvents::ingress_counter() + 1,
                signer_key_pair: signer.key_pair(),
            }
        }
    }

    impl Context {
        fn create_ethereum_event_id(&self) -> EthEventId {
            return EthEventId {
                signature: self.event_type.signature(),
                transaction_hash: self.tx_hash,
            };
        }

        fn build_proof(&self, signature: Signature) -> Proof<Signature, AccountId> {
            return Proof {
                signer: self.signer,
                relayer: self.relayer,
                signature: signature,
            };
        }

        fn create_proof_for_signed_add_ethereum_log(&self, sender_nonce: u64) -> Proof<Signature, AccountId> {
            let context = SIGNED_ADD_ETHEREUM_LOG_CONTEXT;
            let data_to_sign = (context, self.relayer, self.event_type.clone(), self.tx_hash, sender_nonce);
            let signature = sign(&self.signer_key_pair, &data_to_sign.encode());

            return self.build_proof(signature);
        }

        fn create_call_for_signed_add_ethereum_log(&self, sender_nonce: u64) -> Box<<TestRuntime as Config>::Call> {
            let proof = self.create_proof_for_signed_add_ethereum_log(sender_nonce);

            return Box::new(MockCall::EthereumEvents(
                super::super::Call::<TestRuntime>::signed_add_ethereum_log(
                    proof,
                    self.event_type.clone(),
                    self.tx_hash,
                ),
            ));
        }

        fn create_call_for_signed_add_ethereum_log_approved_by_relayer_not_sender(&self, sender_nonce: u64) -> Box<<TestRuntime as Config>::Call> {
            let mut proof = self.create_proof_for_signed_add_ethereum_log(sender_nonce);
            proof.signer = self.relayer;

            return Box::new(MockCall::EthereumEvents(
                super::super::Call::<TestRuntime>::signed_add_ethereum_log(
                    proof,
                    self.event_type.clone(),
                    self.tx_hash,
                ),
            ));
        }
    }

    #[test]
    fn succeeds_with_good_parameters() {
        let mut ext = ExtBuilder::build_default().as_externality();
        ext.execute_with(|| {
            let tx_hash: H256 = H256::random();

            let context: Context = Context {
                tx_hash: tx_hash,
                ..Default::default()
            };

            let nonce = EthereumEvents::proxy_nonce(context.signer);
            let signed_add_ethereum_log_call = context.create_call_for_signed_add_ethereum_log(nonce);

            assert_ok!(AvnProxy::proxy(context.origin.clone(), signed_add_ethereum_log_call, None));
            let ethereum_event = context.create_ethereum_event_id();

            assert_eq!(1, EthereumEvents::unchecked_events().len());
            assert_ne!(context.current_ingress_counter, EthereumEvents::ingress_counter());
            assert_eq!(true, EthereumEvents::unchecked_events().contains(&(ethereum_event.clone(), context.expected_ingress_counter, context.current_block)));

            let event = mock::Event::pallet_ethereum_events(
                crate::Event::<TestRuntime>::EthereumEventAdded(
                    ethereum_event.clone(),
                    context.signer,
                    EthereumEvents::get_contract_address_for_non_nft_event(&context.event_type).unwrap()
            ));

            assert!(EthereumEvents::event_emitted(&event));
            assert_eq!(2, System::events().len());

            // Proxy nonce has increased
            assert_eq!(EthereumEvents::proxy_nonce(context.signer), 1u64);
        });
    }

    mod fails_when {
        use super::*;

        #[test]
        fn extrinsic_is_unsigned() {
            let mut ext = ExtBuilder::build_default().as_externality();
            ext.execute_with(|| {
                let tx_hash: H256 = H256::random();

                let context: Context = Context {
                    tx_hash: tx_hash,
                    ..Default::default()
                };

                let nonce = EthereumEvents::proxy_nonce(context.signer);
                let signed_add_ethereum_log_call = context.create_call_for_signed_add_ethereum_log(nonce);

                assert_noop!(AvnProxy::proxy(RawOrigin::None.into(), signed_add_ethereum_log_call, None), BadOrigin);
            });
        }

        // We don't need to test SenderIsNotSigner error through AvnProxy::proxy call
        // as it always uses the proof.signer as the sender

        #[test]
        fn add_ethereum_log_call_with_malformed_tx_hash() {
            let mut ext = ExtBuilder::build_default().as_externality();
            ext.execute_with(|| {
                let tx_hash: H256 = H256::zero();

                let context: Context = Context {
                    tx_hash: tx_hash,
                    ..Default::default()
                };

                let nonce = EthereumEvents::proxy_nonce(context.signer);
                let signed_add_ethereum_log_call = context.create_call_for_signed_add_ethereum_log_approved_by_relayer_not_sender(nonce);

                assert_noop!(AvnProxy::proxy(context.origin.clone(), signed_add_ethereum_log_call, None), Error::<TestRuntime>::MalformedHash);
            });
        }

        #[test]
        fn add_ethereum_log_call_is_unauthorized() {
            let mut ext = ExtBuilder::build_default().as_externality();
            ext.execute_with(|| {
                let tx_hash: H256 = H256::random();

                let context: Context = Context {
                    tx_hash: tx_hash,
                    ..Default::default()
                };

                let nonce = EthereumEvents::proxy_nonce(context.signer);
                let signed_add_ethereum_log_call = context.create_call_for_signed_add_ethereum_log_approved_by_relayer_not_sender(nonce);

                assert_noop!(AvnProxy::proxy(context.origin.clone(), signed_add_ethereum_log_call, None), Error::<TestRuntime>::UnauthorizedSignedAddEthereumLogTransaction);
            });
        }
    }
}