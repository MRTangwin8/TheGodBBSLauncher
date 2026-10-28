import test from "node:test";
import assert from "node:assert/strict";
import { buildManifest, IDENTITY, msixVersion } from "../scripts/make-msix.mjs";

test("msix version adds a zero revision and rejects odd versions", () => {
  assert.equal(msixVersion("0.3.7"), "0.3.7.0");
  assert.equal(msixVersion(" 1.20.0 "), "1.20.0.0");
  for (const bad of ["0.3", "0.3.7-beta.1", "v0.3.7", "1.2.70000", ""]) assert.throws(() => msixVersion(bad), bad);
});

test("manifest carries the store identity and full-trust entry point", () => {
  const xml = buildManifest("0.3.7");
  assert.match(xml, new RegExp(`Name="${IDENTITY.name}" Publisher="${IDENTITY.publisher}" Version="0.3.7.0"`));
  assert.match(xml, /<PublisherDisplayName>UACG<\/PublisherDisplayName>/);
  assert.match(xml, /EntryPoint="Windows.FullTrustApplication"/);
  assert.match(xml, /rescap:Capability Name="runFullTrust"/);
  assert.match(xml, /Executable="godbbs-launcher.exe"/);
});
