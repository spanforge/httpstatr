# Security policy

## Supported versions

Security fixes are provided for the newest stable release. Version 1.0.0
supports Windows x86-64. Earlier release candidates are superseded by 1.0.0.

## Reporting a vulnerability

Please use GitHub's private vulnerability-reporting form in the repository's
Security tab:

https://github.com/spanforge/httpstatr/security/advisories/new

Do not open a public issue for an unpatched credential-disclosure, command
execution, path-handling, or dependency vulnerability. Include the affected
version, operating system, curl version, reproduction steps, and potential
impact. Maintainers should acknowledge a report within seven days and provide
status updates until it is resolved or closed.

`httpstatr` invokes the configured curl executable and forwards user-provided
curl arguments. Only use trusted executables and trusted suite files.
