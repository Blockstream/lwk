# Tests

Run unit tests:
```shell
cargo test --lib
```

End-to-end tests need some local servers:

```shell
./context/download_bins.sh # needed once unless server binaries changes
. .envrc  # not needed if you use direnv and you executed `direnv allow`
```

And also the following docker images:

```shell
docker pull xenoky/local-jade-emulator:1.0.27
docker pull tulipan81/blind_pin_server:v0.0.7
```

Note: Failed test executions can leave docker containers running. To stop all running containers run:

```shell
docker stop $(docker ps -a -q)
```

To run end-to-end tests:

```shell
cargo test
```

To see log outputs use `RUST_LOG` for example

```shell
RUST_LOG=info cargo test -- test_name
RUST_LOG=jade=debug cargo test -- test_name  # filter only on specific module
```

### Test with a physical Jade

Tests using Jade over serial (via USB cable) need an additional dependency:
```shell
apt install -y libudev-dev
```

These serial tests cannot be executed in parallel, so we need the `--test-threads 1` flag.
```shell
cargo test -p lwk_jade --features serial -- serial --include-ignored --test-threads 1
cargo test -p lwk_wollet --features serial -- serial --include-ignored --test-threads 1
```
