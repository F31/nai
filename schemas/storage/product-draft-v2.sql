CREATE TABLE product_draft (
 singleton INTEGER PRIMARY KEY CHECK(singleton=1),
 revision INTEGER NOT NULL CHECK(revision>0),
 published_revision INTEGER NOT NULL DEFAULT 0 CHECK(published_revision>=0),
 draft_json TEXT NOT NULL
);
PRAGMA user_version=2;
