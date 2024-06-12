Let's say we initialize a private repository and specify that we don't want it
to be seeded.
```
$ rad init --name heartwood --description "radicle heartwood protocol & stack" --no-confirm --private --no-seed

Initializing private radicle 👾 repository in [..]

✓ Repository heartwood created.

Your Repository ID (RID) is rad:z2ug5mwNKZB8KGpBDRTrWHAMbvHCu.
You can show it any time by running `rad .` from this directory.

You have created a private repository.
This repository will only be visible to you, and to peers you explicitly allow.

To make it public, run `rad publish`.
To push changes, run `git push`.
```

```
$ rad seed
No seeding policies to show.
```

We can decide to seed it later:
```
$ rad seed rad:z2ug5mwNKZB8KGpBDRTrWHAMbvHCu
✓ Seeding policy updated for rad:z2ug5mwNKZB8KGpBDRTrWHAMbvHCu with scope 'all'
```

But it still won't show up in our inventory, since it's private:
```
$ rad node inventory
```
