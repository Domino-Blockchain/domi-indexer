CREATE TABLE inscriptions (
    slot BIGINT NOT NULL,
    signature TEXT NOT NULL,
    account TEXT NOT NULL,
    mint_account TEXT,
    metadata_account TEXT,
    authority TEXT NOT NULL,
    updated_on TIMESTAMP NOT NULL,
    CONSTRAINT inscriptions_pk PRIMARY KEY (account)
);
