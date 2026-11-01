CREATE TABLE "object_cleanup" (
    "id" BLOB NOT NULL,
    "bucket" TEXT NOT NULL,
    "pending_key" TEXT NOT NULL,
    "confirmed_key" TEXT NOT NULL,
    "cleanup_at" TEXT NOT NULL,
    PRIMARY KEY ("id")
);
-- #[toasty::breakpoint]
CREATE INDEX "index_object_cleanup_by_cleanup_at" ON "object_cleanup" ("cleanup_at");
