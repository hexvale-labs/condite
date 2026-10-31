CREATE TABLE "uploads" (
    "id" BLOB NOT NULL,
    "filename" TEXT NOT NULL,
    "mime_type" TEXT NOT NULL,
    "size_bytes" INTEGER NOT NULL,
    "status" TEXT NOT NULL CHECK ("status" IN ('pending', 'uploaded')),
    "created_at" TEXT NOT NULL,
    PRIMARY KEY ("id")
);
