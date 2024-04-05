CREATE TABLE inscriptions (
    slot BIGINT NOT NULL,
    signature TEXT NOT NULL,
    account TEXT NOT NULL,
    metadata_account TEXT NOT NULL,
    authority TEXT NOT NULL,
    data BYTEA,
    write_version BIGINT,
    updated_on TIMESTAMP NOT NULL,
    CONSTRAINT inscriptions_pk PRIMARY KEY (account)
);
