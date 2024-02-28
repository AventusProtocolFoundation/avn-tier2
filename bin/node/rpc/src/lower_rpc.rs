use std::sync::Arc;
use codec::Codec;
use jsonrpc_derive::rpc;
use sp_runtime::{traits::Block as BlockT};
use sc_client_api::{UsageProvider, client::BlockBackend};
use jsonrpc_core::{Error as RpcError, ErrorCode, Result};
use node_primitives::AccountId;

use crate::{extrinsic_utils::*};
// TODO [TYPE: refactoring][PRI: low]: Try replace this utils with merkle tree crate https://docs.rs/merkletree/0.21.0/merkletree/
use crate::{merkle_tree_utils::*};

/// Error type of this RPC api.
pub enum Error {
    DecodeError,
    ResponseError,
    InvalidExtrinsicInLocalDB,
    ErrorGettingBlockData,
    BlockDataNotFound,
    BlockNotFinalised,
    ErrorGeneratingRoot,
    LeafDataEmpty,
    EmptyLeaves,
}

impl From<Error> for i64 {
	fn from(e: Error) -> i64 {
	  match e {
            Error::DecodeError => 1,
            Error::ResponseError => 2,
            Error::InvalidExtrinsicInLocalDB => 3,
            Error::ErrorGettingBlockData => 4,
            Error::BlockDataNotFound => 5,
            Error::BlockNotFinalised => 6,
            Error::ErrorGeneratingRoot => 7,
            Error::LeafDataEmpty => 8,
            Error::EmptyLeaves => 9,
	  }
	}
}

#[rpc]
pub trait LowerDataProviderRpc {
    #[rpc(name = "lower_data")]
    fn get_lower_data(
        &self,
        from_block: u32,
        to_block: u32,
        block_number: u32,
        extrinsic_index: u32) -> Result<String>;
}

pub struct LowerDataProvider<C, Block> {
    client: Arc<C>,
    _marker: std::marker::PhantomData<Block>,
}

impl <C, Block> LowerDataProvider<C, Block> {
    pub fn new(client: Arc<C>) -> Self {
        Self { client, _marker: Default::default() }
    }
}

impl <C, Block> LowerDataProviderRpc for LowerDataProvider<C, Block>
where
	Block: BlockT,
    C: Send + Sync + 'static + BlockBackend<Block> + UsageProvider<Block>,
    AccountId: Clone + std::fmt::Display + Codec,
{
    fn get_lower_data(&self,
        from_block: u32,
        to_block: u32,
        block_number: u32,
        extrinsic_index: u32) -> Result<String>
    {
        let leaf_filter: LowerLeafFilter = LowerLeafFilter {
            block_number,
            extrinsic_index
        };

        let (encoded_leaf, extrinsics) = get_extrinsics_and_check_if_filter_target_exists(
            &self.client,
            from_block,
            to_block,
            leaf_filter)?;

        if extrinsics.len() > 0 && encoded_leaf.is_some() {
            let leaf = encoded_leaf.expect("Leaf exists");
            let merkle_path = generate_merkle_path(&leaf, extrinsics)?;
            let response = MerklePathData {
                encoded_leaf: leaf,
                merkle_path: merkle_path
            };

            return Ok(hex::encode(
                serde_json::to_string(&response).map_err(|e| RpcError {
                    code: ErrorCode::ServerError(Error::ResponseError.into()),
                    message: "Error converting response to string".into(),
                    data:  Some(format!("{:?}", e).into()),
                })?
            ));
        }

        // the leaf is missing or the filter values are incorrect
        Ok(hex::encode("".to_string()))
    }
}
