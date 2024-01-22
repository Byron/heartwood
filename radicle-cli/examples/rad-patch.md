When contributing to another's project, it is common for the contribution to be
of many commits and involve a discussion with the project's maintainer.  This is supported
via Radicle's patches.

Here we give a brief overview for using patches in our hypothetical car
scenario.  It turns out instructions containing the power requirements were
missing from the project.

```
$ git checkout -b flux-capacitor-power
$ touch REQUIREMENTS
```

Here the instructions are added to the project's README for 1.21 gigawatts and
commit the changes to git.

```
$ git add REQUIREMENTS
$ git commit -v -m "Define power requirements"
[flux-capacitor-power 3e674d1] Define power requirements
 1 file changed, 0 insertions(+), 0 deletions(-)
 create mode 100644 REQUIREMENTS
```

Once the code is ready, we open (or create) a patch with our changes for the project.

``` (stderr)
$ git push rad -o patch.message="Define power requirements" -o patch.message="See details." HEAD:refs/patches
✓ Patch 0f3cd0b3a69c8f70bfa2d3366122c07704e5bb5f opened
To rad://z42hL2jL4XNk6K8oHQaSWfMgCL7ji/z6MknSLrJoTcukLrE435hVNQT4JUhbvWLX4kUzqkEStBU8Vi
 * [new reference]   HEAD -> refs/patches
```

It will now be listed as one of the project's open patches.

```
$ rad patch
╭─────────────────────────────────────────────────────────────────────────────────────────╮
│ ●  ID       Title                      Author                  Head     +   -   Updated │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│ ●  0f3cd0b  Define power requirements  z6MknSL…StBU8Vi  (you)  3e674d1  +0  -0  now     │
╰─────────────────────────────────────────────────────────────────────────────────────────╯
```
```
$ rad patch show 0f3cd0b3a69c8f70bfa2d3366122c07704e5bb5f -p
╭────────────────────────────────────────────────────╮
│ Title     Define power requirements                │
│ Patch     0f3cd0b3a69c8f70bfa2d3366122c07704e5bb5f │
│ Author    z6MknSL…StBU8Vi (you)                    │
│ Head      3e674d1a1df90807e934f9ae5da2591dd6848a33 │
│ Branches  flux-capacitor-power                     │
│ Commits   ahead 1, behind 0                        │
│ Status    open                                     │
│                                                    │
│ See details.                                       │
├────────────────────────────────────────────────────┤
│ 3e674d1 Define power requirements                  │
├────────────────────────────────────────────────────┤
│ ● opened by z6MknSL…StBU8Vi (you) (3e674d1) now    │
╰────────────────────────────────────────────────────╯

commit 3e674d1a1df90807e934f9ae5da2591dd6848a33
Author: radicle <radicle@localhost>
Date:   Thu Dec 15 17:28:04 2022 +0000

    Define power requirements

diff --git a/REQUIREMENTS b/REQUIREMENTS
new file mode 100644
index 0000000..e69de29

```

We can also list only patches that we've authored.

```
$ rad patch list --authored
╭─────────────────────────────────────────────────────────────────────────────────────────╮
│ ●  ID       Title                      Author                  Head     +   -   Updated │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│ ●  0f3cd0b  Define power requirements  z6MknSL…StBU8Vi  (you)  3e674d1  +0  -0  now     │
╰─────────────────────────────────────────────────────────────────────────────────────────╯
```

We can also see that it set an upstream for our patch branch:
```
$ git branch -vv
* flux-capacitor-power 3e674d1 [rad/patches/0f3cd0b3a69c8f70bfa2d3366122c07704e5bb5f] Define power requirements
  master               f2de534 [rad/master] Second commit
```

We can also label patches as well as assign DIDs to the patch to help
organise your workflow:

```
$ rad patch label 0f3cd0b --add fun --no-announce
$ rad patch assign 0f3cd0b --add did:key:z6MknSLrJoTcukLrE435hVNQT4JUhbvWLX4kUzqkEStBU8Vi --no-announce
$ rad patch show 0f3cd0b
╭────────────────────────────────────────────────────╮
│ Title     Define power requirements                │
│ Patch     0f3cd0b3a69c8f70bfa2d3366122c07704e5bb5f │
│ Author    z6MknSL…StBU8Vi (you)                    │
│ Labels    fun                                      │
│ Head      3e674d1a1df90807e934f9ae5da2591dd6848a33 │
│ Branches  flux-capacitor-power                     │
│ Commits   ahead 1, behind 0                        │
│ Status    open                                     │
│                                                    │
│ See details.                                       │
├────────────────────────────────────────────────────┤
│ 3e674d1 Define power requirements                  │
├────────────────────────────────────────────────────┤
│ ● opened by z6MknSL…StBU8Vi (you) (3e674d1) now    │
╰────────────────────────────────────────────────────╯
```

Wait, let's add a README too! Just for fun.

```
$ touch README.md
$ git add README.md
$ git commit --message "Add README, just for the fun"
[flux-capacitor-power 27857ec] Add README, just for the fun
 1 file changed, 0 insertions(+), 0 deletions(-)
 create mode 100644 README.md
```
``` (stderr)
$ git push rad -o patch.message="Add README, just for the fun"
✓ Patch 0f3cd0b updated to revision 8144632f7cc54793386279283a24565f5841e9f8
To rad://z42hL2jL4XNk6K8oHQaSWfMgCL7ji/z6MknSLrJoTcukLrE435hVNQT4JUhbvWLX4kUzqkEStBU8Vi
   3e674d1..27857ec  flux-capacitor-power -> patches/0f3cd0b3a69c8f70bfa2d3366122c07704e5bb5f
```

And let's leave a quick comment for our team:

```
$ rad patch comment 0f3cd0b --message 'I cannot wait to get back to the 90s!' --no-announce
╭───────────────────────────────────────╮
│ z6MknSL…StBU8Vi (you) now 0f81124     │
│ I cannot wait to get back to the 90s! │
╰───────────────────────────────────────╯
$ rad patch comment 0f3cd0b --message 'My favorite decade!' --reply-to 0f81124 -q --no-announce
7e96054a98a2dc4e3c66cd16479cbe9adf7015dc
```

Now, let's checkout the patch that we just created:

```
$ rad patch checkout 0f3cd0b
✓ Switched to branch patch/0f3cd0b
✓ Branch patch/0f3cd0b setup to track rad/patches/0f3cd0b3a69c8f70bfa2d3366122c07704e5bb5f
```

We can also add a review verdict as such:

```
$ rad patch review 0f3cd0b --accept --no-message --no-announce
✓ Patch 0f3cd0b accepted
```

Showing the patch list now will reveal the favorable verdict:

```
$ rad patch show 0f3cd0b
╭─────────────────────────────────────────────────────────────────────╮
│ Title     Define power requirements                                 │
│ Patch     0f3cd0b3a69c8f70bfa2d3366122c07704e5bb5f                  │
│ Author    z6MknSL…StBU8Vi (you)                                     │
│ Labels    fun                                                       │
│ Head      27857ec9eb04c69cacab516e8bf4b5fd36090f66                  │
│ Branches  flux-capacitor-power, patch/0f3cd0b                       │
│ Commits   ahead 2, behind 0                                         │
│ Status    open                                                      │
│                                                                     │
│ See details.                                                        │
├─────────────────────────────────────────────────────────────────────┤
│ 27857ec Add README, just for the fun                                │
│ 3e674d1 Define power requirements                                   │
├─────────────────────────────────────────────────────────────────────┤
│ ● opened by z6MknSL…StBU8Vi (you) (3e674d1) now                     │
│ ↑ updated to 8144632f7cc54793386279283a24565f5841e9f8 (27857ec) now │
│   └─ ✓ accepted by z6MknSL…StBU8Vi (you) now                        │
╰─────────────────────────────────────────────────────────────────────╯
```

If you make a mistake on the patch description, you can always change it!

```
$ rad patch edit 0f3cd0b --message "Define power requirements" --message "Add requirements file" --no-announce
$ rad patch show 0f3cd0b
╭─────────────────────────────────────────────────────────────────────╮
│ Title     Define power requirements                                 │
│ Patch     0f3cd0b3a69c8f70bfa2d3366122c07704e5bb5f                  │
│ Author    z6MknSL…StBU8Vi (you)                                     │
│ Labels    fun                                                       │
│ Head      27857ec9eb04c69cacab516e8bf4b5fd36090f66                  │
│ Branches  flux-capacitor-power, patch/0f3cd0b                       │
│ Commits   ahead 2, behind 0                                         │
│ Status    open                                                      │
│                                                                     │
│ Add requirements file                                               │
├─────────────────────────────────────────────────────────────────────┤
│ 27857ec Add README, just for the fun                                │
│ 3e674d1 Define power requirements                                   │
├─────────────────────────────────────────────────────────────────────┤
│ ● opened by z6MknSL…StBU8Vi (you) (3e674d1) now                     │
│ ↑ updated to 8144632f7cc54793386279283a24565f5841e9f8 (27857ec) now │
│   └─ ✓ accepted by z6MknSL…StBU8Vi (you) now                        │
╰─────────────────────────────────────────────────────────────────────╯
```
