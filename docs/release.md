# Releasing

How a release is built and signed. The branch and approval flow (`release/X.Y.Z` from `develop`, PR into `main`, tag `vX.Y.Z` on `main`, draft, publish) is in [`development.md` §5](development.md#5-release-process); this file covers the signing key, the version bump and the release workflow.

## 1. Updater signing key

The in-app updater only installs updates signed with the project's minisign key pair. The public key is in `src-tauri/tauri.conf.json` (`plugins.updater.pubkey`); the private key lives only in GitHub secrets and in an offline backup.

**Losing the private key (or its password) means existing installs can never update again**: users would have to download and install a new build by hand, signed with a new key. Never put the private key in the repo, in chat, in an issue or in logs.

### Generate (once, by the owner)

In PowerShell:

```
npm run tauri signer generate -- -w "$env:USERPROFILE\.tauri\prompt-translator.key"
```

(`%USERPROFILE%` only expands in cmd.exe; in PowerShell or Git Bash it creates a literal `%USERPROFILE%` folder in the current directory, so never run that form inside the repo.)

Choose a strong password when asked. This writes the private key (`prompt-translator.key`) and the public key (`prompt-translator.key.pub`).

### Back up

Copy the private key file and the password to offline storage (a password manager entry plus an encrypted USB drive, for example). Keep them out of any synced folder that is public or shared.

### Repo secrets

In the repo's Settings, Secrets and variables, Actions, add:

| Secret | Value |
|---|---|
| `TAURI_SIGNING_PRIVATE_KEY` | the full contents of `prompt-translator.key` (or its path, but on GitHub use the contents) |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | the password chosen above |

### Public key

`plugins.updater.pubkey` in `src-tauri/tauri.conf.json` holds the contents of `prompt-translator.key.pub` (set in #66). The public key is not secret. If the key pair is ever replaced, paste the new `.pub` contents there and commit it; installs with the old key can then no longer update.

### Local builds

`bundle.createUpdaterArtifacts` is `true`, so `npm run tauri build` signs the installer and needs `TAURI_SIGNING_PRIVATE_KEY` (and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`) in the environment. For a local unsigned build, pass `--config "{\"bundle\":{\"createUpdaterArtifacts\":false}}"`.

## 2. Version bump

`package.json` holds the version. Sync the rest with:

```
node scripts/bump-version.mjs X.Y.Z
```

It updates `package.json`, `src-tauri/tauri.conf.json`, `src-tauri/Cargo.toml` and the app's entry in `src-tauri/Cargo.lock`. Without an argument it re-syncs the other files to `package.json`'s version. Do this on the `release/X.Y.Z` branch, with the `CHANGELOG.md` update.

## 3. The release workflow

`.github/workflows/release.yml` runs when a `v*` tag is pushed. On `windows-latest` (permissions: `contents: write` only) it runs the same checks as CI, then `tauri-apps/tauri-action` builds the NSIS installer and creates a **draft** release containing:

- `Prompt Translator_X.Y.Z_x64-setup.exe`
- its `.sig` signature
- `latest.json`, whose URLs point at the release assets

Tags containing `-` (for example `v0.0.1-test`) are marked as pre-releases. The draft stays invisible to users and to the updater until you publish it.

### Test run (before the first real release)

Push a throwaway tag on a commit of a feature branch (never on `main`), for example `git tag v0.0.1-test && git push origin v0.0.1-test`. When the run finishes, open the draft and check the three files above and the URLs in `latest.json`. Then delete the draft release and the tag (`git push origin --delete v0.0.1-test`).

### If the workflow fails

1. Open the run in the Actions tab and find the failing step.
2. **Check signing secret is set**: add the missing secret (section 1).
3. **Lint, typecheck, clippy, bindings**: fix on `develop` (or the release branch), as for any CI failure.
4. **Signing errors in the build step** (wrong password, "invalid private key"): re-check both secrets; the key secret must be the whole file contents with no added whitespace.
5. **Release already exists** or a half-built draft: delete the draft release and the tag (`git push origin --delete vX.Y.Z`), fix the cause, then re-tag the corrected commit. A tag on `main` needs the owner's approval to delete and re-push.
6. A build that fails at the end never publishes anything: drafts are not visible to users. Never publish a draft that lacks the `.sig` or `latest.json`.
