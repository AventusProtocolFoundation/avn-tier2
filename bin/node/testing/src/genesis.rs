// This file is part of Substrate.

// Copyright (C) 2019-2021 Parity Technologies (UK) Ltd.
// SPDX-License-Identifier: GPL-3.0-or-later WITH Classpath-exception-2.0

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

//! Genesis Configuration.

use crate::keyring::*;
use sp_keyring::{Ed25519Keyring, Sr25519Keyring};
use node_runtime::{
	GenesisConfig, BalancesConfig, SessionConfig, StakingConfig, SystemConfig,
	GrandpaConfig, IndicesConfig, ContractsConfig, SocietyConfig, wasm_binary_unwrap,
	AccountId, StakerStatus, ValidatorsManagerConfig, EthereumEventsConfig, EthereumTransactionsConfig,
	TokenManagerConfig,
};
use node_primitives::BlockNumber;
use node_runtime::constants::currency::*;
use sp_core::{H160, H256, ChangesTrieConfiguration, Public};
use sp_runtime::Perbill;
use hex_literal::hex;

/// Create genesis runtime configuration for tests.
pub fn config(support_changes_trie: bool, code: Option<&[u8]>) -> GenesisConfig {
	config_endowed(support_changes_trie, code, Default::default())
}

/// Create genesis runtime configuration for tests with some extra
/// endowed accounts.
pub fn config_endowed(
	support_changes_trie: bool,
	code: Option<&[u8]>,
	extra_endowed: Vec<AccountId>,
) -> GenesisConfig {

	let mut endowed = vec![
		(alice(), 111 * DOLLARS),
		(bob(), 100 * DOLLARS),
		(charlie(), 100_000_000 * DOLLARS),
		(dave(), 111 * DOLLARS),
		(eve(), 101 * DOLLARS),
		(ferdie(), 100 * DOLLARS),
	];

	endowed.extend(
		extra_endowed.into_iter().map(|endowed| (endowed, 100*DOLLARS))
	);

	GenesisConfig {
		frame_system: Some(SystemConfig {
			changes_trie_config: if support_changes_trie { Some(ChangesTrieConfiguration {
				digest_interval: 2,
				digest_levels: 2,
			}) } else { None },
			code: code.map(|x| x.to_vec()).unwrap_or_else(|| wasm_binary_unwrap().to_vec()),
		}),
		pallet_indices: Some(IndicesConfig {
			indices: vec![],
		}),
		pallet_balances: Some(BalancesConfig {
			balances: endowed,
		}),
		pallet_validators_manager: Some(ValidatorsManagerConfig {
			//See bin/node/cli/src/chain_spec.rs for details on how these public keys are generated.
			validators: vec![
				(alice(), Public::from_slice(&hex!["03471b4c1012dddf4d494c506a098c7b1b719b20bbb177b1174f2166f953c29503"])),
				(bob(), Public::from_slice(&hex!["0292a73ad9488b934fd04cb31a0f50634841f7105a5b4a8538e4bfa06aa477bed6"])),
				(charlie(), Public::from_slice(&hex!["03c5527886d8e09ad1fededd3231f890685d2d5345385d54181269f80c8926ff8e"]))
			],
			min_validator_bond: 1000 * DOLLARS,
			validator_max_commission: Perbill::from_percent(50),
			min_user_bond: 10 * DOLLARS,
		}),
		pallet_ethereum_events: Some(EthereumEventsConfig {
			validator_manager_contract_address: H160::random(),
			lifting_contract_address: H160::random(),
			nft_t1_contracts: vec![],
			processed_events: vec![],
			lift_tx_hashes: vec![],
			quorum_factor: 3 as u32,
			event_challenge_period: 2 as BlockNumber,
		}),
		pallet_ethereum_transactions: Some(EthereumTransactionsConfig {
			get_publish_root_contract: H160::random(),
		}),
		pallet_token_manager: Some(TokenManagerConfig {
			lower_account_id: H256::random(),
			avt_token_contract: H160::random(),
		}),
		pallet_session: Some(SessionConfig {
			keys: vec![
				(dave(), alice(), to_session_keys(
					&Ed25519Keyring::Alice,
					&Sr25519Keyring::Alice,
				)),
				(eve(), bob(), to_session_keys(
					&Ed25519Keyring::Bob,
					&Sr25519Keyring::Bob,
				)),
				(ferdie(), charlie(), to_session_keys(
					&Ed25519Keyring::Charlie,
					&Sr25519Keyring::Charlie,
				)),
			]
		}),
		pallet_staking: Some(StakingConfig {
			stakers: vec![
				(dave(), alice(), 111 * DOLLARS, StakerStatus::Validator),
				(eve(), bob(), 100 * DOLLARS, StakerStatus::Validator),
				(ferdie(), charlie(), 100 * DOLLARS, StakerStatus::Validator)
			],
			validator_count: 3,
			minimum_validator_count: 0,
			slash_reward_fraction: Perbill::from_percent(10),
			invulnerables: vec![alice(), bob(), charlie()],
			.. Default::default()
		}),
		pallet_contracts: Some(ContractsConfig {
			current_schedule: Default::default(),
		}),
		pallet_babe: Some(Default::default()),
		pallet_grandpa: Some(GrandpaConfig {
			authorities: vec![],
		}),
		pallet_im_online: Some(Default::default()),
		pallet_authority_discovery: Some(Default::default()),
		pallet_democracy: Some(Default::default()),
		pallet_collective_Instance1: Some(Default::default()),
		pallet_collective_Instance2: Some(Default::default()),
		pallet_membership_Instance1: Some(Default::default()),
		pallet_elections_phragmen: Some(Default::default()),
		pallet_sudo: Some(Default::default()),
		pallet_treasury: Some(Default::default()),
		pallet_society: Some(SocietyConfig {
			members: vec![alice(), bob()],
			pot: 0,
			max_members: 999,
		}),
		pallet_vesting: Some(Default::default()),
		pallet_summary: Some(Default::default()),
	}
}
