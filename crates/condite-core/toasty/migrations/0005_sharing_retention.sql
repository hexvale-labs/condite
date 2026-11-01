ALTER TABLE "uploads" ADD COLUMN "shared" BOOLEAN NOT NULL DEFAULT false;
-- #[toasty::breakpoint]
ALTER TABLE "uploads" ADD COLUMN "deletion_pending" BOOLEAN NOT NULL DEFAULT false;
-- #[toasty::breakpoint]
ALTER TABLE "uploads" ADD COLUMN "expires_at" TEXT;
-- #[toasty::breakpoint]
ALTER TABLE "uploads" ADD COLUMN "cleanup_at" TEXT;
-- #[toasty::breakpoint]
CREATE INDEX "index_uploads_by_cleanup_at" ON "uploads" ("cleanup_at");
-- #[toasty::breakpoint]
CREATE TABLE "shares" (
    "id" BLOB NOT NULL,
    "owner_id" BLOB NOT NULL,
    "upload_id" BLOB NOT NULL,
    "object_key" TEXT NOT NULL,
    "public_url" TEXT NOT NULL,
    "public_bucket" TEXT NOT NULL,
    "requested_expires_at" TEXT,
    "follows_upload" BOOLEAN NOT NULL,
    "status" TEXT NOT NULL,
    "cleanup_at" TEXT,
    "created_at" TEXT NOT NULL,
    PRIMARY KEY ("id")
);
-- #[toasty::breakpoint]
CREATE INDEX "index_shares_by_owner_id" ON "shares" ("owner_id");
-- #[toasty::breakpoint]
CREATE INDEX "index_shares_by_upload_id" ON "shares" ("upload_id");
-- #[toasty::breakpoint]
CREATE INDEX "index_shares_by_cleanup_at" ON "shares" ("cleanup_at");
-- #[toasty::breakpoint]
-- Existing files get a grace period measured from migration, not their old age.
UPDATE "uploads" SET "expires_at" = strftime('%Y-%m-%dT%H:%M:%SZ', 'now', '+14 days');
-- #[toasty::breakpoint]
UPDATE "uploads" SET "cleanup_at" = CASE WHEN "status" = 'confirmed'
    THEN "expires_at" ELSE strftime('%Y-%m-%dT%H:%M:%SZ', "created_at", '+1 hour') END;
