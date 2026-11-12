export interface paths {
    "/api/batches": {
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
                        "application/json": components["schemas"]["BatchListResponse"];
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
    "/api/batches/{batch_id}": {
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
                    batch_id: number;
                };
                cookie?: never;
            };
            requestBody: {
                content: {
                    "application/json": components["schemas"]["BatchUpdateRequest"];
                };
            };
            responses: {
                200: {
                    headers: {
                        [name: string]: unknown;
                    };
                    content: {
                        "application/json": components["schemas"]["BatchStateResponse"];
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
    "/api/outbound": {
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
                        "application/json": components["schemas"]["OutboundListResponse"];
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
    "/api/prompts": {
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
                        "application/json": components["schemas"]["PromptsListResponse"];
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
    "/api/prompts/{name}": {
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
                    name: string;
                };
                cookie?: never;
            };
            requestBody: {
                content: {
                    "application/json": components["schemas"]["PromptsUpdateRequest"];
                };
            };
            responses: {
                200: {
                    headers: {
                        [name: string]: unknown;
                    };
                    content: {
                        "application/json": components["schemas"]["PromptsNameResponse"];
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
    "/api/topics": {
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
                        "application/json": components["schemas"]["TopicOverview"];
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
    "/api/topics/{topic_id}": {
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
                path: {
                    topic_id: string;
                };
                cookie?: never;
            };
            requestBody?: never;
            responses: {
                200: {
                    headers: {
                        [name: string]: unknown;
                    };
                    content: {
                        "application/json": components["schemas"]["TopicDetail"];
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
}
export type webhooks = Record<string, never>;
export interface components {
    schemas: {
        BatchItem: {
            attempts: number;
            batch_id: number;
            lines: number;
            reason: string;
            state: string;
            updated_at_ms: number;
        };
        BatchKey: {
            batch_id: number;
        };
        BatchListRequest: {
            limit?: number | null;
            state?: string | null;
        };
        BatchListResponse: {
            items: components["schemas"]["BatchItem"][];
        };
        BatchStateResponse: {
            state: string;
        };
        BatchUpdateRequest: {
            state: "ready";
        };
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
            reason?: string | null;
            received_at_ms: number;
            sender_id: string;
            sender_name: string;
            source: string;
            state?: string | null;
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
        OutboundItem: {
            account: string;
            batch_id: number;
            conversation_id: string;
            flag: string;
            key: string;
            occurred_at_ms: number;
            reason: string;
            role: string;
            sender: string;
            source: string;
            state: string;
            text: string;
        };
        OutboundListRequest: {
            limit?: number | null;
            state?: string | null;
        };
        OutboundListResponse: {
            items: components["schemas"]["OutboundItem"][];
            placeholders: string[];
        };
        PromptsKey: {
            name: string;
        };
        PromptsListItem: {
            name: string;
            required: string[];
            text: string;
        };
        PromptsListResponse: {
            items: components["schemas"]["PromptsListItem"][];
        };
        PromptsNameResponse: {
            name: string;
        };
        PromptsUpdateRequest: {
            text: string;
        };
        TopicChange: {
            at_ms: number;
            from_topic: string;
            keys: number;
            kind: string;
            reason: string;
            serial: number;
            state: string;
            to_topic: string;
        };
        TopicDetail: {
            changes: components["schemas"]["TopicChange"][];
            timeline: components["schemas"]["TopicEntry"][];
            topic: components["schemas"]["TopicItem"];
        };
        TopicEntry: {
            conversation_id: string;
            key: string;
            occurred_at_ms: number;
            reason: string;
            role: string;
            sender: string;
            source: string;
            state: string;
            text: string;
            via: string;
        };
        TopicItem: {
            conversations: number;
            label: string;
            last_at_ms: number;
            members: number;
            people: number;
            score?: number | null;
            sources: string[];
            state: string;
            topic_id: string;
            urgency?: number | null;
            verdict: string;
        };
        TopicKey: {
            topic_id: string;
        };
        TopicListRequest: {
            limit?: number | null;
            state?: string | null;
        };
        TopicOverview: {
            items: components["schemas"]["TopicItem"][];
            lines: components["schemas"]["TopicTally"][];
            topics: components["schemas"]["TopicTally"][];
        };
        TopicTally: {
            count: number;
            reason: string;
            state: string;
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
