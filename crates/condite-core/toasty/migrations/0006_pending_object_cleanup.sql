ALTER TABLE "uploads" ADD COLUMN "pending_cleanup_at" TEXT;
-- #[toasty::breakpoint]
CREATE INDEX "index_uploads_by_pending_cleanup_at" ON "uploads" ("pending_cleanup_at");
-- #[toasty::breakpoint]
UPDATE "uploads" SET "pending_cleanup_at" = strftime('%Y-%m-%dT%H:%M:%SZ', 'now', '+3 minutes') WHERE "status" = 'confirmed';
