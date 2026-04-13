CREATE TABLE IF NOT EXISTS swaps (
    id              BIGSERIAL PRIMARY KEY,
    signature       VARCHAR(88) NOT NULL UNIQUE,
    slot            BIGINT NOT NULL,
    block_time      TIMESTAMPTZ NOT NULL,
    fee_lamports    BIGINT NOT NULL,
    fee_payer       VARCHAR(44) NOT NULL,
    platform        VARCHAR(32) NOT NULL,
    signer          VARCHAR(44) NOT NULL,
    token_in_mint   VARCHAR(44) NOT NULL,
    token_in_amount BIGINT NOT NULL,
    token_in_decimals SMALLINT NOT NULL,
    token_out_mint  VARCHAR(44) NOT NULL,
    token_out_amount BIGINT NOT NULL,
    token_out_decimals SMALLINT NOT NULL,
    pool_address    VARCHAR(44),
    indexed_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS transfers (
    id              BIGSERIAL PRIMARY KEY,
    signature       VARCHAR(88) NOT NULL,
    slot            BIGINT NOT NULL,
    block_time      TIMESTAMPTZ NOT NULL,
    fee_lamports    BIGINT NOT NULL,
    fee_payer       VARCHAR(44) NOT NULL,
    mint            VARCHAR(44) NOT NULL,
    from_address    VARCHAR(44) NOT NULL,
    to_address      VARCHAR(44) NOT NULL,
    amount          BIGINT NOT NULL,
    decimals        SMALLINT NOT NULL,
    indexed_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(signature, mint, from_address, to_address)
);

CREATE TABLE IF NOT EXISTS tokens (
    mint            VARCHAR(44) PRIMARY KEY,
    symbol          VARCHAR(32),
    name            VARCHAR(128),
    decimals        SMALLINT NOT NULL DEFAULT 0,
    first_seen_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
