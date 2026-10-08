# Releasing

## Versions

The GameController uses [semantic versioning](https://semver.org) with release candidates. The audience are the teams, so a version number says whether a team has to change something:

- **Major** (`8.0.0`): Teams or tools must adapt. This includes changes of
  - the network protocol (`GAMECONTROLLER_STRUCT_VERSION` / `GAMECONTROLLER_RETURN_STRUCT_VERSION` in `RoboCupGameControlData.h`),
  - the format of `config/` files (`params.yaml`, `teams.yaml`),
  - the C API or the command line in an incompatible way,
  - the rules that the GameController implements (usually once per season).
- **Minor** (`7.1.0`): New features that don't require such changes.
- **Patch** (`7.0.1`): Bug fixes.

Before a final version, there are release candidates (`7.1.0-rc.1`, `7.1.0-rc.2`, ...). They are the normal way to test a version in the weeks before a competition and are published as pre-releases. The final version is tagged when the last release candidate hasn't needed any more changes.

## Making a release

The version is only defined in `Cargo.toml` (the Tauri configuration has none, so it uses this one).

1. Bump the version, commit and tag (the working tree must not have uncommitted changes):

   ```bash
   dist/bump-version 7.1.0-rc.1
   ```

2. Push the commit and the tag (the script prints this command):

   ```bash
   git push hsl master v7.1.0-rc.1
   ```

3. The tag triggers `.github/workflows/mkdist.yml`. It builds the app and the C/C++ API on all platforms and then creates a **draft** release with all archives attached. Release candidates are marked as pre-releases. The notes list the commits since the previous release (for a release candidate: since the previous tag; for a final version: since the previous final version).

4. On GitHub, edit the notes of the draft (summarize, remove noise, mention what teams have to change) and publish it.

The notes can be previewed locally with `dist/release-notes v7.1.0-rc.1` (after tagging).

If the workflow fails, fix the problem, delete the draft release (if it was created) and the tag (`git push hsl :refs/tags/v7.1.0-rc.1`, `git tag -d v7.1.0-rc.1`), and start again with the next release candidate number.

## Fixing an older version

To fix a version while `master` already contains changes for the next one, branch from its tag:

```bash
git switch -c release-7.0 v7.0.0
# fix, commit
dist/bump-version 7.0.1
git push hsl release-7.0 v7.0.1
```

The fixes usually need to be merged or cherry-picked into `master`, too.
