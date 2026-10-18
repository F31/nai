/* Generated from JSON Schema. Do not edit. */

export interface BackupManifest {
  schema_version: 1;
  db_schema_version: 12;
  created_at_utc_ms: string;
  runtime_version: string;
  contract_catalog_sha256: string;
  sensitive_scope: "configuration-facts-audit-registered-content;credential-references-only";
  database: {
    path: "state.sqlite3";
    sha256: string;
    bytes: number;
  };
  /**
   * @maxItems 512
   */
  artifacts: {
    path: string;
    sha256: string;
    bytes: number;
  }[];
}
