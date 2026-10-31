PRAGMA foreign_keys = OFF;
-- #[toasty::breakpoint]
CREATE TABLE "_toasty_new_uploads" (
    "id" BLOB NOT NULL,
    "filename" TEXT NOT NULL,
    "mime_type" TEXT NOT NULL,
    "size_bytes" INTEGER NOT NULL,
    "status" TEXT NOT NULL,
    "created_at" TEXT NOT NULL,
    PRIMARY KEY ("id")
);
-- #[toasty::breakpoint]
INSERT INTO "_toasty_new_uploads" ("id", "filename", "mime_type", "size_bytes", "status", "created_at") SELECT "id", "filename", "mime_type", "size_bytes", "status", "created_at" FROM "uploads";
-- #[toasty::breakpoint]
DROP TABLE "uploads";
-- #[toasty::breakpoint]
ALTER TABLE "_toasty_new_uploads" RENAME TO "uploads";
-- #[toasty::breakpoint]
PRAGMA foreign_keys = ON;
