export interface paths {
    "/capabilities": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["capabilities"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/health": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["health"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/search": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["search"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/answer": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["answer"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/sync": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["sync"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/delete": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["delete"];
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
        EmptyRequest: Record<string, never>;
        /** @enum {string} */
        RemoteOperation: "search" | "answer" | "sync" | "delete";
        /** @enum {string} */
        RemoteCapability: "retrieval_with_evidence" | "answer_with_evidence" | "answer_only";
        /** @enum {string} */
        RemoteStatus: "complete" | "partial" | "empty" | "timeout" | "unavailable" | "unauthorized";
        /** @enum {string} */
        RemoteReceiptStatus: "accepted" | "pending" | "deleted" | "retained";
        RemoteError: {
            message: string;
        };
        RemoteRetention: {
            policy_ref: string;
            retention_seconds: number;
            supports_delete: boolean;
        };
        RemoteCapabilities: {
            capabilities: components["schemas"]["RemoteCapability"][];
            operations: components["schemas"]["RemoteOperation"][];
            retention: components["schemas"]["RemoteRetention"];
        };
        RemoteHealth: {
            /** @enum {string} */
            status: "ok" | "unavailable";
        };
        RemoteQuery: {
            request_id: string;
            session_id: string;
            kb_ids: string[];
            query: string;
            filters: {
                [key: string]: string;
            };
            top_k: number;
            remaining_budget_ms: number;
        };
        RemoteRevision: {
            object_id: string;
            revision: string;
        };
        RemoteEvidence: {
            evidence_id: string;
            text: string;
            document_id?: string;
            revision?: string;
            chunk_id?: string;
            source_ref?: string;
            source_span?: string;
            product_id?: string;
            rank?: number;
            valid_from_utc_ms?: number | null;
            valid_until_utc_ms?: number | null;
            permission_revision?: string;
            authority?: string;
            fact_versions?: components["schemas"]["RemoteRevision"][];
            original_verifiable?: boolean;
        };
        RemoteUsage: {
            unit: string;
            quantity: number;
        };
        RemoteReply: {
            status: components["schemas"]["RemoteStatus"];
            capability?: components["schemas"]["RemoteCapability"];
            provider_id?: string;
            evidence: components["schemas"]["RemoteEvidence"][];
            answer_draft?: string | null;
            usage?: components["schemas"]["RemoteUsage"];
        };
        RemoteDocument: {
            document_id: string;
            revision: string;
            content: string;
        };
        RemoteSync: {
            request_id: string;
            idempotency_key: string;
            kb_id: string;
            documents: components["schemas"]["RemoteDocument"][];
        };
        RemoteDelete: {
            request_id: string;
            idempotency_key: string;
            kb_id: string;
            document_ids: string[];
        };
        RemoteReceipt: {
            request_id: string;
            status: components["schemas"]["RemoteReceiptStatus"];
            policy_ref: string;
        };
        /** @description Injected by trusted composition root, never inferred from server capabilities or model claims. */
        RemotePolicy: {
            source_id: string;
            policy_ref: string;
            allowed_kb_ids: string[];
            allowed_operations: components["schemas"]["RemoteOperation"][];
            allow_query_upload: boolean;
            allow_document_upload: boolean;
            allow_third_party_retention: boolean;
            max_retention_seconds: number;
            require_remote_delete: boolean;
            expires_at_utc_ms: string;
            permission_revision: string;
            unknown_revision_ttl_ms: number;
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
    capabilities: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["EmptyRequest"];
            };
        };
        responses: {
            /** @description Explicit capabilities and retention policy */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["RemoteCapabilities"];
                };
            };
        };
    };
    health: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["EmptyRequest"];
            };
        };
        responses: {
            /** @description Health does not authorize access */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["RemoteHealth"];
                };
            };
        };
    };
    search: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["RemoteQuery"];
            };
        };
        responses: {
            /** @description Retrieval status is independent of HTTP success */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["RemoteReply"];
                };
            };
            /** @description Unauthorized, never retry */
            401: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["RemoteError"];
                };
            };
            /** @description Forbidden, never retry */
            403: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["RemoteError"];
                };
            };
            /** @description Rate limited, bounded Retry-After */
            429: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["RemoteError"];
                };
            };
            /** @description Unavailable, bounded retry for read operations */
            503: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["RemoteError"];
                };
            };
        };
    };
    answer: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["RemoteQuery"];
            };
        };
        responses: {
            /** @description Retrieval status is independent of HTTP success */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["RemoteReply"];
                };
            };
            /** @description Unauthorized, never retry */
            401: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["RemoteError"];
                };
            };
            /** @description Forbidden, never retry */
            403: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["RemoteError"];
                };
            };
            /** @description Rate limited, bounded Retry-After */
            429: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["RemoteError"];
                };
            };
            /** @description Unavailable, bounded retry for read operations */
            503: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["RemoteError"];
                };
            };
        };
    };
    sync: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["RemoteSync"];
            };
        };
        responses: {
            /** @description Receipt, not proof of publication */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["RemoteReceipt"];
                };
            };
        };
    };
    delete: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["RemoteDelete"];
            };
        };
        responses: {
            /** @description Remote acknowledgment; local revocation must precede this call */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["RemoteReceipt"];
                };
            };
        };
    };
}
