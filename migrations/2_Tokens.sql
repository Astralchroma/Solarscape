CREATE TYPE STATE AS ENUM ('valid', 'expired');

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
