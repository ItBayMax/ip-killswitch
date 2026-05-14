#!/usr/bin/env node
/**
 * Reads ./tauri-signing-key.key.pub, base64-encodes it, and writes the result
 * into src-tauri/tauri.conf.json under `plugins.updater.pubkey`.
 *
 * Run this once after generating the key pair with:
 *   npx tauri signer generate -w tauri-signing-key.key
 */

import { readFileSync, writeFileSync, existsSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const rootDir = join(dirname(fileURLToPath(import.meta.url)), "..");
const pubKeyPath = join(rootDir, "tauri-signing-key.key.pub");
const confPath = join(rootDir, "src-tauri", "tauri.conf.json");

if (!existsSync(pubKeyPath)) {
  console.error(`[sync-pubkey] ${pubKeyPath} not found.`);
  console.error("[sync-pubkey] Generate it with:");
  console.error("  npx tauri signer generate -w tauri-signing-key.key");
  process.exit(1);
}

// Tauri CLI 2.x writes the .pub file as a SINGLE base64 line — that is the
// exact value the updater plugin expects in `plugins.updater.pubkey`. We do
// NOT re-encode it (an earlier version of this script did, which caused
// `Missing encoded key in public key` at build time).
const pubkey = readFileSync(pubKeyPath, "utf8").replace(/\s+/g, "");

const conf = JSON.parse(readFileSync(confPath, "utf8"));
conf.plugins = conf.plugins || {};
conf.plugins.updater = conf.plugins.updater || {};
conf.plugins.updater.pubkey = pubkey;
writeFileSync(confPath, JSON.stringify(conf, null, 2) + "\n");

console.log(`[sync-pubkey] Wrote ${pubkey.length}-char pubkey into ${confPath}`);
