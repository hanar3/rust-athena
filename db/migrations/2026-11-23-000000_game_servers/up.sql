ALTER TABLE users
    ADD UNIQUE KEY unique_username (username),
    ADD COLUMN sex TINYINT UNSIGNED NOT NULL DEFAULT 1,
    AUTO_INCREMENT = 2000000;

CREATE TABLE characters (
    id INT UNSIGNED AUTO_INCREMENT PRIMARY KEY,
    account_id BIGINT NOT NULL,
    slot TINYINT UNSIGNED NOT NULL,
    name VARCHAR(24) NOT NULL,
    job SMALLINT NOT NULL,
    hair SMALLINT NOT NULL,
    hair_color SMALLINT NOT NULL,
    sex TINYINT UNSIGNED NOT NULL,
    map_name VARCHAR(16) NOT NULL,
    CHECK (slot < 15),
    UNIQUE KEY unique_character_slot (account_id, slot),
    UNIQUE KEY unique_character_name (name),
    FOREIGN KEY (account_id) REFERENCES users(id) ON DELETE CASCADE
) ENGINE=InnoDB AUTO_INCREMENT=150000;

CREATE TABLE login_handoffs (
    account_id BIGINT PRIMARY KEY,
    login_id1 INT UNSIGNED NOT NULL,
    login_id2 INT UNSIGNED NOT NULL,
    sex TINYINT UNSIGNED NOT NULL,
    expires_at DATETIME(6) NOT NULL,
    KEY handoff_expiry (expires_at),
    FOREIGN KEY (account_id) REFERENCES users(id) ON DELETE CASCADE
) ENGINE=InnoDB;
