/* Generated from JSON Schema. Do not edit. */

export interface ModelMigrationBackup {
  schema_version: 1;
  backup_id: string;
  plan_sha256: string;
  credential_keys: "external_keyring_not_exported";
  files: {
    [k: string]: {
      sha256: string;
      bytes: number;
    };
  };
}
