-- As keeping track of the Database Schema manually is frustraiting beyond a few migrations,
-- this file provides a combination of those migrations to be used as a programmer
-- reference, it should not be used for an actual database testing or otherwise.
--
-- Currently in line with: `3_Add_Player_Creation_Date.sql`

CREATE TYPE STATE AS ENUM ('valid', 'expired');

CREATE TABLE players (
	id           BIGINT       PRIMARY KEY,
	username     VARCHAR(32)  NOT NULL UNIQUE,

	-- Largest address SMTP will allow, though no sane person should have an address this long
	email        VARCHAR(254) NOT NULL UNIQUE,

	-- We don't really want a limit, however ideally we should have one, so let's
	-- just specify a limit that is big enough that it shouldn't be reached.
	phc_password VARCHAR(256) NOT NULL,

	created      TIMESTAMP    NOT NULL DEFAULT 'now'
);

CREATE TABLE tokens (
	token   BYTEA     PRIMARY KEY,
	player  BIGINT,

	created TIMESTAMP NOT NULL CHECK (used >= created) DEFAULT 'now',
	used    TIMESTAMP NOT NULL CHECK (used >= created) DEFAULT 'now',
	
	-- 1 day is temporary as the client currently doesn't persist tokens across restarts
	state   STATE     NOT NULL
		GENERATED ALWAYS AS (CASE WHEN (used - created > '1 day')
			THEN STATE('expired')
			ELSE STATE('valid')
		END) STORED,
	
	FOREIGN KEY (player) REFERENCES players(id) ON DELETE SET NULL
);
