/* Generated from JSON Schema. Do not edit. */

export type WorkbenchConfiguration = {
  edition: "Lite" | "Standard" | "Pro";
  deployment: "Local" | "Hybrid" | "Distributed";
  presentation: "ContentOnly" | "Voice" | "Avatar" | "HumanAssist";
  behavior: "Rehearsal" | "Assisted" | "AutoSupervised";
  delivery_scope: "Private" | "Public";
  queue_limit: number;
  task_deadline_ms: number;
  shadow: boolean;
};
