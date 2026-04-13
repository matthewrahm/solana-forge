CREATE INDEX IF NOT EXISTS idx_swaps_block_time ON swaps(block_time DESC);
CREATE INDEX IF NOT EXISTS idx_swaps_signer ON swaps(signer);
CREATE INDEX IF NOT EXISTS idx_swaps_token_in ON swaps(token_in_mint);
CREATE INDEX IF NOT EXISTS idx_swaps_token_out ON swaps(token_out_mint);
CREATE INDEX IF NOT EXISTS idx_swaps_platform ON swaps(platform);

CREATE INDEX IF NOT EXISTS idx_transfers_block_time ON transfers(block_time DESC);
CREATE INDEX IF NOT EXISTS idx_transfers_from ON transfers(from_address);
CREATE INDEX IF NOT EXISTS idx_transfers_to ON transfers(to_address);
CREATE INDEX IF NOT EXISTS idx_transfers_mint ON transfers(mint);
