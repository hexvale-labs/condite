ALTER TABLE "uploads" ADD COLUMN "owner_id" BLOB;
-- #[toasty::breakpoint]
CREATE INDEX "index_uploads_by_owner_id" ON "uploads" ("owner_id");
-- #[toasty::breakpoint]
CREATE TABLE "api_keys" (
    "id" BLOB NOT NULL,
    "digest" TEXT NOT NULL,
    "per_upload_quota" INTEGER NOT NULL,
    "storage_quota" INTEGER NOT NULL,
    "revoked" BOOLEAN NOT NULL,
    "lock_nonce" TEXT NOT NULL,
    "created_at" TEXT NOT NULL,
    PRIMARY KEY ("id")
);
-- #[toasty::breakpoint]
CREATE UNIQUE INDEX "index_api_keys_by_digest" ON "api_keys" ("digest");
