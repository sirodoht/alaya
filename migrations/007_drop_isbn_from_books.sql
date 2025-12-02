-- Drop isbn column from books table
-- SQLite doesn't support DROP COLUMN directly, so we need to recreate the table

CREATE TABLE books_new (
    id TEXT PRIMARY KEY,
    title TEXT NOT NULL,
    author TEXT,
    publication_year INTEGER,
    filepath TEXT,
    notes TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

INSERT INTO books_new (id, title, author, publication_year, filepath, notes, created_at, updated_at)
SELECT id, title, author, publication_year, filepath, notes, created_at, updated_at FROM books;

DROP TABLE books;

ALTER TABLE books_new RENAME TO books;
