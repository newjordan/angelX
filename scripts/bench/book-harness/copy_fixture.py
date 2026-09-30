"""Copy an experiment's files into an empty directory with private Git state."""

import argparse
import os
from pathlib import Path
import shutil
import stat
import subprocess


def copy_directory(source, destination, active, destination_key):
    identity = source.stat()
    key = (identity.st_dev, identity.st_ino)
    if key == destination_key:
        raise ValueError("fixture source links back into its destination")
    if key in active:
        raise ValueError(f"fixture contains a directory symlink cycle: {source}")
    active = active | {key}
    for entry in sorted(source.iterdir()):
        # Worktrees/submodules can point outside this copy. Never copy their
        # Git metadata, including .git directories and symbolic links.
        if entry.name == ".git":
            continue
        target = destination / entry.name
        metadata = entry.stat()
        if stat.S_ISDIR(metadata.st_mode):
            target.mkdir()
            copy_directory(entry, target, active, destination_key)
        elif stat.S_ISREG(metadata.st_mode):
            # Materialize symbolic/hard links as private bytes, including
            # executable links in a copied virtualenv. No write aliases.
            shutil.copy2(entry, target)
        else:
            raise ValueError(f"fixture contains an unsupported file type: {entry}")


def snapshot_fixture(source, destination):
    source = Path(source).resolve(strict=True)
    destination = Path(destination).resolve(strict=True)
    if not source.is_dir() or not destination.is_dir():
        raise ValueError("source and destination must be directories")
    if destination == source or source in destination.parents:
        raise ValueError("fixture destination must be outside its source tree")
    if any(destination.iterdir()):
        raise ValueError("fixture destination must be empty")
    metadata = destination.stat()
    copy_directory(source, destination, set(), (metadata.st_dev, metadata.st_ino))
    # Inherited GIT_DIR/GIT_COMMON_DIR/index settings must not redirect this
    # baseline into the caller's repository; no hooks or signing are needed.
    env = {"PATH": os.environ["PATH"], "GIT_CONFIG_GLOBAL": "/dev/null", "GIT_CONFIG_NOSYSTEM": "1"}
    indexed = subprocess.run(["git", "-C", str(source), "ls-files", "-z"], env=env,
                             stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, check=False)
    tracked = []
    if indexed.returncode == 0:
        for name in indexed.stdout.split(b"\0"):
            path = Path(os.fsdecode(name))
            if name and not path.is_absolute() and ".." not in path.parts and (destination / path).is_file():
                tracked.append(name)
    command = ["git", "--literal-pathspecs", "-C", str(destination), "-c", "core.hooksPath=/dev/null",
               "-c", "user.name=benchmark", "-c", "user.email=benchmark@example.invalid",
               "-c", "commit.gpgsign=false"]
    for args in (["-c", "init.templateDir=", "init", "-q"], ["add", "-A"]):
        subprocess.run([*command, *args], env=env, check=True, stdin=subprocess.DEVNULL)
    # Preserve tracked files that now match an ignore rule, while keeping
    # ignored virtualenvs and other untracked dependencies out of the index.
    if tracked:
        subprocess.run([*command, "add", "-f", "--pathspec-from-file=-", "--pathspec-file-nul"],
                       input=b"\0".join(tracked) + b"\0", env=env, check=True)
    subprocess.run([*command, "commit", "--allow-empty", "-qm", "experiment baseline"],
                   env=env, check=True, stdin=subprocess.DEVNULL)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("source", type=Path)
    parser.add_argument("destination", type=Path)
    args = parser.parse_args()
    try:
        snapshot_fixture(args.source, args.destination)
    except (ValueError, OSError, subprocess.CalledProcessError) as error:
        parser.error(str(error))


if __name__ == "__main__":
    main()
