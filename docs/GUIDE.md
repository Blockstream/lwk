# Guidelines
The LWK mdbook is a high level, descriptive documentation for LWK.
It serves as an entry point for introducing LWK, explaining what LWK enables.

For this reason there is no need to go too deep with explanations,
for those we have the rust-docs and tests.

## Snippets
### Style
Use `#ANCHOR: ` and `#ANCHOR_END:` to capture the code portion displayed in the docs.

Leave setup and most asserts out of those blocks.

Consider renaming variables to make them more descriptive.

### CI
Code snippets must be executable locally to ensure they compile, they're up-to-date, and do what they're describing.

Code snippets should be run in CI.
However in some rare cases, running such test in CI can be problematic;
in these cases, state explicitly why the test is not run in CI.

### Public Interface
Code snippets must document the public interface,
thus they must use the language public interface,
they must not use internal test utilities.

Code snippets must use variable names that document function parameters.

This has the annoying consequence that we often cannot re-use tests for documentation.
