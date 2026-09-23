# LWK Boltz

Integration with the [Boltz Exchange](https://github.com/boltzexchange)

## Regtest env

The first time we need to initialize the git submodule

```shell
git submodule update --init --recursive
```

The following recipes are defined in this crate's `justfile`. From the
repository root, enter the crate directory first:

```shell
cd lwk_boltz
just regtest-env-start
```

Launch integration testing

```shell
just test-submarine
```

```shell
just test-reverse
```
