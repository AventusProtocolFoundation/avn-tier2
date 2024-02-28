use super::*;
use crate::benchmarking::*;
use sp_staking::SessionIndex;
use sp_runtime::traits::One;
use hex_literal::hex;
use hex::FromHex;
use frame_benchmarking::account;
use sp_core::ecdsa::Public;
use pallet_ethereum_transactions::{self as ethereum_transactions};

// TODO: replace with create_validators
pub fn setup_validators<T: Config>(number_of_validator_account_ids: u32) -> Vec<Validator<<T as pallet_avn::Config>::AuthorityId, T::AccountId>>
{
    let mnemonic: &str = "basic anxiety marine match castle rival moral whisper insane away avoid bike";
    let mut validators: Vec<Validator<<T as pallet_avn::Config>::AuthorityId, T::AccountId>> = Vec::new();
    for i in 0..number_of_validator_account_ids {
        let account = account("dummy_validator", i, i);
        //TODO: Move out of loop
        let key = <T as avn::Config>::AuthorityId::generate_pair(Some(mnemonic.as_bytes().to_vec()));
        validators.push(Validator::new(account, key));
    }

    // setup sender account id and key
    let sender_index = validators.len() - (1 as usize);
    let sender: Validator<T::AuthorityId, T::AccountId> = validators[sender_index].clone();
    let mut account_bytes: [u8; 32] = [0u8; 32];
    account_bytes.copy_from_slice(&hex!("f0110a85f7ac1e5877c8b0e2e950aa56f40e6cbb39b8bfd1b63018afeb1c7462"));
    let account_id = T::AccountId::decode(&mut &account_bytes.encode()[..]).unwrap();
    validators[sender_index] = Validator::new(account_id, sender.key);

    // setup resigner account id and key
    let resigner: Validator<T::AuthorityId, T::AccountId> = validators[1].clone();
    let mut resigner_account_bytes: [u8; 32] = [0u8; 32];
    resigner_account_bytes.copy_from_slice(&hex!("1ed1aadead9704b693af012a9f24e1f00dc7e2a0b4eb99f9e0bc0c35a8d20223"));
    let resigner_account_id = T::AccountId::decode(&mut &resigner_account_bytes.encode()[..]).unwrap();
    validators[1] = Validator::new(resigner_account_id, resigner.key);

    // Setup validators in avn pallet
    avn::Validators::<T>::put(validators.clone());

    // Setup validators in validators-manager pallet
    let validator_account_ids: Vec<T::AccountId> = validators.iter().map(|v| v.account_id.clone()).collect();
    ValidatorAccountIds::<T>::put(validator_account_ids);

    return validators;
}

pub fn generate_signature<T: pallet_avn::Config>() -> <<T as avn::Config>::AuthorityId as RuntimeAppPublic>::Signature {
    let encoded_data = 0.encode();
    let authority_id = T::AuthorityId::generate_pair(None);
    let signature = authority_id.sign(&encoded_data).expect("able to make signature");
    return signature;
}

pub fn generate_ecdsa_signature<T: pallet_avn::Config>(key: <T as pallet_avn::Config>::AuthorityId, msg: u64) -> ecdsa::Signature {
    let sr25519_signature= key.sign(&msg.encode()).expect("able to make signature").encode();

    let mut signature_bytes: [u8; 65] = [0u8; 65];
    let start = if sr25519_signature.len() <= 65 { 65 - sr25519_signature.len() } else { 0 };
    signature_bytes[start..].copy_from_slice(&sr25519_signature);

    return ecdsa::Signature::from_slice(&signature_bytes);
}

pub fn setup_action_voting<T: Config>(validators: Vec<Validator<<T as pallet_avn::Config>::AuthorityId, T::AccountId>>) -> (
    Validator<T::AuthorityId, T::AccountId>,
    ActionId<T::AccountId>,
    ecdsa::Signature,
    <T::AuthorityId as RuntimeAppPublic>::Signature,
    u32
) {
    let sender_index = validators.len() - (1 as usize);
    let sender: Validator<T::AuthorityId, T::AccountId> = validators[sender_index].clone();
    let action_account_id: T::AccountId = validators[1].account_id.clone();
    let ingress_counter: IngressCounter = 1;
    let action_id: ActionId<T::AccountId> = ActionId::new(action_account_id, ingress_counter);
    let approval_signature: ecdsa::Signature = ecdsa::Signature::from_slice(&hex!("2b01699be62c1aabaf0dd85f956567ac495d4293323ee1eb79d827d705ff86c80bdd4a26af6f50544af9510e0c21082b94ecb8a8d48d74ee4ebda6605a96d77901")).into();
    let signature: <T::AuthorityId as RuntimeAppPublic>::Signature = generate_signature::<T>();
    let quorum = setup_voting_session::<T>(&action_id);

    let eth_public_key = Public::from_raw(<[u8; 33]>::from_hex("0327471645ed3347c0123db4d97b6df8ae2fe1e1a6aed4afd0766e73a50b0f39e2").unwrap());
    EthereumPublicKeys::<T>::insert(eth_public_key.clone(), sender.account_id.clone());

    setup_action_data::<T>(sender.account_id.clone(), action_id.action_account_id.clone(), action_id.ingress_counter);

    (sender, action_id, approval_signature, signature, quorum)
}

pub fn setup_action_data<T: Config>(sender: T::AccountId, action_account_id: T::AccountId, ingress_counter: IngressCounter) {
    let eth_transaction_id: TransactionId = 0;
    let candidate_tx = EthTransactionType::DeregisterValidator(
        DeregisterValidatorData::new(<T as Config>::AccountToBytesConvert::into_bytes(&action_account_id))
    );

    ethereum_transactions::ReservedTransactions::insert(candidate_tx.clone(), 0u64);

    ValidatorActions::<T>::insert(
        action_account_id,
        ingress_counter,
        ValidatorsActionData::new(
            ValidatorsActionStatus::AwaitingConfirmation,
            sender,
            eth_transaction_id,
            ValidatorsActionType::Resignation,
            candidate_tx
        )
    )
}

pub fn setup_voting_session<T: Config>(action_id: &ActionId<T::AccountId>) -> u32 {
    PendingApprovals::<T>::insert(
        action_id.action_account_id.clone(),
        action_id.ingress_counter
    );

    let quorum = calculate_two_third_quorum(AVN::<T>::validators().len() as u32);
    let voting_period_end = safe_add_block_numbers(
        <system::Module<T>>::block_number(),
        T::VotingPeriod::get()
    );
    VotesRepository::<T>::insert(
        action_id,
        VotingSessionData::<T::AccountId, T::BlockNumber>::new(action_id.encode(), quorum, voting_period_end.expect("already checked"), 0u32.into()),
    );

    return quorum;
}

pub fn setup_approval_votes<T: Config>(
    validators: &Vec<Validator<<T as pallet_avn::Config>::AuthorityId, T::AccountId>>,
    number_of_votes: u32,
    action_id: &ActionId<T::AccountId>
) {
    setup_votes::<T>(validators, number_of_votes, action_id, true);
}

pub fn setup_reject_votes<T: Config>(
    validators: &Vec<Validator<<T as pallet_avn::Config>::AuthorityId, T::AccountId>>,
    number_of_votes: u32,
    action_id: &ActionId<T::AccountId>
) {
    setup_votes::<T>(validators, number_of_votes, action_id, false);
}

pub fn setup_votes<T: Config>(
    validators: &Vec<Validator<<T as pallet_avn::Config>::AuthorityId, T::AccountId>>,
    number_of_votes: u32,
    action_id: &ActionId<T::AccountId>,
    is_approval: bool
) {
    for i in 0 .. validators.len() {
        if i < (number_of_votes as usize) {
            let approval_signature: ecdsa::Signature = generate_ecdsa_signature::<T>(validators[i].key.clone(), i as u64);
            match is_approval {
                true => VotesRepository::<T>::mutate(action_id, |vote| {
                    vote.ayes.push(validators[i].account_id.clone());
                    vote.confirmations.push(approval_signature.clone());
                }),
                false => VotesRepository::<T>::mutate(action_id, |vote|
                    vote.nays.push(validators[i].account_id.clone())
                )
            }
        }
    }
}


// create `max` validators.
pub fn create_validators<T: Config>(max: u32) -> Result<Vec<<T::Lookup as StaticLookup>::Source>, &'static str>
{
    let mnemonic: &str = "basic anxiety marine match castle rival moral whisper insane away avoid bike";
    let key = <T as avn::Config>::AuthorityId::generate_pair(Some(mnemonic.as_bytes().to_vec()));
    let mut avn_validators: Vec<Validator<<T as pallet_avn::Config>::AuthorityId, T::AccountId>> = Vec::new();
	let mut validators: Vec<<T::Lookup as StaticLookup>::Source> = Vec::with_capacity(max as usize);
    let mut validator_account_ids: Vec<T::AccountId> = Vec::new();

	for i in 0 .. max {
		let (stash, controller) = create_bonded_user::<T>(i, RewardDestination::Staked)?;
		let validator_prefs = ValidatorPrefs {
			commission: Perbill::from_percent(50),
			.. Default::default()
		};
		Staking::<T>::validate(RawOrigin::Signed(controller).into(), validator_prefs)?;
		let stash_lookup: <T::Lookup as StaticLookup>::Source = T::Lookup::unlookup(stash.clone());
		validators.push(stash_lookup);
        avn_validators.push(Validator::new(stash.clone(), key.clone()));
        validator_account_ids.push(stash);
	}

    // Setup validators in avn pallet
    avn::Validators::<T>::put(avn_validators);

    // Setup validators in validators-manager pallet
    ValidatorAccountIds::<T>::put(validator_account_ids);

    Ok(validators)
}

// This function clears all existing validators and nominators from the set, and generates new
// "validators_count" validators being nominated by "nominators_count" nominators, and returns the vector of validator stash
// accounts and the vetor of nominators for all the validators. Each nominator nominates ALL validators.
// It also starts an era and creates pending payouts.
pub fn create_validators_with_nominators_for_era<T: Config>(validators_count: u32, nominators_count: u32)
    -> Result<(Vec<<T::Lookup as StaticLookup>::Source>, Vec<(T::AccountId, T::AccountId)>), &'static str>
{
	// Clean up any existing state.
	pallet_staking::Validators::<T>::remove_all();
	pallet_staking::Nominators::<T>::remove_all();

    let mnemonic: &str = "basic anxiety marine match castle rival moral whisper insane away avoid bike";
    let key = <T as avn::Config>::AuthorityId::generate_pair(Some(mnemonic.as_bytes().to_vec()));
	let mut validators_stash: Vec<<T::Lookup as StaticLookup>::Source> = Vec::with_capacity(validators_count as usize);
    let mut validator_account_ids = Vec::with_capacity(validators_count as usize);
    let mut validator_authority_ids  = Vec::with_capacity(validators_count as usize);

	// Create validators
	for i in 0 .. validators_count {
		let (v_stash, v_controller) = create_bonded_user::<T>(i, RewardDestination::Stash)?;
		let validator_prefs = ValidatorPrefs {
			commission: Perbill::from_percent(50),
			.. Default::default()
		};
		Staking::<T>::validate(RawOrigin::Signed(v_controller.clone()).into(), validator_prefs)?;
		let stash_lookup: <T::Lookup as StaticLookup>::Source = T::Lookup::unlookup(v_stash.clone());
		validators_stash.push(stash_lookup.clone());
        validator_account_ids.push(v_stash.clone());
        validator_authority_ids.push(Validator::new(v_stash.clone(), key.clone()))
	}

    // Setup validators in avn pallet
    avn::Validators::<T>::put(validator_authority_ids);
    // Setup validators in validators-manager pallet
    ValidatorAccountIds::<T>::put(validator_account_ids);

    // Create nominators
    let mut nominators = Vec::new();
    for j in 0 .. nominators_count {
        let (n_stash, n_controller) = create_bonded_user::<T>(u32::max_value() - j, RewardDestination::Stash)?;
        //let (n_stash, n_controller) = create_stash_and_dead_controller::<T>(u32::max_value() - j, RewardDestination::Controller)?;
        Staking::<T>::nominate(RawOrigin::Signed(n_controller.clone()).into(), validators_stash.clone())?;
        nominators.push((n_stash, n_controller));
    }

    pallet_staking::ValidatorCount::put(validators_count);

    // Start a new Era
    let new_validators = Staking::<T>::new_era(SessionIndex::one()).unwrap();
    // Set active era
    Staking::<T>::start_era(SessionIndex::one());

    // Make sure all the validators have been selected for the given era
    assert!(new_validators.len() == validators_stash.len());

    Ok((validators_stash, nominators))
}

pub fn create_known_validator<T: Config>() -> Result<T::AccountId, &'static str> {
    let mnemonic: &str = "basic anxiety marine match castle rival moral whisper insane away avoid bike";
    let key = <T as avn::Config>::AuthorityId::generate_pair(Some(mnemonic.as_bytes().to_vec()));

    let (_, controller) = create_bonded_staker::<T>(Default::default())?;
    let validator_prefs = ValidatorPrefs {
        commission: Perbill::from_percent(5),
        .. Default::default()
    };

    Staking::<T>::validate(RawOrigin::Signed(controller.clone()).into(), validator_prefs)?;

    // Setup validators in avn pallet
    avn::Validators::<T>::put(vec![Validator::new(controller.clone(), key)]);

    // Setup validators in validators-manager pallet
    ValidatorAccountIds::<T>::put(vec![controller.clone()]);

    Ok(controller)
}