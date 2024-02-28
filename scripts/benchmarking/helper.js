const { hexToU8a, u8aToHex, u8aConcat } = require('@polkadot/util');
const { TypeRegistry } = require('@polkadot/types');
const {Keyring} = require('@polkadot/keyring');
const { blake2AsU8a, decodeAddress } = require('@polkadot/util-crypto');

const registry = new TypeRegistry();

function get_seed(accountName, index, seed) {
  const encodedName = registry.createType('Text', accountName);
  const encodedIndex = registry.createType('u32', index);
  const encodedSeed = registry.createType('u32', seed);
  const encoded_data = u8aConcat(
    encodedName.toU8a(false),
    encodedIndex.toU8a(true),
    encodedSeed.toU8a(true)
  );

  return blake2AsU8a(encoded_data);
}

function encode_signed_transfer_signature_data(params) {
    const context = registry.createType('Text', params.context);
    const relayer = registry.createType('AccountId', hexToU8a(params.relayer));
    const from = registry.createType('AccountId', hexToU8a(params.from));
    const to = registry.createType('AccountId', hexToU8a(params.to));
    const token = registry.createType('H160', hexToU8a(params.token));
    const amount = registry.createType('u128', params.amount);
    const nonce = registry.createType('u64', params.nonce);

    const encoded_params = u8aConcat(
      context.toU8a(false),
      relayer.toU8a(true),
      from.toU8a(true),
      to.toU8a(true),
      token.toU8a(true),
      amount.toU8a(true),
      nonce.toU8a(true)
    );

    let result = u8aToHex(encoded_params);

    return result;
}

function encode_signed_lower_signature_data(params) {
  const context = registry.createType('Text', params.context);
  const relayer = registry.createType('AccountId', hexToU8a(params.relayer));
  const from = registry.createType('AccountId', hexToU8a(params.from));
  const token = registry.createType('H160', hexToU8a(params.token));
  const amount = registry.createType('u128', params.amount);
  const t1_recipient = registry.createType('H160', hexToU8a(params.t1_recipient));
  const nonce = registry.createType('u64', params.nonce);

  const encoded_params = u8aConcat(
    context.toU8a(false),
    relayer.toU8a(true),
    from.toU8a(true),
    token.toU8a(true),
    amount.toU8a(true),
    t1_recipient.toU8a(true),
    nonce.toU8a(true)
  );

  let result = u8aToHex(encoded_params);

  return result;
}

function encode_signed_mint_single_nft_data(params) {
  const context = registry.createType('Text', params.context);
  const relayer = registry.createType('AccountId', hexToU8a(params.relayer));
  const unique_external_ref = registry.createType('Vec<u8>', params.unique_external_ref);
  const royalties = registry.createType('Vec<(H160, u32)>', royaltiesToU8a(params.royalties));
  const t1_authority = registry.createType('H160', hexToU8a(params.t1_authority));

  const encoded_params = u8aConcat(
    context.toU8a(false),
    relayer.toU8a(true),
    unique_external_ref.toU8a(false),
    royalties.toU8a(false),
    t1_authority.toU8a(true)
  );

  return u8aToHex(encoded_params);
}

function royaltiesToU8a(royalties) {
  let result = [];
  royalties.forEach(function(royalty){
    let recipient_t1_address = hexToU8a(royalty[0]);
    let rate = registry.createType('u32', royalty[1]).toU8a(true);
    result.push(u8aConcat(recipient_t1_address, rate));
  });
  return result;
}

function encode_signed_list_nft_open_for_sale_data(params) {
  const context = registry.createType('Text', params.context);
  const relayer = registry.createType('AccountId', hexToU8a(params.relayer));
  const nft_id = registry.createType('U256', params.nft_id);
  const market = registry.createType('u8', hexToU8a(params.market));
  const nonce = registry.createType('u64', params.nonce);

  const encoded_params = u8aConcat(
    context.toU8a(false),
    relayer.toU8a(true),
    nft_id.toU8a(true),
    market.toU8a(true),
    nonce.toU8a(true)
  );

  let result = u8aToHex(encoded_params);

  return result;
}

function encode_signed_transfer_fiat_nft_data(params) {
  const context = registry.createType('Text', params.context);
  const relayer = registry.createType('AccountId', hexToU8a(params.relayer));
  const nft_id = registry.createType('U256', params.nft_id);
  const t2_transfer_to_public_key = registry.createType('H256', hexToU8a(params.t2_transfer_to_public_key));
  const nonce = registry.createType('u64', params.nonce);

  const encoded_params = u8aConcat(
    context.toU8a(false),
    relayer.toU8a(true),
    nft_id.toU8a(true),
    t2_transfer_to_public_key.toU8a(true),
    nonce.toU8a(true)
  );

  let result = u8aToHex(encoded_params);

  return result;
}

function encode_signed_cancel_list_fiat_nft_data(params) {
  const context = registry.createType('Text', params.context);
  const relayer = registry.createType('AccountId', hexToU8a(params.relayer));
  const nft_id = registry.createType('U256', params.nft_id);
  const nonce = registry.createType('u64', params.nonce);

  const encoded_params = u8aConcat(
    context.toU8a(false),
    relayer.toU8a(true),
    nft_id.toU8a(true),
    nonce.toU8a(true)
  );

  let result = u8aToHex(encoded_params);

  return result;
}

function encode_bond_extra_signature_data(params) {
    const context = registry.createType('Text', params.context);
    const relayer = registry.createType('AccountId', hexToU8a(params.relayer));
    const amount = registry.createType('BalanceOf', params.amount);
    const nonce = registry.createType('u64', params.nonce);

    const encoded_params = u8aConcat(
      context.toU8a(false),
      relayer.toU8a(true),
      amount.toU8a(true),
      nonce.toU8a(true)
    );

    return u8aToHex(encoded_params);
}

function encode_nominate_signature_data(params) {
    const context = registry.createType('Text', params.context);
    const relayer = registry.createType('AccountId', hexToU8a(params.relayer));
    const targets = registry.createType('Vec<LookupSource>', params.targets);
    const nonce = registry.createType('u64', params.nonce);

    const encoded_params = u8aConcat(
      context.toU8a(false),
      relayer.toU8a(true),
      targets.toU8a(false),
      nonce.toU8a(true)
    );

    return u8aToHex(encoded_params);
}

function encode_bond_signature_data(params) {
    const context = registry.createType('Text', params.context);
    const relayer = registry.createType('AccountId', hexToU8a(params.relayer));
    const controller = registry.createType('LookupSource', hexToU8a(params.controller));
    const amount = registry.createType('BalanceOf', params.amount);
    const payee = registry.createType('RewardDestination', params.payee);
    const nonce = registry.createType('u64', params.nonce);

    const encoded_params = u8aConcat(
      context.toU8a(false),
      relayer.toU8a(true),
      controller.toU8a(true), // set to `toU8a(false)` to generate benchmark test compatible signature
      amount.toU8a(true),
      payee.toU8a(false),
      nonce.toU8a(true)
    );

    return u8aToHex(encoded_params);
}

function encode_unbond_signature_data(params) {
    const context = registry.createType('Text', params.context);
    const relayer = registry.createType('AccountId', params.relayer);
    const amount = registry.createType('BalanceOf', params.amount);
    const nonce = registry.createType('u64', params.nonce);

    const encoded_params = u8aConcat(
        context.toU8a(false),
        relayer.toU8a(true),
        amount.toU8a(true),
        nonce.toU8a(true)
    );

    return u8aToHex(encoded_params);
}

function encode_payout_stakers_signature_data(params) {
    const context = registry.createType('Text', params.context);
    const relayer = registry.createType('AccountId', params.relayer);
    const eraIndex = registry.createType('EraIndex', params.eraIndex);
    const nonce = registry.createType('u64', params.nonce);

    const encoded_params = u8aConcat(
        context.toU8a(false),
        relayer.toU8a(true),
        eraIndex.toU8a(true),
        nonce.toU8a(true)
    );

    return u8aToHex(encoded_params);
}

function encode_set_payee_signature_data(params) {
    const context = registry.createType('Text', params.context);
    const relayer = registry.createType('AccountId', params.relayer);
    const payee = registry.createType('RewardDestination', params.payee);
    const nonce = registry.createType('u64', params.nonce);

    const encoded_params = u8aConcat(
        context.toU8a(false),
        relayer.toU8a(true),
        payee.toU8a(false),
        nonce.toU8a(true)
    );

    return u8aToHex(encoded_params);
}

function encode_signed_add_ethereum_log_data(params) {
    const context = registry.createType('Text', params.context);
    const relayer = registry.createType('AccountId', params.relayer);
    const event_type = registry.createType('u8', hexToU8a(params.event_type));
    const tx_hash = registry.createType('H256', hexToU8a(params.tx_hash));
    const nonce = registry.createType('u64', params.sender_nonce);

    const encoded_params = u8aConcat(
        context.toU8a(false),
        relayer.toU8a(true),
        event_type.toU8a(true),
        tx_hash.toU8a(true),
        nonce.toU8a(true)
    );

    return u8aToHex(encoded_params);
}

function get_signer(signer_suri) {
  let keyring = new Keyring({ type: 'sr25519' });
  return keyring.addFromUri(signer_suri);
}

function get_address_from_bytes(accountName, index, seed) {
  let entropy = get_seed(accountName, index, seed);
  let result = decodeAddress(entropy, false);
  return result;
}

function sign_data(signer, encoded_data) {
  let signature = u8aToHex(signer.sign(encoded_data));
  return [signature, signer];
}

module.exports = {
    encode_signed_transfer_signature_data,
    encode_signed_lower_signature_data,
    encode_signed_mint_single_nft_data,
    encode_signed_list_nft_open_for_sale_data,
    encode_signed_transfer_fiat_nft_data,
    encode_signed_cancel_list_fiat_nft_data,
    encode_bond_extra_signature_data,
    encode_bond_signature_data,
    encode_nominate_signature_data,
    encode_unbond_signature_data,
    encode_payout_stakers_signature_data,
    encode_set_payee_signature_data,
    encode_signed_add_ethereum_log_data,
    get_signer,
    get_address_from_bytes,
    sign_data,
};
