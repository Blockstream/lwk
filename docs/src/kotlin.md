# Kotlin

## Build

Create Kotlin Multiplatform bindings

```shell
just kotlin-multiplatform
```

## Publish

To publish the Kotlin bindings yourself:

* register a Central Portal account and a namespace; e.g. for `github.com/KyrylR`, the GitHub-backed namespace is `io.github.kyrylr`
* generate a Central Portal user token and store it in `~/.gradle/gradle.properties` as `mavenCentralUsername` and `mavenCentralPassword`
* start from [`gradle.publish.example.properties`](../../lwk_bindings/android_bindings/gradle.publish.example.properties) and copy the keys you need into `~/.gradle/gradle.properties`
* configure signing locally for release publishing with either `signing.keyId` plus `signing.password` plus `signing.secretKeyRingFile` or `signingInMemoryKey` plus `signingInMemoryKeyPassword`

For local publishing commands:

* `just maven-publish <version>` builds with `SIMPLICITY=1 JADE=1` and runs Gradle `publish` against whatever repository target your local Gradle properties configure

The Gradle publishing configuration also accepts environment variables through
the standard `ORG_GRADLE_PROJECT_*` mapping if you prefer not to store the
values in `~/.gradle/gradle.properties`.

## Examples

* [List transactions](../lwk_bindings/tests/bindings/list_transactions.kts) of a wpkh/slip77 wallet
