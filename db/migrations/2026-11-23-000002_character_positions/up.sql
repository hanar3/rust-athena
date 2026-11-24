ALTER TABLE characters
    ADD COLUMN last_x SMALLINT UNSIGNED NULL,
    ADD COLUMN last_y SMALLINT UNSIGNED NULL,
    ADD CONSTRAINT valid_last_position CHECK (
        (last_x IS NULL AND last_y IS NULL) OR
        (last_x IS NOT NULL AND last_y IS NOT NULL AND last_x < 1024 AND last_y < 1024)
    );
