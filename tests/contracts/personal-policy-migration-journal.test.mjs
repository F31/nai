import { test } from 'node:test';
import assert from 'node:assert/strict';
import { validators } from '../../scripts/contracts/validation.mjs';

const validate = (await validators()).personalPolicyMigrationJournal;
const journal = {schema_version:1,migration_id:'00000000-0000-0000-0000-000000000001',source_version:2,target_version:3,phase:'prepared',backup_id:'00000000-0000-0000-0000-000000000002',backup_manifest_sha256:'a'.repeat(64)};

test('journal pins versions, bounded phases and verified-backup identity', () => {
  for (const phase of ['prepared','backup_verified','committed']) assert.equal(validate({...journal, phase}), true);
  for (const bad of [{...journal,source_version:1},{...journal,target_version:4},{...journal,phase:'rollback'},{...journal,backup_id:'../outside'},{...journal,backup_manifest_sha256:'a'.repeat(63)},{...journal,restore:true}]) assert.equal(validate(bad), false);
});
