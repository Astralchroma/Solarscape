CREATE TYPE Item AS ENUM ('TestOre');

CREATE TABLE items (
	id      BigInt    PRIMARY KEY,

	created Timestamp NOT NULL
	                  DEFAULT NOW(),

	item    Item      NOT NULL
);

CREATE TABLE inventories (
	id      BigInt    PRIMARY KEY,

	created Timestamp NOT NULL
	                  DEFAULT NOW()
);

CREATE TABLE inventory_items (
	inventory_id BigInt REFERENCES inventories(id) ON DELETE CASCADE,
	item_id      BigInt REFERENCES items(id) ON DELETE CASCADE,

	PRIMARY KEY (inventory_id, item_id)
);

INSERT INTO inventories(id) SELECT id FROM players;
ALTER TABLE players ADD FOREIGN KEY (id) REFERENCES inventories(id) ON DELETE RESTRICT;
