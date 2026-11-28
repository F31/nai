export interface paths {
    "/health": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["observeCosyVoiceDrain"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/inference_zero_shot": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["streamCosyVoice"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/nai/tts/status/{request_id}": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["getCosyVoiceCompletion"];
        put?: never;
        post?: never;
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
        Completion: {
            /** @enum {string} */
            protocol: "nai.cosyvoice.completion.v1";
            request_id: string;
            instance_id: string;
            /** @enum {string} */
            state: "running" | "completed" | "failed" | "cancelled" | "expired";
            samples: number;
            sha256: string;
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
    observeCosyVoiceDrain: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Read-only drain observation; does not cancel GPU work or grant admission */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": {
                        /** @enum {string} */
                        status: "ok" | "blocked";
                        busy: boolean;
                    };
                };
            };
        };
    };
    streamCosyVoice: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody: {
            content: {
                "multipart/form-data": {
                    tts_text: string;
                    prompt_text: string;
                    /** Format: binary */
                    prompt_wav: string;
                    /** @enum {boolean} */
                    stream: true;
                };
            };
        };
        responses: {
            /** @description Raw PCM16LE mono at 24000Hz; success additionally requires matching completed receipt. EOF alone is insufficient. */
            200: {
                headers: {
                    "X-Sample-Rate"?: "24000";
                    "X-Audio-Channels"?: "1";
                    "X-Audio-Encoding"?: "pcm_s16le";
                    "X-Model-Stream"?: "true";
                    "X-Nai-Request"?: string;
                    "X-Nai-Instance"?: string;
                    [name: string]: unknown;
                };
                content: {
                    "application/octet-stream": string;
                };
            };
            /** @description Busy, no automatic retry */
            409: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            /** @description Invalid form */
            422: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            /** @description Generation failed; failures after headers can also abort the byte stream */
            500: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
        };
    };
    getCosyVoiceCompletion: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                request_id: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Ephemeral generation evidence; not playback permission, voice identity or cache authorization */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["Completion"];
                };
            };
            /** @description Unknown or expired request; fail closed without automatic synthesis retry */
            404: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
        };
    };
}
