/* Generated from JSON Schema. Do not edit. */

export interface PluginUiDeclaration {
  schema_version: 1;
  plugin_id: string;
  config_schema_ref: string;
  /**
   * @maxItems 32
   */
  navigation: {
    id: string;
    label: string;
    component_ref: string;
    required_permission: string;
  }[];
}
