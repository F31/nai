/* Generated from JSON Schema. Do not edit. */

/**
 * Maintenance journal metadata only; never a grant or a rollback instruction.
 */
export interface PersonalPolicyMigrationJournal {
  schema_version: 1;
  migration_id: string;
  source_version: 2;
  target_version: 3;
  phase: "prepared" | "backup_verified" | "committed";
  backup_id: string;
  backup_manifest_sha256: string;
}
