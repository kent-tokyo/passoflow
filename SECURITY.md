# Security and safety

PassoFlow is a local automation tool. Scenarios can control the mouse and keyboard, open URLs, access files, operate Excel, or control a browser. Review a scenario before running it, use preflight validation, and test risky flows on non-production data.

- Do not run untrusted YAML or images.
- Keep passwords, tokens, and personal data out of scenarios and logs.
- Prefer a narrow image `region`, a specific selector, and explicit waits over unrestricted actions.
- Stop a run when the screen or target page is not in the expected state.
- Keep the tool and its dependencies updated.

## Reporting

For a security issue, contact the project maintainer privately. Include PassoFlow version, OS/Python version, the affected action, reproduction steps, and relevant logs or screenshots. Remove credentials and personal data before sharing.

