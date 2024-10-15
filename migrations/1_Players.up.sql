CREATE TABLE players (
	id            BIGINT       PRIMARY KEY,
	username      VARCHAR(32)  NOT NULL UNIQUE,

	-- Largest address SMTP will allow, though no sane person should have an address this long
	email         VARCHAR(254) NOT NULL UNIQUE,

	-- We don't really want a limit, however ideally we should have one, so let's
	-- just specify a limit that is big enough that it shouldn't be reached.
	phc_password  VARCHAR(256) NOT NULL
);
