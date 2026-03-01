# Security Policy

## Supported Versions

We release patches for security vulnerabilities. Currently supported versions:

| Version | Supported          |
| ------- | ------------------ |
| 0.1.x   | :white_check_mark: |

## Reporting a Vulnerability

We take the security of crgx seriously. If you believe you have found a security vulnerability, please report it to us as described below.

### Where to Report

**Please do not report security vulnerabilities through public GitHub issues.**

Instead, please email security reports to the maintainers. You can find contact information in [CONTRIBUTING.md](CONTRIBUTING.md).

### What to Include

Please include the following information in your report:

* Type of issue (e.g. arbitrary code execution, path traversal, etc.)
* Full paths of source file(s) related to the manifestation of the issue
* The location of the affected source code (tag/branch/commit or direct URL)
* Any special configuration required to reproduce the issue
* Step-by-step instructions to reproduce the issue
* Proof-of-concept or exploit code (if possible)
* Impact of the issue, including how an attacker might exploit it

### What to Expect

* We will acknowledge your email within 48 hours
* We will send a more detailed response within 7 days indicating the next steps
* We will keep you informed about progress towards a fix
* We may ask for additional information or guidance
* Once fixed, we will publicly disclose the vulnerability (crediting you if desired)

## Security Considerations

crgx downloads and executes third-party binaries. Users should be aware of:

* **Binary provenance**: crgx downloads pre-built binaries from crate authors' configured sources, GitHub Releases, or cargo-quickinstall. Verify that you trust the crate before running it.
* **No code signing verification**: Downloaded binaries are not currently verified against signatures or checksums beyond HTTPS transport security.
* **Cache integrity**: Cached binaries at `~/.cache/crgx/` should be treated as executable code. Protect the cache directory with appropriate file permissions.
* **`-y` flag**: The `--yes` flag skips the download confirmation prompt. Use with caution in automated environments.

### Best Practices

1. **Review crates before running**: Check the crate on crates.io before first use
2. **Pin versions in CI**: Use exact versions (`crgx tool@1.2.3`) in automated pipelines
3. **Protect your cache**: Ensure `~/.cache/crgx/` is only writable by your user
4. **Update regularly**: Keep crgx updated with latest security patches

## Disclosure Policy

When we receive a security bug report, we will:

1. Confirm the problem and determine affected versions
2. Audit code to find similar problems
3. Prepare fixes for all supported versions
4. Release patches as soon as possible

We ask security researchers to:

* Give us reasonable time to respond before public disclosure
* Make a good faith effort to avoid privacy violations and service disruption
* Not access or modify other users' data

## Comments on this Policy

If you have suggestions on how this process could be improved, please submit a pull request.
