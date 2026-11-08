---
doppler_automation_app: release_please
---

# Credential delivery

The release App credentials use Doppler's [native GitHub synchronization](https://docs.doppler.com/docs/github-actions).
The [delivery decision](adr/sync-release-app-credentials-to-the-protected-environment.md) governs the cutover.
Canonical values remain in Doppler, and a purpose-specific config references the automation fields.
The [manifest](../automation/release-app-sync.toml) identifies the repository, environment, and field mapping.

The release-please environment admits main jobs and prevents administrator bypass.
RELEASE_PLEASE_APP_ID holds the App client ID, and RELEASE_PLEASE_APP_PRIVATE_KEY holds its private key.
Both secrets reach only the App token action.
Consumers reject empty inputs and unresolved references before minting tokens for the current repository and their required permissions.
The draft job downloads qualified artifacts before creating its App token.
The shared App key retains authority across its installations.
Protected main and environment policies replace Doppler workflow and event claims.

The manual **Verify credential access** workflow checks the selected fields and creates a repository-scoped token with contents read access.
It requires the exact repository identity and main ref.
The action revokes the token during cleanup.
Existing qualification, publication approval, and attestation OIDC remain independent gates.

Verify the exact synchronization target and successful initial synchronization before merging the workflow cutover.
Then run the manual authentication workflow for reviewed main.
Keep the existing Doppler identity until authentication succeeds and all consumers use the replacement.
Replace credentials in their canonical Doppler config, verify synchronization, and repeat authentication before retiring the previous key.

Store only field names, identity metadata, source revisions, and verification outcomes in issues and logs.
Keep credential values and native authentication state out of the repository.
