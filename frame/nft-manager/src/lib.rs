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

//! # nft-manager pallet

#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Decode, Encode};
use core::convert::TryInto;
use frame_support::{
    decl_error, decl_event, decl_module, decl_storage,
    dispatch::{
        DispatchErrorWithPostInfo, DispatchResult, DispatchResultWithPostInfo, Dispatchable,
    },
    ensure,
    traits::{Get, IsSubType},
    weights::{PostDispatchInfo, Weight},
    Parameter,
};
use frame_system::{self as system, ensure_signed};
use pallet_avn::{self as avn};
use pallet_ethereum_events::{self as ethereum_events, ProcessedEventsChecker};
use sp_avn_common::{
    event_types::{
        EthEvent, EthEventId, EventData, NftCancelListingData, NftTransferToData,
        ProcessedEventHandler, NftEndBatchListingData
    },
    CallDecoder, InnerCallValidator, Proof,
};
use sp_core::{H160, H256, U256};
use sp_io::hashing::keccak_256;
use sp_runtime::traits::{Hash, IdentifyAccount, Member, Verify};
use sp_std::prelude::*;

pub mod nft_data;
use crate::nft_data::*;

pub mod batch_nft;
use crate::batch_nft::*;

pub mod default_weights;
pub use default_weights::WeightInfo;

const SINGLE_NFT_ID_CONTEXT: &'static [u8; 1] = b"A";
const BATCH_NFT_ID_CONTEXT: &'static [u8; 1] = b"B";
const BATCH_ID_CONTEXT: &'static [u8; 1] = b"G";
pub const SIGNED_MINT_SINGLE_NFT_CONTEXT: &'static [u8] =
    b"authorization for mint single nft operation";
pub const SIGNED_LIST_NFT_OPEN_FOR_SALE_CONTEXT: &'static [u8] =
    b"authorization for list nft open for sale operation";
pub const SIGNED_TRANSFER_FIAT_NFT_CONTEXT: &'static [u8] =
    b"authorization for transfer fiat nft operation";
pub const SIGNED_CANCEL_LIST_FIAT_NFT_CONTEXT: &'static [u8] =
    b"authorization for cancel list fiat nft for sale operation";
pub const SIGNED_MINT_BATCH_NFT_CONTEXT: &'static [u8] =
    b"authorization for mint batch nft operation";

const MAX_NUMBER_OF_ROYALTIES: u32 = 5;

pub trait Config: system::Config + avn::Config {
    /// The overarching event type.
    type Event: From<Event<Self>> + Into<<Self as system::Config>::Event>;

    /// The overarching call type.
    type Call: Parameter
        + Dispatchable<Origin = <Self as frame_system::Config>::Origin>
        + IsSubType<Call<Self>>
        + From<Call<Self>>;

    type ProcessedEventsChecker: ProcessedEventsChecker;

    /// A type that can be used to verify signatures
    type Public: IdentifyAccount<AccountId = Self::AccountId>;

    /// The signature type used by accounts/transactions.
    type Signature: Verify<Signer = Self::Public>
        + Member
        + Decode
        + Encode
        + From<sp_core::sr25519::Signature>;

    type WeightInfo: WeightInfo;
}

pub type NftId = U256;
pub type NftInfoId = U256;
pub type NftBatchId = U256;
pub type NftUniqueId = U256;
decl_event!(
    pub enum Event<T>
    where
        AccountId = <T as system::Config>::AccountId,
        MinterTier1Address = H160,
        TotalSupply = u64,
        Relayer = <T as system::Config>::AccountId,
        Hash = <T as system::Config>::Hash,
        OpId = u64,
    {
        SingleNftMinted(NftId, AccountId, MinterTier1Address),
        ///nft_id, batch_id, provenance, owner
        BatchNftMinted(NftId, NftBatchId, MinterTier1Address, AccountId),
        /// nft_id, sale_type
        NftOpenForSale(NftId, NftSaleType),
        /// batch_id, sale_type
        BatchOpenForSale(NftBatchId, NftSaleType),
        /// EthNftTransfer(NftId, NewOwnerAccountId, NftSaleType, OpId, EthEventId),
        EthNftTransfer(NftId, AccountId, NftSaleType, OpId, EthEventId),
        /// FiatNftTransfer(NftId, SenderAccountId, NewOwnerAccountId, NftSaleType, NftNonce)
        FiatNftTransfer(NftId, AccountId, AccountId, NftSaleType, OpId),
        /// CancelSingleEthNftListing(NftId, NftSaleType, OpId, EthEventId),
        CancelSingleEthNftListing(NftId, NftSaleType, OpId, EthEventId),
        /// CancelSingleFiatNftListing(NftId, NftSaleType, NftNonce)
        CancelSingleFiatNftListing(NftId, NftSaleType, OpId),
        CallDispatched(Relayer, Hash),
        ///batch_id, total_supply, batch_creator, provenance
        BatchCreated(NftBatchId, TotalSupply, AccountId, MinterTier1Address),
        /// batch_id, market
        BatchSaleEnded(NftBatchId, NftSaleType),
    }
);

decl_error! {
    pub enum Error for Module<T: Config> {
        NftAlreadyExists,
        /// When specifying rates, parts_per_million must not be greater than 1 million
        RoyaltyRateIsNotValid,
        /// When specifying rates, sum of parts_per_millions must not be greater than 1 million
        TotalRoyaltyRateIsNotValid,
        T1AuthorityIsMandatory,
        ExternalRefIsMandatory,
        /// The external reference is already used
        ExternalRefIsAlreadyInUse,
        /// There is not data associated with an nftInfoId
        NftInfoMissing,
        NftIdDoesNotExist,
        UnsupportedMarket,
        /// Signed extrinsic with a proof must be called by the signer of the proof
        SenderIsNotSigner,
        SenderIsNotOwner,
        NftAlreadyListed,
        NftIsLocked,
        NftNotListedForSale,
        NftNotListedForEthereumSale,
        NftNotListedForFiatSale,
        NoTier1EventForNftOperation,
        /// The op_id did not match the nft token nonce for the operation
        NftNonceMismatch,
        UnauthorizedTransaction,
        UnauthorizedProxyTransaction,
        UnauthorizedSignedLiftNftOpenForSaleTransaction,
        UnauthorizedSignedMintSingleNftTransaction,
        UnauthorizedSignedTransferFiatNftTransaction,
        UnauthorizedSignedCancelListFiatNftTransaction,
        TransactionNotSupported,
        TransferToIsMandatory,
        UnauthorizedSignedCreateBatchTransaction,
        BatchAlreadyExists,
        TotalSupplyZero,
        UnauthorizedSignedMintBatchNftTransaction,
        BatchIdIsMandatory,
        BatchDoesNotExist,
        SenderIsNotBatchCreator,
        TotalSupplyExceeded,
        UnauthorizedSignedListBatchForSaleTransaction,
        BatchAlreadyListed,
        NoNftsToSell,
        BatchNotListed,
        UnauthorizedSignedEndBatchSaleTransaction,
        BatchNotListedForFiatSale,
        BatchNotListedForEthereumSale,
    }
}

decl_storage! {
    trait Store for Module<T: Config> as NftManager {
        /// A mapping between NFT Id and data
        pub Nfts get(fn nfts): map hasher(blake2_128_concat) NftId => Nft<T::AccountId>;
        /// A mapping between NFT info Id and info data
        pub NftInfos get(fn nft_infos): map hasher(blake2_128_concat) NftInfoId => NftInfo<T::AccountId>;
        /// A mapping between the external batch id and its nft Ids
        pub NftBatches get(fn nft_batches): map hasher(blake2_128_concat) NftBatchId => Vec<NftId>;
        /// A mapping between the external batch id and its corresponding NtfInfoId
        pub BatchInfoId get(fn batch_info_id): map hasher(blake2_128_concat) NftBatchId => NftInfoId;
        /// A mapping between an ExternalRef and a flag to show that an NFT has used it
        pub UsedExternalReferences get(fn is_external_ref_used) : map hasher(blake2_128_concat) Vec<u8> => bool;
        /// The Id that will be used when creating the new NftInfo record
        pub NextInfoId get(fn next_info_id): NftInfoId;
        /// The Id that will be used when creating the new single Nft
        //TODO: Rename this item because its not just used for single NFTs
        pub NextSingleNftUniqueId get(fn next_unique_id): U256;
        /// A mapping that keeps all the nfts that are open to sale in a specific market
        pub NftOpenForSale get(fn get_nft_open_for_sale_on): map hasher(blake2_128_concat) NftId => NftSaleType;
        /// A mapping between the external batch id and its nft Ids
        pub OwnedNfts get(fn get_owned_nfts): map hasher(blake2_128_concat) T::AccountId => Vec<NftId>;
        StorageVersion: Releases;
        /// An account nonce that represents the number of proxy transactions from this account
        pub BatchNonces get(fn batch_nonce): map hasher(blake2_128_concat) T::AccountId => u64;
        /// A mapping that keeps all the batches that are open to sale in a specific market
        pub BatchOpenForSale get(fn get_batch_sale_market): map hasher(blake2_128_concat) NftBatchId => NftSaleType;
    }
}

decl_module! {
    pub struct Module<T: Config> for enum Call where origin: T::Origin {
        type Error = Error<T>;

        fn deposit_event() = default;

        /// Mint a single NFT
        //
        // # <weight>
        //  Keys:
        //   R - number of royalties
        //   - 1 iteration through all royalties O(R).
        //   - 3 DbReads O(1).
        //   - 3 DbWrites O(1).
        //   - 1 Event emitted O(1).
        // Total Complexity: O(1 + R)
        // # </weight>
        #[weight = T::WeightInfo::mint_single_nft(MAX_NUMBER_OF_ROYALTIES)]
        fn mint_single_nft(origin,
            unique_external_ref: Vec<u8>,
            royalties: Vec<Royalty>,
            t1_authority: H160) -> DispatchResult
        {
            let sender = ensure_signed(origin)?;
            Self::validate_mint_single_nft_request(&unique_external_ref, &royalties, t1_authority)?;

            // We trust the input for the value of t1_authority
            let nft_id = Self::generate_nft_id_single_mint(&t1_authority, Self::get_unique_id_and_advance());
            ensure!(Nfts::<T>::contains_key(&nft_id) == false, Error::<T>::NftAlreadyExists);

            // No errors allowed after this point because `get_info_id_and_advance` mutates storage
            let info_id = Self::get_info_id_and_advance();
            let (nft, info) = Self::insert_single_nft_into_chain(
                info_id, royalties, t1_authority, nft_id, unique_external_ref, sender
            );

            Self::deposit_event(RawEvent::SingleNftMinted(nft.nft_id, nft.owner, info.t1_authority));

            Ok(())
        }

        /// Mint a single NFT signed by nft owner
        //
        // # <weight>
        //  Keys: R - number of royalties
        //  - 2 * Iteration through all royalties: O(R).
        //  - DbReads: Nfts, NextSingleNftUniqueId, UsedExternalReferences, NextInfoId: O(1)
        //  - DbWrites: NextSingleNftUniqueId, NextInfoId, NftInfos, Nfts, UsedExternalReferences: O(1)
        //  - One codec encode operation: O(1).
        //  - One signature verification operation: O(1).
        //  - Event Emitted: O(1)
        //  Total Complexity: `O(1 + R)`
        // # </weight>
        #[weight = T::WeightInfo::signed_mint_single_nft(MAX_NUMBER_OF_ROYALTIES)]
        fn signed_mint_single_nft(origin,
            proof: Proof<T::Signature, T::AccountId>,
            unique_external_ref: Vec<u8>,
            royalties: Vec<Royalty>,
            t1_authority: H160) -> DispatchResult
        {
            let sender = ensure_signed(origin)?;
            ensure!(sender == proof.signer, Error::<T>::SenderIsNotSigner);
            Self::validate_mint_single_nft_request(&unique_external_ref, &royalties, t1_authority)?;

            let signed_payload = Self::encode_mint_single_nft_params(&proof, &unique_external_ref, &royalties, &t1_authority);
            ensure!(
                Self::verify_signature(&proof, &signed_payload.as_slice()).is_ok(),
                Error::<T>::UnauthorizedSignedMintSingleNftTransaction
            );

            // We trust the input for the value of t1_authority
            let nft_id = Self::generate_nft_id_single_mint(&t1_authority, Self::get_unique_id_and_advance());
            ensure!(Nfts::<T>::contains_key(&nft_id) == false, Error::<T>::NftAlreadyExists);

            // No errors allowed after this point because `get_info_id_and_advance` mutates storage
            let info_id = Self::get_info_id_and_advance();
            let (nft, info) = Self::insert_single_nft_into_chain(
                info_id, royalties, t1_authority, nft_id, unique_external_ref, proof.signer
            );

            Self::deposit_event(RawEvent::SingleNftMinted(nft.nft_id, nft.owner, info.t1_authority));

            Ok(())
        }

        /// List an nft open for sale
        //
        // # <weight>
        //  - DbReads: 2 * Nfts, NftOpenForSale: O(1)
        //  - DbWrites: Nfts, NftOpenForSale: O(1)
        //  - Event Emitted: O(1)
        //  Total Complexity: `O(1)`
        // # </weight>
        #[weight = T::WeightInfo::list_nft_open_for_sale()]
        fn list_nft_open_for_sale(origin,
            nft_id: NftId,
            market: NftSaleType,
        ) -> DispatchResult
        {
            let sender = ensure_signed(origin)?;
            Self::validate_open_for_sale_request(sender, nft_id, market.clone())?;
            Self::open_nft_for_sale(&nft_id, &market);
            Self::deposit_event(RawEvent::NftOpenForSale(nft_id, market));
            Ok(())
        }

        /// List an nft open for sale by a relayer
        //
        // # <weight>
        //  - DbReads: 2 * Nfts, NftOpenForSale: O(1)
        //  - DbWrites: Nfts, NftOpenForSale: O(1)
        //  - One codec encode operation: O(1).
        //  - One signature verification operation: O(1).
        //  - Event Emitted: O(1)
        //  Total Complexity: `O(1)`
        // # </weight>
        #[weight = T::WeightInfo::signed_list_nft_open_for_sale()]
        fn signed_list_nft_open_for_sale(origin,
            proof: Proof<T::Signature, T::AccountId>,
            nft_id: NftId,
            market: NftSaleType,
        ) -> DispatchResult
        {
            let sender = ensure_signed(origin)?;
            ensure!(sender == proof.signer, Error::<T>::SenderIsNotSigner);
            Self::validate_open_for_sale_request(sender, nft_id, market.clone())?;

            let signed_payload = Self::encode_list_nft_for_sale_params(&proof, &nft_id, &market);
            ensure!(
                Self::verify_signature(&proof, &signed_payload.as_slice()).is_ok(),
                Error::<T>::UnauthorizedSignedLiftNftOpenForSaleTransaction
            );

            Self::open_nft_for_sale(&nft_id, &market);
            Self::deposit_event(RawEvent::NftOpenForSale(nft_id, market));

            Ok(())
        }

        /// Transfer a nft open for sale on fiat market to a new owner by a relayer
        //
        // # <weight>
        //  - DbReads: 2 * Nfts, 4* NftOpenForSale: O(1)
        //  - DbWrites: Nfts, NftOpenForSale : O(1)
        //  - One codec encode operation: O(1).
        //  - One signature verification operation: O(1).
        //  - Event Emitted: FiatNftTransfer: O(1)
        //  Total Complexity: `O(1)`
        // # </weight>
        #[weight = T::WeightInfo::signed_transfer_fiat_nft()]
        fn signed_transfer_fiat_nft(origin,
            proof: Proof<T::Signature, T::AccountId>,
            nft_id: U256,
            t2_transfer_to_public_key: H256,
        ) -> DispatchResult {
            let sender = ensure_signed(origin)?;
            ensure!(sender == proof.signer, Error::<T>::SenderIsNotSigner);
            ensure!(t2_transfer_to_public_key.is_zero() == false, Error::<T>::TransferToIsMandatory);
            Self::validate_nft_open_for_fiat_sale(sender.clone(), nft_id)?;

            let nft = Self::nfts(nft_id);
            let signed_payload = Self::encode_transfer_fiat_nft_params(&proof, &nft_id, &t2_transfer_to_public_key);
            ensure!(
                Self::verify_signature(&proof, &signed_payload.as_slice()).is_ok(),
                Error::<T>::UnauthorizedSignedTransferFiatNftTransaction
            );

            let new_nft_owner = T::AccountId::decode(&mut t2_transfer_to_public_key.as_bytes())
                .expect("32 bytes will always decode into an AccountId");
            let market = Self::get_nft_open_for_sale_on(nft_id);

            Self::transfer_nft(&nft_id, &nft.owner, &new_nft_owner.clone())?;
            Self::deposit_event(RawEvent::FiatNftTransfer(nft_id, sender, new_nft_owner, market, nft.nonce));

            Ok(())
        }

        /// Cancel a nft open for sale on fiat market by a relayer
        //
        // # <weight>
        //  - DbReads: 2* Nfts, 4 * NftOpenForSale: O(1)
        //  - DbWrites: Nfts, NftOpenForSale: O(1)
        //  - One codec encode operation: O(1).
        //  - One signature verification operation: O(1).
        //  - Event Emitted: CancelSingleFiatNftListing: O(1)
        //  Total Complexity: `O(1)`
        // # </weight>
        #[weight = T::WeightInfo::signed_cancel_list_fiat_nft()]
        fn signed_cancel_list_fiat_nft(origin,
            proof: Proof<T::Signature, T::AccountId>,
            nft_id: U256,
        ) -> DispatchResult {
            let sender = ensure_signed(origin)?;
            ensure!(sender == proof.signer, Error::<T>::SenderIsNotSigner);
            Self::validate_nft_open_for_fiat_sale(sender.clone(), nft_id)?;

            let nft = Self::nfts(nft_id);
            let signed_payload = Self::encode_cancel_list_fiat_nft_params(&proof, &nft_id);
            ensure!(
                Self::verify_signature(&proof, &signed_payload.as_slice()).is_ok(),
                Error::<T>::UnauthorizedSignedCancelListFiatNftTransaction
            );

            let market = Self::get_nft_open_for_sale_on(nft_id);

            Self::unlist_nft_for_sale(nft_id)?;
            Self::deposit_event(RawEvent::CancelSingleFiatNftListing(nft_id, market, nft.nonce));

            Ok(())
        }

        /// This extrinsic allows a relayer to dispatch a call from this pallet for a sender.
        /// Currently only `signed_list_nft_open_for_sale` is allowed
        ///
        /// As a general rule, every function that can be proxied should follow this convention:
        /// - its first argument (after origin) should be a public verification key and a signature
        //
        // # <weight>
        // - One get proof operation: O(1)
        // - One hash of operation: O(1)
        // - One signed transfer operation: O(1)
        // - One event emitted: O(1)
        // Total Complexity: `O(1)`
        // # </weight>
        #[weight = T::WeightInfo::proxy_signed_list_nft_open_for_sale()
            .max(T::WeightInfo::proxy_signed_mint_single_nft(MAX_NUMBER_OF_ROYALTIES))
            .max(T::WeightInfo::proxy_signed_transfer_fiat_nft())
            .max(T::WeightInfo::proxy_signed_cancel_list_fiat_nft())]
        pub fn proxy(origin, call: Box<<T as Config>::Call>) -> DispatchResultWithPostInfo {
            let relayer = ensure_signed(origin)?;

            let proof = Self::get_proof(&*call)?;
            ensure!(relayer == proof.relayer, Error::<T>::UnauthorizedProxyTransaction);

            let call_hash: T::Hash = T::Hashing::hash_of(&call);
            call.clone().dispatch(frame_system::RawOrigin::Signed(proof.signer).into()).map(|_| ()).map_err(|e| e.error)?;
            Self::deposit_event(RawEvent::CallDispatched(relayer, call_hash));

            return Self::get_dispatch_result_with_post_info(call);
        }

        /// Creates a new batch
        #[weight = T::WeightInfo::proxy_signed_create_batch(MAX_NUMBER_OF_ROYALTIES)]
        fn signed_create_batch(origin,
            proof: Proof<T::Signature, T::AccountId>,
            total_supply: u64,
            royalties: Vec<Royalty>,
            t1_authority: H160) -> DispatchResult
        {
            let sender = ensure_signed(origin)?;
            ensure!(sender == proof.signer, Error::<T>::SenderIsNotSigner);
            ensure!(t1_authority.is_zero() == false, Error::<T>::T1AuthorityIsMandatory);
            ensure!(total_supply > 0u64, Error::<T>::TotalSupplyZero);

            Self::validate_royalties(&royalties)?;

            let sender_nonce = Self::batch_nonce(&sender);
            let signed_payload = encode_create_batch_params::<T>(&proof, &royalties, &t1_authority, &total_supply, &sender_nonce);
            ensure!(
                Self::verify_signature(&proof, &signed_payload.as_slice()).is_ok(),
                Error::<T>::UnauthorizedSignedCreateBatchTransaction
            );

            let batch_id = generate_batch_id::<T>(Self::get_unique_id_and_advance());
            ensure!(BatchInfoId::contains_key(&batch_id) == false, Error::<T>::BatchAlreadyExists);

            // No errors allowed after this point because `get_info_id_and_advance` mutates storage
            let info_id = Self::get_info_id_and_advance();
            create_batch::<T>(info_id, batch_id, royalties, total_supply, t1_authority, sender.clone());

            <BatchNonces<T>>::mutate(&sender, |n| *n += 1);

            Self::deposit_event(RawEvent::BatchCreated(batch_id, total_supply, sender, t1_authority));

            Ok(())
        }

        /// Mints an nft that belongs to a batch
        #[weight = T::WeightInfo::proxy_signed_mint_batch_nft()]
        fn signed_mint_batch_nft(origin,
            proof: Proof<T::Signature, T::AccountId>,
            batch_id: NftBatchId,
            index: u64,
            owner: T::AccountId,
            unique_external_ref: Vec<u8>) -> DispatchResult
        {
            let sender = ensure_signed(origin)?;
            ensure!(sender == proof.signer, Error::<T>::SenderIsNotSigner);

            let nft_info = validate_mint_batch_nft_request::<T>(batch_id, &unique_external_ref)?;
            ensure!(<BatchOpenForSale>::get(&batch_id) == NftSaleType::Fiat, Error::<T>::BatchNotListedForFiatSale);
            ensure!(nft_info.creator == Some(sender), Error::<T>::SenderIsNotBatchCreator);

            let signed_payload = encode_mint_batch_nft_params::<T>(&proof, &batch_id, &index, &unique_external_ref, &owner);
            ensure!(
                Self::verify_signature(&proof, &signed_payload.as_slice()).is_ok(),
                Error::<T>::UnauthorizedSignedMintBatchNftTransaction
            );

            mint_batch_nft::<T>(batch_id, owner, index, &unique_external_ref)?;

            Ok(())
        }

        #[weight = T::WeightInfo::proxy_signed_list_batch_for_sale()]
        fn signed_list_batch_for_sale(origin,
            proof: Proof<T::Signature, T::AccountId>,
            batch_id: NftBatchId,
            market: NftSaleType,
        ) -> DispatchResult
        {
            let sender = ensure_signed(origin)?;
            ensure!(sender == proof.signer, Error::<T>::SenderIsNotSigner);
            ensure!(batch_id.is_zero() == false, Error::<T>::BatchIdIsMandatory);
            ensure!(BatchInfoId::contains_key(&batch_id), Error::<T>::BatchDoesNotExist);
            ensure!(market != NftSaleType::Unknown, Error::<T>::UnsupportedMarket);

            let sender_nonce = Self::batch_nonce(&sender);
            let nft_info = get_nft_info_for_batch::<T>(&batch_id)?;
            //Only the batch creator can allow mint operations.
            ensure!(nft_info.creator == Some(sender.clone()), Error::<T>::SenderIsNotBatchCreator);
            ensure!((NftBatches::get(&batch_id).len() as u64) < nft_info.total_supply, Error::<T>::NoNftsToSell);
            ensure!(<BatchOpenForSale>::contains_key(&batch_id) == false, Error::<T>::BatchAlreadyListed);

            let signed_payload = encode_list_batch_for_sale_params::<T>(&proof, &batch_id, &market, &sender_nonce);
            ensure!(
                Self::verify_signature(&proof, &signed_payload.as_slice()).is_ok(),
                Error::<T>::UnauthorizedSignedListBatchForSaleTransaction
            );

            <BatchOpenForSale>::insert(batch_id, market);
            <BatchNonces<T>>::mutate(&sender, |n| *n += 1);

            Self::deposit_event(RawEvent::BatchOpenForSale(batch_id, market));

            Ok(())
        }

        #[weight = T::WeightInfo::proxy_signed_end_batch_sale()]
        fn signed_end_batch_sale(origin,
            proof: Proof<T::Signature, T::AccountId>,
            batch_id: NftBatchId,
        ) -> DispatchResult {
            let sender = ensure_signed(origin)?;
            ensure!(sender == proof.signer, Error::<T>::SenderIsNotSigner);
            validate_end_batch_listing_request::<T>(&batch_id)?;
            ensure!(<BatchOpenForSale>::get(&batch_id) == NftSaleType::Fiat, Error::<T>::BatchNotListedForFiatSale);

            let sender_nonce = Self::batch_nonce(&sender);
            let nft_info = get_nft_info_for_batch::<T>(&batch_id)?;
            //Only the batch creator can end the listing
            ensure!(nft_info.creator == Some(sender.clone()), Error::<T>::SenderIsNotBatchCreator);

            let signed_payload = encode_end_batch_sale_params::<T>(&proof, &batch_id, &sender_nonce);
            ensure!(
                Self::verify_signature(&proof, &signed_payload.as_slice()).is_ok(),
                Error::<T>::UnauthorizedSignedEndBatchSaleTransaction
            );

            let market = <BatchOpenForSale>::get(batch_id);
            end_batch_listing::<T>(&batch_id, market)?;
            <BatchNonces<T>>::mutate(&sender, |n| *n += 1);

            Ok(())
        }

        // Note: this "special" function will run during every runtime upgrade. Any complicated migration logic should be done in a
        // separate function so it can be tested properly.
        fn on_runtime_upgrade() -> Weight {
            if StorageVersion::get() == Releases::V2_0_0 {
                StorageVersion::put(Releases::V3_0_0);
                return migrations::migrate_to_batch_nft::<T>()
            }

            return 0;
        }
    }
}

impl<T: Config> Module<T> {
    fn validate_mint_single_nft_request(
        unique_external_ref: &Vec<u8>,
        royalties: &Vec<Royalty>,
        t1_authority: H160,
    ) -> DispatchResult {
        ensure!(
            t1_authority.is_zero() == false,
            Error::<T>::T1AuthorityIsMandatory
        );

        Self::validate_external_ref(unique_external_ref)?;
        Self::validate_royalties(royalties)?;

        Ok(())
    }

    fn validate_external_ref(unique_external_ref: &Vec<u8>) -> DispatchResult {
        ensure!(
            unique_external_ref.len() > 0,
            Error::<T>::ExternalRefIsMandatory
        );
        ensure!(
            Self::is_external_ref_used(&unique_external_ref) == false,
            Error::<T>::ExternalRefIsAlreadyInUse
        );

        Ok(())
    }

    fn validate_royalties(royalties: &Vec<Royalty>) -> DispatchResult {
        let invalid_rates_found = royalties.iter().any(|r| !r.rate.is_valid());
        ensure!(invalid_rates_found == false, Error::<T>::RoyaltyRateIsNotValid);

        let rate_total = royalties
            .iter()
            .map(|r| r.rate.parts_per_million)
            .sum::<u32>();

        ensure!(rate_total <= 1_000_000, Error::<T>::TotalRoyaltyRateIsNotValid);

        Ok(())
    }

    fn validate_open_for_sale_request(
        sender: T::AccountId,
        nft_id: NftId,
        market: NftSaleType,
    ) -> DispatchResult {
        ensure!(
            market != NftSaleType::Unknown,
            Error::<T>::UnsupportedMarket
        );
        ensure!(
            <Nfts<T>>::contains_key(&nft_id) == true,
            Error::<T>::NftIdDoesNotExist
        );
        ensure!(
            <NftOpenForSale>::contains_key(&nft_id) == false,
            Error::<T>::NftAlreadyListed
        );

        let nft = Self::nfts(nft_id);
        ensure!(nft.owner == sender, Error::<T>::SenderIsNotOwner);
        ensure!(nft.is_locked == false, Error::<T>::NftIsLocked);

        Ok(())
    }

    fn validate_nft_open_for_fiat_sale(sender: T::AccountId, nft_id: NftId) -> DispatchResult {
        ensure!(
            <NftOpenForSale>::contains_key(nft_id) == true,
            Error::<T>::NftNotListedForSale
        );
        ensure!(
            Self::get_nft_open_for_sale_on(nft_id) == NftSaleType::Fiat,
            Error::<T>::NftNotListedForFiatSale
        );

        let nft = Self::nfts(nft_id);
        ensure!(nft.owner == sender, Error::<T>::SenderIsNotOwner);
        ensure!(nft.is_locked == false, Error::<T>::NftIsLocked);

        Ok(())
    }

    /// Returns the next available info id and increases the storage item by 1
    fn get_info_id_and_advance() -> NftInfoId {
        let id = Self::next_info_id();
        <NextInfoId>::mutate(|n| *n += U256::from(1));

        return id;
    }

    fn get_unique_id_and_advance() -> NftUniqueId {
        let id = Self::next_unique_id();
        <NextSingleNftUniqueId>::mutate(|n| *n += U256::from(1));

        return id;
    }

    fn insert_single_nft_into_chain(
        info_id: NftInfoId,
        royalties: Vec<Royalty>,
        t1_authority: H160,
        nft_id: NftId,
        unique_external_ref: Vec<u8>,
        owner: T::AccountId
    ) -> (Nft<T::AccountId>, NftInfo<T::AccountId>) {
        let info = NftInfo::new(info_id, royalties, t1_authority);
        let nft = Nft::new(nft_id, info_id, unique_external_ref, owner.clone());

        <NftInfos<T>>::insert(info.info_id, &info);

        Self::add_nft_and_update_owner(&owner, &nft);
        return (nft, info);
    }

    fn open_nft_for_sale(nft_id: &NftId, market: &NftSaleType) {
        <NftOpenForSale>::insert(nft_id, market);
        <Nfts<T>>::mutate(nft_id, |nft| {
            nft.nonce += 1u64;
        });
    }

    /// The NftId for a single mint is calculated by this formula: uint256(keccak256(“A”, contract_address, unique_id))
    // TODOs: Confirm that the data are packed the same as encodePacked.
    // TODOs: Confirm that which data needs to be in BE format.
    fn generate_nft_id_single_mint(contract: &H160, unique_id: NftUniqueId) -> U256 {
        let mut data_to_hash = SINGLE_NFT_ID_CONTEXT.to_vec();

        data_to_hash.append(&mut contract[..].to_vec());

        let mut unique_id_be = [0u8; 32];
        unique_id.to_big_endian(&mut unique_id_be);
        data_to_hash.append(&mut unique_id_be.to_vec());

        let hash = keccak_256(&data_to_hash);

        return U256::from(hash);
    }

    fn remove_listing_from_open_for_sale(nft_id: &NftId) -> DispatchResult {
        ensure!(
            <NftOpenForSale>::contains_key(nft_id) == true,
            Error::<T>::NftNotListedForSale
        );
        <NftOpenForSale>::remove(nft_id);
        Ok(())
    }

    fn transfer_eth_nft(event_id: &EthEventId, data: &NftTransferToData) -> DispatchResult {
        let market = Self::get_nft_open_for_sale_on(data.nft_id);
        ensure!(
            market == NftSaleType::Ethereum,
            Error::<T>::NftNotListedForEthereumSale
        );

        let nft = Self::nfts(data.nft_id);

        ensure!(data.op_id == nft.nonce, Error::<T>::NftNonceMismatch);
        ensure!(
            T::ProcessedEventsChecker::check_event(event_id),
            Error::<T>::NoTier1EventForNftOperation
        );

        let new_nft_owner = T::AccountId::decode(&mut data.t2_transfer_to_public_key.as_bytes())
            .expect("32 bytes will always decode into an AccountId");
        Self::transfer_nft(&data.nft_id, &nft.owner, &new_nft_owner)?;
        Self::deposit_event(RawEvent::EthNftTransfer(
            data.nft_id,
            new_nft_owner,
            market,
            data.op_id,
            event_id.clone(),
        ));

        Ok(())
    }

    fn transfer_nft(
        nft_id: &NftId,
        old_nft_owner: &T::AccountId,
        new_nft_owner: &T::AccountId,
    ) -> DispatchResult {
        Self::remove_listing_from_open_for_sale(nft_id)?;
        Self::update_owner_for_transfer(nft_id, old_nft_owner, new_nft_owner);
        Ok(())
    }

    // See https://github.com/Aventus-Network-Services/avn-tier2/pull/991#discussion_r832470480 for details of why we have this
    // as a separate function
    fn update_owner_for_transfer(
        nft_id: &NftId,
        old_nft_owner: &T::AccountId,
        new_nft_owner: &T::AccountId,
    ) {
        <Nfts<T>>::mutate(nft_id, |nft| {
            nft.owner = new_nft_owner.clone();
            nft.nonce += 1u64;
        });

        <OwnedNfts<T>>::mutate(old_nft_owner, |owner_nfts| {
            if let Some(pos) = owner_nfts.iter().position(|n| n == nft_id) {
                owner_nfts.swap_remove(pos);
            }
        });

        if <OwnedNfts<T>>::contains_key(new_nft_owner) {
            <OwnedNfts<T>>::mutate(new_nft_owner, |owner_nfts| {
                owner_nfts.push(*nft_id);
            });
        } else {
            <OwnedNfts<T>>::insert(new_nft_owner, vec![*nft_id]);
        }
    }

    // See https://github.com/Aventus-Network-Services/avn-tier2/pull/991#discussion_r832470480 for details of why we have this
    // as a separate function
    fn add_nft_and_update_owner(owner: &T::AccountId, nft: &Nft<T::AccountId>) {
        <Nfts<T>>::insert(nft.nft_id, &nft);
        <UsedExternalReferences>::insert(&nft.unique_external_ref, true);

        if <OwnedNfts<T>>::contains_key(owner) {
            <OwnedNfts<T>>::mutate(owner, |owner_nfts| {
                owner_nfts.push(nft.nft_id);
            });
        } else {
            <OwnedNfts<T>>::insert(owner, vec![nft.nft_id]);
        }
    }

    fn cancel_eth_nft_listing(
        event_id: &EthEventId,
        data: &NftCancelListingData,
    ) -> DispatchResult {
        let market = Self::get_nft_open_for_sale_on(data.nft_id);
        ensure!(
            market == NftSaleType::Ethereum,
            Error::<T>::NftNotListedForEthereumSale
        );
        ensure!(
            data.op_id == Self::nfts(data.nft_id).nonce,
            Error::<T>::NftNonceMismatch
        );
        ensure!(
            T::ProcessedEventsChecker::check_event(event_id),
            Error::<T>::NoTier1EventForNftOperation
        );

        Self::unlist_nft_for_sale(data.nft_id)?;
        Self::deposit_event(RawEvent::CancelSingleEthNftListing(
            data.nft_id,
            market,
            data.op_id,
            event_id.clone(),
        ));

        Ok(())
    }

    fn unlist_nft_for_sale(nft_id: NftId) -> DispatchResult {
        Self::remove_listing_from_open_for_sale(&nft_id)?;
        <Nfts<T>>::mutate(nft_id, |nft| {
            nft.nonce += 1u64;
        });

        Ok(())
    }

    fn verify_signature(
        proof: &Proof<T::Signature, T::AccountId>,
        signed_payload: &[u8],
    ) -> Result<(), Error<T>> {
        match proof.signature.verify(signed_payload, &proof.signer) {
            true => Ok(()),
            false => Err(<Error<T>>::UnauthorizedTransaction.into()),
        }
    }

    fn get_dispatch_result_with_post_info(
        call: Box<<T as Config>::Call>,
    ) -> DispatchResultWithPostInfo {
        match call.is_sub_type() {
            Some(call) => {
                let final_weight = match call {
                    Call::signed_mint_single_nft(_, _, royalties, _) => {
                        T::WeightInfo::proxy_signed_mint_single_nft(
                            royalties.len().try_into().unwrap(),
                        )
                    }
                    Call::signed_list_nft_open_for_sale(_, _, _) => {
                        T::WeightInfo::proxy_signed_list_nft_open_for_sale()
                    }
                    Call::signed_transfer_fiat_nft(_, _, _) => {
                        T::WeightInfo::proxy_signed_transfer_fiat_nft()
                    }
                    Call::signed_cancel_list_fiat_nft(_, _) => {
                        T::WeightInfo::proxy_signed_cancel_list_fiat_nft()
                    }
                    _ => T::WeightInfo::proxy_signed_list_nft_open_for_sale().max(
                        T::WeightInfo::proxy_signed_mint_single_nft(MAX_NUMBER_OF_ROYALTIES),
                    ),
                };
                Ok(Some(final_weight).into())
            }
            None => Err(DispatchErrorWithPostInfo {
                error: Error::<T>::TransactionNotSupported.into(),
                post_info: PostDispatchInfo {
                    actual_weight: None, // None which stands for the worst case static weight
                    pays_fee: Default::default(),
                },
            }),
        }
    }

    fn encode_mint_single_nft_params(
        proof: &Proof<T::Signature, T::AccountId>,
        unique_external_ref: &Vec<u8>,
        royalties: &Vec<Royalty>,
        t1_authority: &H160,
    ) -> Vec<u8> {
        return (
            SIGNED_MINT_SINGLE_NFT_CONTEXT,
            &proof.relayer,
            unique_external_ref,
            royalties,
            t1_authority,
        )
            .encode();
    }

    fn encode_list_nft_for_sale_params(
        proof: &Proof<T::Signature, T::AccountId>,
        nft_id: &NftId,
        market: &NftSaleType,
    ) -> Vec<u8> {
        let nft = Self::nfts(nft_id);
        return (
            SIGNED_LIST_NFT_OPEN_FOR_SALE_CONTEXT,
            &proof.relayer,
            nft_id,
            market,
            nft.nonce,
        )
            .encode();
    }

    fn encode_transfer_fiat_nft_params(
        proof: &Proof<T::Signature, T::AccountId>,
        nft_id: &NftId,
        recipient: &H256,
    ) -> Vec<u8> {
        let nft = Self::nfts(nft_id);
        return (
            SIGNED_TRANSFER_FIAT_NFT_CONTEXT,
            &proof.relayer,
            nft_id,
            recipient,
            nft.nonce,
        )
            .encode();
    }

    fn encode_cancel_list_fiat_nft_params(
        proof: &Proof<T::Signature, T::AccountId>,
        nft_id: &NftId,
    ) -> Vec<u8> {
        let nft = Self::nfts(nft_id);
        return (
            SIGNED_CANCEL_LIST_FIAT_NFT_CONTEXT,
            &proof.relayer,
            nft_id,
            nft.nonce,
        )
            .encode();
    }

    fn get_encoded_call_param(
        call: &<T as Config>::Call,
    ) -> Option<(&Proof<T::Signature, T::AccountId>, Vec<u8>)> {
        let call = match call.is_sub_type() {
            Some(call) => call,
            None => return None,
        };

        match call {
            Call::signed_mint_single_nft(proof, external_ref, royalties, t1_authority) => {
                return Some((
                    proof,
                    Self::encode_mint_single_nft_params(
                        proof,
                        external_ref,
                        royalties,
                        t1_authority,
                    ),
                ))
            }
            Call::signed_list_nft_open_for_sale(proof, nft_id, market) => {
                return Some((
                    proof,
                    Self::encode_list_nft_for_sale_params(proof, nft_id, market),
                ))
            }
            Call::signed_transfer_fiat_nft(proof, nft_id, recipient) => {
                return Some((
                    proof,
                    Self::encode_transfer_fiat_nft_params(proof, nft_id, recipient),
                ))
            }
            Call::signed_cancel_list_fiat_nft(proof, nft_id) => {
                return Some((
                    proof,
                    Self::encode_cancel_list_fiat_nft_params(proof, nft_id),
                ))
            }
            Call::signed_create_batch(proof, total_supply, royalties, t1_authority) => {
                let sender_nonce = Self::batch_nonce(&proof.signer);
                return Some((
                    proof,
                    encode_create_batch_params::<T>(proof, royalties, t1_authority, total_supply, &sender_nonce),
                ))
            }
            Call::signed_mint_batch_nft(proof, batch_id, index, owner, unique_external_ref) => {
                return Some((
                    proof,
                    encode_mint_batch_nft_params::<T>(proof, batch_id, index, unique_external_ref, owner),
                ))
            }
            Call::signed_list_batch_for_sale(proof, batch_id, market) => {
                let sender_nonce = Self::batch_nonce(&proof.signer);
                return Some((
                    proof,
                    encode_list_batch_for_sale_params::<T>(proof, batch_id, market, &sender_nonce),
                ))
            }
            Call::signed_end_batch_sale(proof, batch_id) => {
                let sender_nonce = Self::batch_nonce(&proof.signer);
                return Some((
                    proof,
                    encode_end_batch_sale_params::<T>(proof, batch_id, &sender_nonce),
                ))
            }
            _ => return None,
        }
    }
}

impl<T: Config + ethereum_events::Config> ProcessedEventHandler for Module<T> {
    fn on_event_processed(event: &EthEvent) -> DispatchResult {
        return match &event.event_data {
            EventData::LogNftTransferTo(data) => Self::transfer_eth_nft(&event.event_id, data),
            EventData::LogNftCancelListing(data) => Self::cancel_eth_nft_listing(&event.event_id, data),
            EventData::LogNftMinted(data) => process_mint_batch_nft_event::<T>(&event.event_id, data),
            EventData::LogNftEndBatchListing(data) => process_end_batch_listing_event::<T>(&event.event_id, data),
            _ => Ok(()),
        };
    }
}

impl<T: Config> CallDecoder for Module<T> {
    type AccountId = T::AccountId;
    type Signature = <T as Config>::Signature;
    type Error = Error<T>;
    type Call = <T as Config>::Call;

    fn get_proof(
        call: &Self::Call,
    ) -> Result<Proof<Self::Signature, Self::AccountId>, Self::Error> {
        let call = match call.is_sub_type() {
            Some(call) => call,
            None => return Err(Error::TransactionNotSupported),
        };

        match call {
            Call::signed_mint_single_nft(
                proof,
                _unique_external_ref,
                _royalties,
                _t1_authority,
            ) => return Ok(proof.clone()),
            Call::signed_list_nft_open_for_sale(proof, _nft_id, _market) => {
                return Ok(proof.clone())
            }
            Call::signed_transfer_fiat_nft(proof, _nft_id, _t2_transfer_to_public_key) => {
                return Ok(proof.clone())
            }
            Call::signed_cancel_list_fiat_nft(proof, _nft_id) => return Ok(proof.clone()),
            Call::signed_create_batch(proof, _, _, _) => return Ok(proof.clone()),
            Call::signed_mint_batch_nft(proof, _, _, _, _) => return Ok(proof.clone()),
            Call::signed_list_batch_for_sale(proof, _, _) => return Ok(proof.clone()),
            Call::signed_end_batch_sale(proof, _) => return Ok(proof.clone()),
            _ => return Err(Error::TransactionNotSupported),
        }
    }
}

impl<T: Config> InnerCallValidator for Module<T> {
    type Call = <T as Config>::Call;

    fn signature_is_valid(call: &Box<Self::Call>) -> bool {
        if let Some((proof, signed_payload)) = Self::get_encoded_call_param(call) {
            return Self::verify_signature(&proof, &signed_payload.as_slice()).is_ok();
        }

        return false;
    }
}

// A value placed in storage that represents the current version of the Staking storage. This value
// is used by the `on_runtime_upgrade` logic to determine whether we run storage migration logic.
#[derive(Encode, Decode, Clone, Copy, PartialEq, Eq)]
enum Releases {
    Unknown,
    V2_0_0,
    V3_0_0,
}

impl Default for Releases {
    fn default() -> Self {
        Releases::Unknown
    }
}

pub mod migrations {
    use super::*;

    #[derive(Decode)]
    struct OldNftInfo {
        pub info_id: NftInfoId,
        pub batch_id: Option<NftBatchId>,
        pub royalties: Vec<Royalty>,
        pub total_supply: u64,
        pub t1_authority: H160,
    }

    impl OldNftInfo {
        fn upgraded<T: Config>(self) -> NftInfo<T::AccountId> {
            NftInfo {
                info_id: self.info_id,
                batch_id: self.batch_id,
                royalties: self.royalties,
                total_supply: self.total_supply,
                t1_authority: self.t1_authority,
                creator: None
            }
        }
    }

    pub fn migrate_to_batch_nft<T: Config>() -> frame_support::weights::Weight {
        frame_support::debug::RuntimeLogger::init();
        frame_support::debug::info!("ℹ️  Nft manager pallet data migration invoked");

        NftInfos::<T>::translate::<OldNftInfo, _>(|_, p| Some(p.upgraded::<T>()));

        frame_support::debug::info!("ℹ️  Migrated NftInfo data successfully");
        return T::BlockWeights::get().max_block;
    }
}


#[cfg(test)]
#[path = "tests/mock.rs"]
mod mock;

#[cfg(test)]
#[path = "../../avn/src/tests/extension_builder.rs"]
pub mod extension_builder;

#[cfg(test)]
#[path = "tests/single_mint_nft_tests.rs"]
pub mod single_mint_nft_tests;

#[cfg(test)]
#[path = "tests/open_for_sale_tests.rs"]
pub mod open_for_sale_tests;

#[cfg(test)]
#[path = "tests/proxy_signed_mint_single_nft_tests.rs"]
pub mod proxy_signed_mint_single_nft_tests;

#[cfg(test)]
#[path = "tests/proxy_signed_list_nft_open_for_sale_tests.rs"]
pub mod proxy_signed_list_nft_open_for_sale_tests;

#[cfg(test)]
#[path = "tests/proxy_signed_transfer_fiat_nft_tests.rs"]
pub mod proxy_signed_transfer_fiat_nft_tests;

#[cfg(test)]
#[path = "tests/proxy_signed_cancel_list_fiat_nft_tests.rs"]
pub mod proxy_signed_cancel_list_fiat_nft_tests;

#[cfg(test)]
#[path = "tests/transfer_to_tests.rs"]
pub mod transfer_to_tests;

#[cfg(test)]
#[path = "tests/cancel_single_nft_listing_tests.rs"]
pub mod cancel_single_nft_listing_tests;

#[cfg(test)]
#[path = "tests/batch_nft_tests.rs"]
pub mod batch_nft_tests;

mod benchmarking;
