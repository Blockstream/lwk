# Nix

We provide a flake for a dev environment and for running the `lwk_cli`.
If you use direnv and allow the `.envrc` the dev environment is automatically loaded
as soon as you enter the directory, otherwise you can run:

```
nix develop
```

To run `lwk_cli` on nix-enabled system:

```
nix run github:blockstream/lwk
```
