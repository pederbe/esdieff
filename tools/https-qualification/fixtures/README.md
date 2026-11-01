# Local TLS fixtures

These are public test credentials, not application credentials. The CA is used
only by the local test client. RSA-2048 and ECDSA P-256 leaf certificates name
`localhost` and permit server authentication. They have distinct public test
keys and share an ECDSA test issuer. Their validity begins on 1 November 2026
and extends for 36,500 days. The expiration test uses a fixed later verifier time.
The CA signing key was discarded after generation. Tests require no generator.
