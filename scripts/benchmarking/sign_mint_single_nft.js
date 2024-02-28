const helper = require('./helper.js');
const { cryptoWaitReady } = require('@polkadot/util-crypto');

async function main() {
  await cryptoWaitReady();

  const signing_context = 'authorization for mint single nft operation';

  const relayer = '0x0000000000000000000000000000000000000000000000000000000000000001';
  const signer = helper.get_signer("kiss mule sheriff twice make bike twice improve rate quote draw enough");

  const unique_external_ref = 'Offchain location of NFT';
  const royalties = [
    ['0x0000000000000000000000000000000000000002', 1000],
    ['0x0000000000000000000000000000000000000003', 500],
  ];
  const t1_authority = '0x0000000000000000000000000000000000000004';

  //-----------------------------------------------------------------------------------------------------------------
  let mint_single_nft_data = {
    context: signing_context,
    relayer: relayer,
    unique_external_ref: unique_external_ref,
    royalties: royalties,
    t1_authority: t1_authority
  };
  console.log("mint_single_nft_data: ", mint_single_nft_data);
  console.log();

  console.log("signer: ", signer.address);
  console.log();

  let encoded_data = helper.encode_signed_mint_single_nft_data(mint_single_nft_data);
  console.log("encoded_data:", encoded_data);
  console.log();

  let [mint_single_nft_data_signature, ] = helper.sign_data(signer, encoded_data);
  console.log('Signature: ', mint_single_nft_data_signature);
}

if (require.main === module) await main();
