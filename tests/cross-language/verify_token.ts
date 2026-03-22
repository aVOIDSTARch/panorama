#!/usr/bin/env -S npx tsx
/**
 * Cross-language token verification test.
 *
 * Verifies that a Cloak token minted by Rust can be decoded and verified in TypeScript.
 * Token format: base64url(claims_json).hex(hmac_sha256)
 * HMAC is computed over the base64url payload string (not the raw JSON).
 *
 * Usage: npx tsx verify_token.ts [fixture.json]
 *
 * No dependencies needed — uses Node.js built-in crypto module.
 */

import { createHmac, timingSafeEqual } from "crypto";
import { readFileSync } from "fs";

function verifyToken(
  token: string,
  keyHex: string
): Record<string, unknown> {
  const key = Buffer.from(keyHex, "hex");

  // Split on last '.' to separate payload from signature
  const dotIdx = token.lastIndexOf(".");
  if (dotIdx === -1) throw new Error("Malformed token: no separator");

  const payloadB64 = token.slice(0, dotIdx);
  const signatureHex = token.slice(dotIdx + 1);

  // Compute expected HMAC-SHA256 over the base64url payload string
  const expected = createHmac("sha256", key)
    .update(payloadB64, "utf-8")
    .digest("hex");

  // Constant-time comparison
  const expectedBuf = Buffer.from(expected, "utf-8");
  const actualBuf = Buffer.from(signatureHex, "utf-8");
  if (
    expectedBuf.length !== actualBuf.length ||
    !timingSafeEqual(expectedBuf, actualBuf)
  ) {
    throw new Error("Invalid signature");
  }

  // Decode base64url payload
  // Node's Buffer handles base64url natively with 'base64url' encoding
  const claimsJson = Buffer.from(payloadB64, "base64url").toString("utf-8");
  return JSON.parse(claimsJson);
}

function main() {
  const fixturePath = process.argv[2] || "tests/cross-language/fixture.json";
  const fixture = JSON.parse(readFileSync(fixturePath, "utf-8"));

  const { token, key_hex: keyHex, claims: expectedClaims } = fixture;

  const claims = verifyToken(token, keyHex);

  // Assertions
  if (claims.job_id !== expectedClaims.job_id) {
    throw new Error(
      `job_id mismatch: ${claims.job_id} != ${expectedClaims.job_id}`
    );
  }
  if (claims.agent_class !== expectedClaims.agent_class) {
    throw new Error("agent_class mismatch");
  }
  const services = claims.services as Array<Record<string, unknown>>;
  if (services.length !== 1 || services[0].service !== "episteme") {
    throw new Error("services mismatch");
  }

  console.log(`PASS: TypeScript verified token for job_id=${claims.job_id}`);
  console.log(`  Claims: ${JSON.stringify(claims, null, 2)}`);
}

main();
