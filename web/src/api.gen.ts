export interface paths {
    "/api/health": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: {
            parameters: {
                query?: never;
                header?: {
                    "x-session-id"?: string;
                    "x-user"?: string;
                };
                path?: never;
                cookie?: never;
            };
            requestBody?: never;
            responses: {
                200: {
                    headers: {
                        [name: string]: unknown;
                    };
                    content: {
                        "application/json": components["schemas"]["HealthCheckResponse"];
                    };
                };
                403: {
                    headers: {
                        [name: string]: unknown;
                    };
                    content?: never;
                };
            };
        };
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/messages": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: {
            parameters: {
                query?: {
                    cursor?: string | null;
                    from_ms?: number | null;
                    limit?: number | null;
                    to_ms?: number | null;
                };
                header?: {
                    "x-session-id"?: string;
                    "x-user"?: string;
                };
                path?: never;
                cookie?: never;
            };
            requestBody?: never;
            responses: {
                200: {
                    headers: {
                        [name: string]: unknown;
                    };
                    content: {
                        "application/json": components["schemas"]["MessagesListResponse"];
                    };
                };
                400: {
                    headers: {
                        [name: string]: unknown;
                    };
                    content: {
                        "application/json": components["schemas"]["ErrorBody"];
                    };
                };
                403: {
                    headers: {
                        [name: string]: unknown;
                    };
                    content?: never;
                };
                503: {
                    headers: {
                        [name: string]: unknown;
                    };
                    content: {
                        "application/json": components["schemas"]["ErrorBody"];
                    };
                };
            };
        };
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/reviews": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: {
            parameters: {
                query?: {
                    limit?: number | null;
                    state?: string | null;
                };
                header?: {
                    "x-session-id"?: string;
                    "x-user"?: string;
                };
                path?: never;
                cookie?: never;
            };
            requestBody?: never;
            responses: {
                200: {
                    headers: {
                        [name: string]: unknown;
                    };
                    content: {
                        "application/json": components["schemas"]["ReviewListResponse"];
                    };
                };
                400: {
                    headers: {
                        [name: string]: unknown;
                    };
                    content: {
                        "application/json": components["schemas"]["ErrorBody"];
                    };
                };
                403: {
                    headers: {
                        [name: string]: unknown;
                    };
                    content?: never;
                };
                503: {
                    headers: {
                        [name: string]: unknown;
                    };
                    content: {
                        "application/json": components["schemas"]["ErrorBody"];
                    };
                };
            };
        };
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/reviews/{key}": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch: {
            parameters: {
                query?: never;
                header?: {
                    "x-session-id"?: string;
                    "x-user"?: string;
                };
                path: {
                    key: string;
                };
                cookie?: never;
            };
            requestBody: {
                content: {
                    "application/json": components["schemas"]["ReviewUpdateRequest"];
                };
            };
            responses: {
                200: {
                    headers: {
                        [name: string]: unknown;
                    };
                    content: {
                        "application/json": components["schemas"]["ReviewRulingResponse"];
                    };
                };
                400: {
                    headers: {
                        [name: string]: unknown;
                    };
                    content: {
                        "application/json": components["schemas"]["ErrorBody"];
                    };
                };
                403: {
                    headers: {
                        [name: string]: unknown;
                    };
                    content?: never;
                };
                404: {
                    headers: {
                        [name: string]: unknown;
                    };
                    content: {
                        "application/json": components["schemas"]["ErrorBody"];
                    };
                };
                409: {
                    headers: {
                        [name: string]: unknown;
                    };
                    content: {
                        "application/json": components["schemas"]["ErrorBody"];
                    };
                };
                415: {
                    headers: {
                        [name: string]: unknown;
                    };
                    content?: never;
                };
                422: {
                    headers: {
                        [name: string]: unknown;
                    };
                    content: {
                        "application/json": components["schemas"]["ErrorBody"];
                    };
                };
                503: {
                    headers: {
                        [name: string]: unknown;
                    };
                    content: {
                        "application/json": components["schemas"]["ErrorBody"];
                    };
                };
            };
        };
        trace?: never;
    };
}
export type webhooks = Record<string, never>;
export interface components {
    schemas: {
        ErrorBody: {
            code: string;
        };
        HealthCheckResponse: {
            status: string;
        };
        MessagesListItem: {
            account: string;
            analysis?: string | null;
            conversation_id: string;
            key: string;
            occurred_at_ms: number;
            priority?: number | null;
            received_at_ms: number;
            sender_id: string;
            sender_name: string;
            source: string;
            text: string;
            topic?: string | null;
        };
        MessagesListRequest: {
            cursor?: string | null;
            from_ms?: number | null;
            limit?: number | null;
            to_ms?: number | null;
        };
        MessagesListResponse: {
            items: components["schemas"]["MessagesListItem"][];
            next_cursor?: string | null;
        };
        ReviewItem: {
            batch_id: number;
            conversation_id: string;
            flag: string;
            key: string;
            occurred_at_ms: number;
            reason: string;
            sender: string;
            state: string;
            text: string;
        };
        ReviewKey: {
            key: string;
        };
        ReviewListRequest: {
            limit?: number | null;
            state?: string | null;
        };
        ReviewListResponse: {
            items: components["schemas"]["ReviewItem"][];
            reasons: components["schemas"]["ReviewReason"][];
        };
        ReviewReason: {
            code: string;
            label: string;
            placeholder: string | null;
        };
        ReviewRulingResponse: {
            state: string;
        };
        ReviewUpdateRequest: {
            state: "approved";
        } | {
            leak?: string | null;
            reason: string;
            state: "rejected";
        };
    };
    responses: never;
    parameters: never;
    requestBodies: never;
    headers: never;
    pathItems: never;
}
export type $defs = Record<string, never>;
export type operations = Record<string, never>;
