# Unraid GitHub Actions runner

This repository uses a repository-scoped runner on Unraid for trusted `main`-branch verification. Keep the runner configuration separate from this repository; never commit registration tokens, personal access tokens, Docker environment dumps, or Unraid appdata.

## Container template

Clone an existing `myoung34/github-runner` Unraid template and change only the repository-specific fields:

- Image: `myoung34/github-runner:ubuntu-jammy` (the CI installs WebKitGTK 4.1 development packages).
- Repository URL: this repository's GitHub URL.
- Runner scope: `repo`.
- Runner name: unique to this repository.
- Runner label: `unraid`; leave the default `self-hosted`, `linux`, and `x64` labels enabled.
- Temporary runner path and persistent runner-files path: use separate, unique host directories for every repository.
- `CONFIGURED_ACTIONS_RUNNER_FILES_DIR`: the container path for the persistent runner-files mount.
- `DISABLE_AUTOMATIC_DEREGISTRATION`: `true` when persistent runner files are enabled.
- `UNSET_CONFIG_VARS`: `true`, so runner configuration variables are removed before workflow steps run.
- Registration: prefer a short-lived repository runner token in `RUNNER_TOKEN`. Avoid placing an all-repositories personal access token in this public repository's runner.

The `RUNNER_TOKEN` is needed only to register a new runner. The persisted runner configuration allows the container to reuse that registration after restart. If the persistent runner data is discarded, generate a fresh registration token before starting the container.

## Workflow security

`.github/workflows/unraid-ci.yml` runs only for pushes to `main` and manual dispatches on `main`; it has no pull-request trigger. Do not add an untrusted `pull_request` event to a workflow that targets this persistent Unraid runner. Public pull-request code can execute arbitrary commands, and the runner container has access to the Unraid Docker socket. GitHub recommends avoiding self-hosted runners for public-repository pull requests. See [GitHub's secure-use guidance](https://docs.github.com/en/actions/reference/security/secure-use).

The release workflow uses GitHub-hosted runners to package Apple silicon macOS and Windows x64 builds only. The Unraid runner is for verification, not release packaging.
