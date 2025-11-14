// Instruction modules for CCM Token-2022

pub mod admin;
pub mod channel;
pub mod claim;
pub mod cleanup;
pub mod cnft_verify;
pub mod governance;
pub mod hooks;
pub mod initialize_mint;
pub mod merkle;
pub mod merkle_ring;

// Re-exports (explicit to avoid namespace pollution)
pub use admin::{
    set_paused, set_paused_open, set_policy, set_policy_open, update_admin,
    update_admin_open, update_publisher, update_publisher_open, SetPaused,
    SetPausedOpen, SetPolicy, SetPolicyOpen, UpdateAdmin, UpdateAdminOpen,
    UpdatePublisher, UpdatePublisherOpen,
};
pub use channel::{set_channel_merkle_root, SetChannelMerkleRoot};
pub use claim::{
    claim, claim_open, compute_leaf, verify_proof, Claim, ClaimOpen,
};
pub use cleanup::{close_epoch_state, close_epoch_state_open, CloseEpochState, CloseEpochStateOpen};
pub use cnft_verify::{verify_cnft_receipt, verify_merkle_proof, CnftReceiptProof};
pub use governance::{
    update_fee_config, update_fee_config_open, UpdateFeeConfig, UpdateFeeConfigOpen,
};
pub use hooks::{transfer_hook, TransferHook, TransferObserved};
pub use initialize_mint::{handler, handler_open, InitializeMint, InitializeMintOpen};
pub use merkle::{set_merkle_root, set_merkle_root_open, SetMerkleRoot, SetMerkleRootOpen};
pub use merkle_ring::{
    initialize_channel, set_merkle_root_ring, InitializeChannel, SetMerkleRootRing,
};
