/* Generated from JSON Schema. Do not edit. */

export interface ProductDraft {
  name: string;
  /**
   * @minItems 1
   * @maxItems 32
   */
  documents: [
    {
      source: string;
      text: string;
      local_only: boolean;
      valid_from_utc_ms?: number;
      valid_until_utc_ms?: number;
    },
    ...{
      source: string;
      text: string;
      local_only: boolean;
      valid_from_utc_ms?: number;
      valid_until_utc_ms?: number;
    }[]
  ];
  /**
   * @minItems 1
   * @maxItems 16
   */
  segments:
    | [string]
    | [string, string]
    | [string, string, string]
    | [string, string, string, string]
    | [string, string, string, string, string]
    | [string, string, string, string, string, string]
    | [string, string, string, string, string, string, string]
    | [string, string, string, string, string, string, string, string]
    | [string, string, string, string, string, string, string, string, string]
    | [string, string, string, string, string, string, string, string, string, string]
    | [string, string, string, string, string, string, string, string, string, string, string]
    | [string, string, string, string, string, string, string, string, string, string, string, string]
    | [string, string, string, string, string, string, string, string, string, string, string, string, string]
    | [string, string, string, string, string, string, string, string, string, string, string, string, string, string]
    | [
        string,
        string,
        string,
        string,
        string,
        string,
        string,
        string,
        string,
        string,
        string,
        string,
        string,
        string,
        string
      ]
    | [
        string,
        string,
        string,
        string,
        string,
        string,
        string,
        string,
        string,
        string,
        string,
        string,
        string,
        string,
        string,
        string
      ];
}
