-- Create new books table without user_id requirement
CREATE TABLE IF NOT EXISTS books_new (
    id TEXT PRIMARY KEY,
    title TEXT NOT NULL,
    author TEXT,
    isbn TEXT,
    publication_year INTEGER,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

-- Copy existing data
INSERT INTO books_new (id, title, author, isbn, publication_year, created_at, updated_at)
SELECT id, title, author, isbn, publication_year, created_at, updated_at FROM books;

-- Drop old table and rename
DROP TABLE books;
ALTER TABLE books_new RENAME TO books;
