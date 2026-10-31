# Condite

Condite is planned as self-hosted file storage with a web panel for managing files and their lifetimes. You bring your own S3-compatible storage credentials; Condite uses your bucket to store and serve files.

The project will also support extensions that connect other apps to your storage. The first planned extension is for Vencord: it will intercept Discord's file upload handler and route uploads to Condite.

The current codebase is an early Rust service with S3 upload URL generation. See [`core/README.md`](core/README.md) for setup and API details.
