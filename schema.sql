-- As keeping track of the Database Schema manually is frustraiting beyond a few migrations,
-- this file provides a combination of those migrations to be used as a programmer
-- reference, it should not be used for an actual database testing or otherwise.
--
-- Currently in line with: `1_Initial.sql`

CREATE TABLE players (
	id       BigInt       PRIMARY KEY,

	created  Timestamp    NOT NULL
	                      DEFAULT NOW(),

	-- Largest address SMTP will allow, though no sane person should have an address this long
	email    VarChar(254) NOT NULL
	                      UNIQUE,

	username VarChar(32)  NOT NULL
	                      UNIQUE,

	-- We don't want a limit, however it's dangerous to not put limits on things, so
	-- let's just specify a limit that is big enough that it shouldn't be reached.
	password VarChar(256) NOT NULL
);

CREATE TABLE tokens (
	player_id BigInt    REFERENCES players(id),

	created   Timestamp NOT NULL
	                    CHECK (used >= created)
	                    DEFAULT NOW(),

	used      Timestamp NOT NULL
	                    CHECK (used >= created)
	                    DEFAULT NOW(),

	-- 1 day is temporary as the client currently doesn't persist tokens across restarts
	valid     Boolean   NOT NULL
	                    GENERATED ALWAYS AS (used - created < '1 day') STORED,

	token     ByteA     PRIMARY KEY
);
