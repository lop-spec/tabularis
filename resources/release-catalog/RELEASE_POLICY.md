# Release catalog policy

1. A releasable code change on a listed product's `main` branch must have a
   corresponding GitHub Release.
2. Each listed Release must include a Windows portable executable and
   `SHA256SUMS.txt`.
3. The catalog records only assets that already exist and have a verified
   SHA-256 digest; binary assets are never committed to the repository.
4. Catalog changes are versioned directly in `lop-spec/tabularis`. There is no
   separate catalog repository or catalog-only Release.
5. The repositories and Release assets remain private to `lop-spec`.
