CREATE TABLE map_transfers (
    account_id BIGINT PRIMARY KEY,
    character_id INT UNSIGNED NOT NULL,
    auth_code INT UNSIGNED NOT NULL,
    sex TINYINT UNSIGNED NOT NULL,
    map_name VARCHAR(16) NOT NULL,
    expires_at DATETIME(6) NOT NULL,
    KEY map_transfer_expiry (expires_at),
    FOREIGN KEY (account_id) REFERENCES users(id) ON DELETE CASCADE,
    FOREIGN KEY (character_id) REFERENCES characters(id) ON DELETE CASCADE
) ENGINE=InnoDB;
