// Copyright 2021 Aventus Network Services (UK) Ltd.

pub mod ethereum_converters {
    use sp_std::vec::Vec;
    pub fn into_32_be_bytes (bytes: &[u8]) -> Vec<u8> {
        let mut vec = Vec::new();
        vec.extend(bytes.iter().copied());
        vec.resize(32,0);
        vec.reverse();
        return vec;
    }
}