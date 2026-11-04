export interface paths {
    "/api/v2/capabilities": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["inspectWorkbench"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/v2/prepare": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["prepareWorkbench"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
}
export type webhooks = Record<string, never>;
export interface components {
    schemas: {
        CapabilityView: {
            capability_id: string;
            available: boolean;
            /** @enum {string} */
            implementation: "implemented" | "not_started";
            reason_code: string;
        };
    };
    responses: never;
    parameters: never;
    requestBodies: never;
    headers: never;
    pathItems: never;
}
export type $defs = Record<string, never>;
export interface operations {
    inspectWorkbench: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Typed capability view, never grants output authorization */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["CapabilityView"];
                };
            };
            /** @description Authentication required */
            401: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
        };
    };
    prepareWorkbench: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": {
                    /** @enum {string} */
                    edition: "Lite" | "Standard" | "Pro";
                    /** @enum {string} */
                    deployment: "Local" | "Hybrid" | "Distributed";
                    /** @enum {string} */
                    presentation: "ContentOnly" | "Voice" | "Avatar" | "HumanAssist";
                    /** @enum {string} */
                    behavior: "Rehearsal" | "Assisted" | "AutoSupervised";
                    /** @enum {string} */
                    delivery_scope: "Private" | "Public";
                    queue_limit: number;
                    task_deadline_ms: number;
                    shadow: boolean;
                } & unknown;
            };
        };
        responses: {
            /** @description Authentication required */
            401: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            /** @description Business dependencies not implemented; no output grant */
            503: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
        };
    };
}
