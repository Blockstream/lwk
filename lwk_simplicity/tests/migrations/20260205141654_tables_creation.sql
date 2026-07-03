-- Migrations are copied from https://github.com/BlockstreamResearch/simplicity-lending/tree/main/crates/indexer/migrations
CREATE TABLE sync_state (
    id INTEGER PRIMARY KEY DEFAULT 1,
    last_indexed_height BIGINT NOT NULL,
    last_indexed_hash TEXT NOT NULL,
    updated_at timestamptz DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT single_row CHECK (id = 1)
);

CREATE TYPE factory_status AS ENUM (
    'active',
    'removed'
);

CREATE TABLE factories (
    id uuid NOT NULL,
    PRIMARY KEY (id),
    factory_asset_id BYTEA NOT NULL,
    program_script_pubkey BYTEA NOT NULL,
    issuing_utxos_count SMALLINT NOT NULL,
    reissuance_flags BIGINT NOT NULL,
    current_status factory_status NOT NULL DEFAULT 'active',
    created_at_height BIGINT NOT NULL,
    created_at_txid BYTEA NOT NULL UNIQUE
);

CREATE TABLE factory_utxos (
    factory_id uuid NOT NULL REFERENCES factories(id) ON DELETE CASCADE,

    txid BYTEA NOT NULL,
    vout INTEGER NOT NULL,
    created_at_height BIGINT NOT NULL,

    spent_txid BYTEA,
    spent_at_height BIGINT,

    PRIMARY KEY (txid, vout)
);

CREATE UNIQUE INDEX idx_factory_utxos_one_active_per_factory
ON factory_utxos (factory_id)
WHERE spent_txid IS NULL;

CREATE TABLE factory_auths (
    factory_id uuid NOT NULL REFERENCES factories(id) ON DELETE CASCADE,
    script_pubkey BYTEA NOT NULL,

    txid BYTEA NOT NULL,
    vout INTEGER NOT NULL,
    created_at_height BIGINT NOT NULL,

    spent_txid BYTEA,
    spent_at_height BIGINT,

    PRIMARY KEY (txid, vout)
);

CREATE INDEX idx_factory_auths_script_pubkey_active
ON factory_auths (script_pubkey)
WHERE spent_txid IS NULL;

CREATE TYPE offer_status AS ENUM (
    'pending',
    'active',
    'repaid',
    'liquidated',
    'cancelled',
    'claimed'
);

CREATE TABLE offers (
    id uuid NOT NULL,
    PRIMARY KEY (id),
    issuance_factory_id uuid NOT NULL REFERENCES factories(id) ON DELETE CASCADE,
    collateral_asset_id BYTEA NOT NULL,
    principal_asset_id BYTEA NOT NULL,
    borrower_nft_asset_id BYTEA NOT NULL,
    lender_nft_asset_id BYTEA NOT NULL,
    protocol_fee_keeper_asset_id BYTEA NOT NULL,
    collateral_amount BIGINT NOT NULL,
    principal_amount BIGINT NOT NULL,
    interest_rate INTEGER NOT NULL,
    loan_expiration_time INTEGER NOT NULL,
    current_status offer_status NOT NULL DEFAULT 'pending',
    created_at_height BIGINT NOT NULL,
    created_at_txid BYTEA NOT NULL UNIQUE
);

CREATE TYPE utxo_type AS ENUM (
    'pending_offer',
    'active_offer',
    'cancellation',
    'repayment',
    'liquidation',
    'claim'
);

CREATE TABLE offer_utxos (
    offer_id uuid NOT NULL REFERENCES offers(id) ON DELETE CASCADE,
    utxo_type utxo_type NOT NULL DEFAULT 'pending_offer',

    txid BYTEA NOT NULL,
    vout INTEGER NOT NULL,
    created_at_height BIGINT NOT NULL,

    spent_txid BYTEA,
    spent_at_height BIGINT,

    PRIMARY KEY (txid, vout)
);

CREATE INDEX idx_offer_utxos_unspent 
ON offer_utxos (txid, vout) 
WHERE spent_txid IS NULL;

CREATE TYPE participant_type AS ENUM (
    'borrower',
    'lender'
);

CREATE TABLE offer_participants (
    offer_id uuid NOT NULL REFERENCES offers(id) ON DELETE CASCADE,
    participant_type participant_type NOT NULL,
    script_pubkey BYTEA NOT NULL,

    txid BYTEA NOT NULL,
    vout INTEGER NOT NULL,
    created_at_height BIGINT NOT NULL,

    spent_txid BYTEA,
    spent_at_height BIGINT,

    PRIMARY KEY (txid, vout)
);

CREATE INDEX idx_participants_current_owner 
ON offer_participants(script_pubkey) 
WHERE spent_txid IS NULL;
