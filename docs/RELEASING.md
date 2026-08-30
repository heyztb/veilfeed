# Releasing Veilfeed

Veilfeed publishes separate Apple Silicon and Intel DMGs. Tag workflows create
draft releases only; publishing is always a manual maintainer action.

## One-time GitHub setup

1. Push this repository to GitHub and protect `main`. Require the `ci / test`
   check before merging.
2. Create a protected `release` environment restricted to tags matching `v*`.
3. Add these environment secrets:
   `APPLE_CERTIFICATE`, `APPLE_CERTIFICATE_PASSWORD`, `KEYCHAIN_PASSWORD`,
   `APPLE_SIGNING_IDENTITY`, `APPLE_API_ISSUER`, `APPLE_API_KEY`, and
   `APPLE_API_PRIVATE_KEY`.
4. Use a Developer ID Application certificate and an App Store Connect API key
   with Developer access. Never commit either private key.

## Cut a release

1. From a clean branch based on `main`, run
   `deno task release:version X.Y.Z`.
2. Review and merge the version-only pull request after CI passes.
3. Create an annotated tag at that merge commit: `git tag -a vX.Y.Z -m
   "Veilfeed vX.Y.Z"`, then push the tag once. Never move or reuse a release
   tag.
4. Wait for the release workflow. It creates a draft GitHub Release containing
   signed, notarized Intel and Apple Silicon DMGs plus `SHA256SUMS`.
5. Review the generated notes and confirm both DMGs pass the workflow's
   signature, Gatekeeper, stapling, architecture, mount, and startup checks.
6. Download and open both DMGs through Finder on representative Macs. For the
   first release, exercise a fresh database, feed refresh, unavailable proxy,
   Keychain credential, and OPML import/export. On later releases, also open a
   copy of data created by the previous version and confirm its migration.
7. Publish the draft manually. Veilfeed's release check will not see the release
   before publication.

If a release is defective, return it to draft immediately and publish a new
patch release. Do not reuse its tag and do not automate a downgrade.

