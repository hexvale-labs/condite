CREATE TABLE "sessions" (
    "digest" TEXT NOT NULL,
    "owner_id" BLOB NOT NULL,
    "credential_digest" TEXT NOT NULL,
    "root" BOOLEAN NOT NULL,
    "upload_id" BLOB,
    "expires_at" TEXT NOT NULL,
    PRIMARY KEY ("digest")
);
-- #[toasty::breakpoint]
CREATE INDEX "index_sessions_by_expires_at" ON "sessions" ("expires_at");
