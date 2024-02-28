# Updating to the latest version of substrate
These instructions will delete and restore folders and files. It is advised to use a fresh checkout to avoid losing local changes that are not committed.

## Prerequisites
* A clean checkout of substrate on a branch with our custom changes
* A clean checkout of avn-tier2 project on latest master

## Substrate
We maintain a fork of [substrate](https://github.com/ArtosSystems/substrate) where we keep minimal changes. This is used as a library from avn-tier2 project.
Substrate project has tags of all the releases with this format ```v2.0.new```.
```
git checkout v2.0.new
git checkout -b v2.0.new_avn
```
This will create a new branch, on top of that version, which we can use to apply on top of it our changes.
To identify these changes use
```
git log v2.0.old-_avn
```
And then `cherry-pick` the changes to the new branch.

## Avn-tier2

run from the repository root:
```
rm -rf bin/*
cd bin
cp -r <path_to_current_version>/bin/* ./
git checkout -- node/cli/avn-service
git add -u .
git commit -m "Restore bin folder to substrate <version>"
git show
```
Note the commit hash here, we will refer to it as {restore_hash}

Test removal of files
```
git clean -n
```
Actual removes files
```
git clean -f
cp -r <path_to_new_version>/bin/node ./
cp -r <path_to_new_version>/bin/utils ./
# node_template folder is not used atm
# bin/node/browser-testing // not used prolly
# bin/node/inspect // not used
# bin/node/primitives // not used
# bin/node/rpc-client // not used prolly
```
At this point you need to eye ball the status of the repository.

If paths that already staged have new files, or dependencies add them.

```
git commit -m "Upgrade bin folder to substrate <new_version>"
git revert {restore_hash}
```
Expect conflicts, and resolve them one by one.

There will be several conflicts of this format in Cargo.toml files:
```
node-primitives = { version = "3.0.<new_version>", path = "../primitives" }
node-primitives = { version = "3.0.<old_version>", git = "https://github.com/ArtosSystems/substrate", branch = "v2.0.0-<old_version>_avn" }
```

You need to keep the incoming change that has the dependency that uses git but you need to update it to the new values: for the version and git branch used. The latter can be done as a seperate step using search and replace in the toml files:

from ```version = "2.0.<old>"``` to ```version = "2.0.<new>"```

from ```branch = "2.0.<old>"``` to ```branch = "2.0.<new>_avn"```

Copy substrate's projects Cargo.lock file from the branch and perform a successful build.

Resolve any build errors or broken interfaces from the upgraded packages.

A very useful command at this point is `cargo tree` which shows the dependencies a module or binary has. You can use it to identify dependencies duplicates and rectify them. It is important to avoid having multiple versions of a library as it can cause runtime panics.
Some useful commands to help you untangle this:

```
# Shows module/project dependencies
cargo tree

# Shows a package and its dependencies
cargo tree -p <package name>

# Inverts the dependency tree showing other packages that have a dependency on this package
cargo tree -i -p <package name>

# Show only dependencies which come in multiple versions (implies -i)
cargo tree -d <-p package name>
```
Commit any changes on Cargo.lock.

`Update these steps if needed.`

## Updating cargo dependencies

When new commits gets pushed to the remote branch run the following command to force cargo to update the substrate libraries. The precise flag is optional, and you can specify exactly the commit you want to be used.

```
cargo update -p sp-core [--precise <commit-hash>]
```

## Regression Testing
* Run all unit tests
* Perform a full build without any errors
* Ensure that all modules & pallets are included
* Run binary in --dev mode
* Run multiple nodes and setup a local blockchain, ensure that everything is working as intended