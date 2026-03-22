#!/usr/bin/env python3
"""
Cross-language token verification test.

Verifies that a Cloak token minted by Rust can be decoded and verified in Python.
Token format: base64url(claims_json).hex(hmac_sha256)
HMAC is computed over the base64url payload string (not the raw JSON).

Usage: python verify_token.py [fixture.json]
"""

import base64
import hashlib
import hmac
import json
import sys


def verify_token(token: str, key_hex: str) -> dict:
    """Verify a Cloak token and return decoded claims."""
    key = bytes.fromhex(key_hex)

    # Split on last '.' to separate payload from signature
    dot_idx = token.rfind(".")
    if dot_idx == -1:
        raise ValueError("Malformed token: no separator")

    payload_b64 = token[:dot_idx]
    signature_hex = token[dot_idx + 1 :]

    # Compute expected HMAC-SHA256 over the base64url payload string
    expected = hmac.new(key, payload_b64.encode("utf-8"), hashlib.sha256).hexdigest()

    # Constant-time comparison
    if not hmac.compare_digest(expected, signature_hex):
        raise ValueError("Invalid signature")

    # Decode payload (base64url, no padding)
    # Add padding back
    padding = 4 - len(payload_b64) % 4
    if padding != 4:
        payload_b64 += "=" * padding

    claims_json = base64.urlsafe_b64decode(payload_b64)
    return json.loads(claims_json)


def main():
    fixture_path = sys.argv[1] if len(sys.argv) > 1 else "tests/cross-language/fixture.json"

    with open(fixture_path) as f:
        fixture = json.load(f)

    token = fixture["token"]
    key_hex = fixture["key_hex"]
    expected_claims = fixture["claims"]

    # Verify
    claims = verify_token(token, key_hex)

    # Check fields
    assert claims["job_id"] == expected_claims["job_id"], (
        f"job_id mismatch: {claims['job_id']} != {expected_claims['job_id']}"
    )
    assert claims["agent_class"] == expected_claims["agent_class"], (
        f"agent_class mismatch"
    )
    assert len(claims["services"]) == 1
    assert claims["services"][0]["service"] == "episteme"

    print(f"PASS: Python verified token for job_id={claims['job_id']}")
    print(f"  Claims: {json.dumps(claims, indent=2)}")


if __name__ == "__main__":
    main()
