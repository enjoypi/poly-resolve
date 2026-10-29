export interface paths {
    "/health/check": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: {
            parameters: {
                query?: never;
                header?: {
                    "x-session-id"?: string;
                    "x-user"?: string;
                };
                path?: never;
                cookie?: never;
            };
            requestBody: {
                content: {
                    "application/json": {
                        [key: string]: unknown;
                    };
                };
            };
            responses: {
                200: {
                    headers: {
                        [name: string]: unknown;
                    };
                    content: {
                        "application/json": components["schemas"]["HealthCheckResponse"];
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
            };
        };
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/ingest/assemble": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: {
            parameters: {
                query?: never;
                header?: {
                    "x-session-id"?: string;
                    "x-user"?: string;
                };
                path?: never;
                cookie?: never;
            };
            requestBody: {
                content: {
                    "application/json": components["schemas"]["IngestAssembleRequest"];
                };
            };
            responses: {
                200: {
                    headers: {
                        [name: string]: unknown;
                    };
                    content: {
                        "application/json": components["schemas"]["IngestEnvelope"];
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
            };
        };
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/ingest/cipher": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: {
            parameters: {
                query?: never;
                header?: {
                    "x-session-id"?: string;
                    "x-user"?: string;
                };
                path?: never;
                cookie?: never;
            };
            requestBody: {
                content: {
                    "application/json": components["schemas"]["IngestCipherRequest"];
                };
            };
            responses: {
                200: {
                    headers: {
                        [name: string]: unknown;
                    };
                    content: {
                        "application/json": components["schemas"]["IngestCipherResponse"];
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
            };
        };
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/ingest/parse_event": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: {
            parameters: {
                query?: never;
                header?: {
                    "x-session-id"?: string;
                    "x-user"?: string;
                };
                path?: never;
                cookie?: never;
            };
            requestBody: {
                content: {
                    "application/json": components["schemas"]["IngestParseRequest"];
                };
            };
            responses: {
                200: {
                    headers: {
                        [name: string]: unknown;
                    };
                    content: {
                        "application/json": components["schemas"]["IngestEnvelope"];
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
            };
        };
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/ingest/parse_page": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: {
            parameters: {
                query?: never;
                header?: {
                    "x-session-id"?: string;
                    "x-user"?: string;
                };
                path?: never;
                cookie?: never;
            };
            requestBody: {
                content: {
                    "application/json": components["schemas"]["IngestParseRequest"];
                };
            };
            responses: {
                200: {
                    headers: {
                        [name: string]: unknown;
                    };
                    content: {
                        "application/json": components["schemas"]["IngestParsePageResponse"];
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
            };
        };
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/ingest/reveal": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: {
            parameters: {
                query?: never;
                header?: {
                    "x-session-id"?: string;
                    "x-user"?: string;
                };
                path?: never;
                cookie?: never;
            };
            requestBody: {
                content: {
                    "application/json": components["schemas"]["IngestRevealRequest"];
                };
            };
            responses: {
                200: {
                    headers: {
                        [name: string]: unknown;
                    };
                    content: {
                        "application/json": components["schemas"]["IngestEnvelope"];
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
            };
        };
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/ingest/window": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: {
            parameters: {
                query?: never;
                header?: {
                    "x-session-id"?: string;
                    "x-user"?: string;
                };
                path?: never;
                cookie?: never;
            };
            requestBody: {
                content: {
                    "application/json": components["schemas"]["IngestWindowRequest"];
                };
            };
            responses: {
                200: {
                    headers: {
                        [name: string]: unknown;
                    };
                    content: {
                        "application/json": components["schemas"]["IngestWindowResponse"];
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
            };
        };
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/messages/list": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: {
            parameters: {
                query?: never;
                header?: {
                    "x-session-id"?: string;
                    "x-user"?: string;
                };
                path?: never;
                cookie?: never;
            };
            requestBody: {
                content: {
                    "application/json": components["schemas"]["MessagesListRequest"];
                };
            };
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
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/review/approve": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: {
            parameters: {
                query?: never;
                header?: {
                    "x-session-id"?: string;
                    "x-user"?: string;
                };
                path?: never;
                cookie?: never;
            };
            requestBody: {
                content: {
                    "application/json": components["schemas"]["ReviewApproveRequest"];
                };
            };
            responses: {
                200: {
                    headers: {
                        [name: string]: unknown;
                    };
                    content: {
                        "application/json": components["schemas"]["ReviewApproveResponse"];
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
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/review/list": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: {
            parameters: {
                query?: never;
                header?: {
                    "x-session-id"?: string;
                    "x-user"?: string;
                };
                path?: never;
                cookie?: never;
            };
            requestBody: {
                content: {
                    "application/json": components["schemas"]["ReviewListRequest"];
                };
            };
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
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/review/reject": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: {
            parameters: {
                query?: never;
                header?: {
                    "x-session-id"?: string;
                    "x-user"?: string;
                };
                path?: never;
                cookie?: never;
            };
            requestBody: {
                content: {
                    "application/json": components["schemas"]["ReviewRejectRequest"];
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
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/triage/brief": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: {
            parameters: {
                query?: never;
                header?: {
                    "x-session-id"?: string;
                    "x-user"?: string;
                };
                path?: never;
                cookie?: never;
            };
            requestBody: {
                content: {
                    "application/json": components["schemas"]["TriageBriefRequest"];
                };
            };
            responses: {
                200: {
                    headers: {
                        [name: string]: unknown;
                    };
                    content: {
                        "application/json": components["schemas"]["TriageBriefResponse"];
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
            };
        };
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/triage/line": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: {
            parameters: {
                query?: never;
                header?: {
                    "x-session-id"?: string;
                    "x-user"?: string;
                };
                path?: never;
                cookie?: never;
            };
            requestBody: {
                content: {
                    "application/json": components["schemas"]["TriageLineRequest"];
                };
            };
            responses: {
                200: {
                    headers: {
                        [name: string]: unknown;
                    };
                    content: {
                        "application/json": components["schemas"]["TriageLineResponse"];
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
            };
        };
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/triage/notice": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: {
            parameters: {
                query?: never;
                header?: {
                    "x-session-id"?: string;
                    "x-user"?: string;
                };
                path?: never;
                cookie?: never;
            };
            requestBody: {
                content: {
                    "application/json": components["schemas"]["TriageNoticeRequest"];
                };
            };
            responses: {
                200: {
                    headers: {
                        [name: string]: unknown;
                    };
                    content: {
                        "application/json": components["schemas"]["TriageNoticeResponse"];
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
            };
        };
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/triage/pack": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: {
            parameters: {
                query?: never;
                header?: {
                    "x-session-id"?: string;
                    "x-user"?: string;
                };
                path?: never;
                cookie?: never;
            };
            requestBody: {
                content: {
                    "application/json": components["schemas"]["TriagePackRequest"];
                };
            };
            responses: {
                200: {
                    headers: {
                        [name: string]: unknown;
                    };
                    content: {
                        "application/json": components["schemas"]["TriagePackResponse"];
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
            };
        };
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/triage/redact": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: {
            parameters: {
                query?: never;
                header?: {
                    "x-session-id"?: string;
                    "x-user"?: string;
                };
                path?: never;
                cookie?: never;
            };
            requestBody: {
                content: {
                    "application/json": components["schemas"]["TriageRedactRequest"];
                };
            };
            responses: {
                200: {
                    headers: {
                        [name: string]: unknown;
                    };
                    content: {
                        "application/json": components["schemas"]["TriageRedactResponse"];
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
            };
        };
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/triage/reply": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: {
            parameters: {
                query?: never;
                header?: {
                    "x-session-id"?: string;
                    "x-user"?: string;
                };
                path?: never;
                cookie?: never;
            };
            requestBody: {
                content: {
                    "application/json": components["schemas"]["TriageReplyRequest"];
                };
            };
            responses: {
                200: {
                    headers: {
                        [name: string]: unknown;
                    };
                    content: {
                        "application/json": components["schemas"]["TriageReplyResponse"];
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
            };
        };
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/triage/screen": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: {
            parameters: {
                query?: never;
                header?: {
                    "x-session-id"?: string;
                    "x-user"?: string;
                };
                path?: never;
                cookie?: never;
            };
            requestBody: {
                content: {
                    "application/json": components["schemas"]["TriageScreenRequest"];
                };
            };
            responses: {
                200: {
                    headers: {
                        [name: string]: unknown;
                    };
                    content: {
                        "application/json": components["schemas"]["TriageScreenResponse"];
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
            };
        };
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/triage/screened": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: {
            parameters: {
                query?: never;
                header?: {
                    "x-session-id"?: string;
                    "x-user"?: string;
                };
                path?: never;
                cookie?: never;
            };
            requestBody: {
                content: {
                    "application/json": components["schemas"]["TriageScreenedRequest"];
                };
            };
            responses: {
                200: {
                    headers: {
                        [name: string]: unknown;
                    };
                    content: {
                        "application/json": components["schemas"]["TriageScreenedResponse"];
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
            };
        };
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/triage/sort": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: {
            parameters: {
                query?: never;
                header?: {
                    "x-session-id"?: string;
                    "x-user"?: string;
                };
                path?: never;
                cookie?: never;
            };
            requestBody: {
                content: {
                    "application/json": components["schemas"]["TriageSortRequest"];
                };
            };
            responses: {
                200: {
                    headers: {
                        [name: string]: unknown;
                    };
                    content: {
                        "application/json": components["schemas"]["TriageSortResponse"];
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
            };
        };
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
        ErrorBody: {
            code: string;
        };
        HealthCheckResponse: {
            status: string;
        };
        IngestAssembleRequest: {
            children: components["schemas"]["IngestChild"][];
            envelope: components["schemas"]["IngestEnvelope"];
        };
        IngestChild: {
            sender: string;
            text: string;
        };
        IngestCipherRequest: {
            envelope: components["schemas"]["IngestEnvelope"];
            min_length: number;
        };
        IngestCipherResponse: {
            cipher?: string | null;
        };
        IngestEnvelope: {
            account: string;
            conversation_id: string;
            key: string;
            lookup?: string | null;
            occurred_at_ms: number;
            raw: string;
            received_at_ms: number;
            sender_id: string;
            sender_name: string;
            source: string;
            text: string;
        };
        IngestParsePageResponse: {
            envelopes: components["schemas"]["IngestEnvelope"][];
            next_cursor?: string | null;
            skipped: number;
        };
        IngestParseRequest: {
            account: string;
            body: string;
            cipher_min_length: number;
            received_at_ms: number;
            source: string;
            utc_offset_minutes: number;
        };
        IngestRevealRequest: {
            envelope: components["schemas"]["IngestEnvelope"];
            plaintext: string;
        };
        IngestWindowRequest: {
            from_ms: number;
            to_ms: number;
            utc_offset_minutes: number;
        };
        IngestWindowResponse: {
            end: string;
            start: string;
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
        ReviewApproveRequest: {
            keys: string[];
        };
        ReviewApproveResponse: {
            states: string[];
        };
        ReviewItem: {
            batch_id: number;
            conversation_id: string;
            flag: string;
            key: string;
            occurred_at_ms: number;
            reason: string;
            reviewer: string;
            sender: string;
            state: string;
            text: string;
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
        ReviewRejectRequest: {
            key: string;
            leak?: string | null;
            reason: string;
        };
        ReviewRulingResponse: {
            state: string;
        };
        TriageAttach: {
            conversation_id: string;
            last_ms: number;
            topic_id: string;
        };
        TriageBriefRequest: {
            followup: string;
            max_chars: number;
            resume?: components["schemas"]["TriageResume"] | null;
            rows: components["schemas"]["TriageRow"][];
            rules: components["schemas"]["TriageRule"][];
            template: string;
        };
        TriageBriefResponse: {
            prompt: string;
            start: components["schemas"]["TriageStart"];
            until_ms: number;
        };
        TriageCandidate: {
            key: string;
            text: string;
        };
        TriageChoice: {
            segment?: number | null;
            topic?: string | null;
            value: string;
        };
        TriageEntry: {
            conversation_id: string;
            hints: string[];
            key: string;
            occurred_at_ms: number;
            sender: string;
            text: string;
        };
        TriageLayout: {
            batch_id: number;
            lines: components["schemas"]["TriagePlaced"][];
            segments: components["schemas"]["TriageSegment"][];
        };
        TriageLineRequest: {
            cipher_min_length: number;
            max_chars: number;
            message: components["schemas"]["TriageSubject"];
            notify_conversation_id: string;
            rules: components["schemas"]["TriageRule"][];
            self_sender_id: string;
            unknown_sender: string;
        };
        TriageLineResponse: {
            sender: string;
            skip?: components["schemas"]["TriageSkip"] | null;
            text: string;
        };
        TriageNoticeRequest: {
            analysis: string;
            max_chars: number;
            probability: number;
            template: string;
            topic_id: string;
        };
        TriageNoticeResponse: {
            text: string;
        };
        TriageOpen: {
            label: string;
            topic: string;
        };
        TriagePackRequest: {
            attach: components["schemas"]["TriageAttach"][];
            batch_id: number;
            gap_ms: number;
            label_chars: number;
            lines: components["schemas"]["TriageEntry"][];
            max_chars: number;
            model: string;
            new_label: string;
            questions: components["schemas"]["TriageQuestions"];
            rules: components["schemas"]["TriageRule"][];
            topics: components["schemas"]["TriageOpen"][];
        };
        TriagePackResponse: {
            body: string;
            layout: components["schemas"]["TriageLayout"];
        };
        TriagePlaced: {
            key: string;
            occurred_at_ms: number;
            segment: number;
        };
        TriagePlacement: {
            key: string;
            probability?: number | null;
            topic: string;
        };
        TriageQuestion: {
            instructions: string;
            name: string;
        };
        TriageQuestions: {
            topic: components["schemas"]["TriageQuestion"];
            urgent: components["schemas"]["TriageQuestion"];
        };
        TriageRedactRequest: {
            rules: components["schemas"]["TriageRule"][];
            text: string;
        };
        TriageRedactResponse: {
            text: string;
        };
        TriageReplyRequest: {
            max_chars: number;
            stdout: string;
        };
        TriageReplyResponse: {
            text: string;
            thread_id: string;
        };
        TriageResume: {
            until: number;
        };
        TriageRow: {
            occurred_at_ms: number;
            sender: string;
            text: string;
        };
        TriageRule: {
            pattern: string;
            placeholder: string;
        };
        TriageScreenRequest: {
            instructions: string;
            lines: components["schemas"]["TriageCandidate"][];
            model: string;
            topics: components["schemas"]["TriageOpen"][];
        };
        TriageScreenResponse: {
            body: string;
        };
        TriageScreened: {
            flag: string;
            hints: string[];
            key: string;
            text: string;
        };
        TriageScreenedRequest: {
            lines: components["schemas"]["TriageCandidate"][];
            min_chars: number;
            placeholder: string;
            response: string;
            topics: components["schemas"]["TriageOpen"][];
        };
        TriageScreenedResponse: {
            lines: components["schemas"]["TriageScreened"][];
        };
        TriageSegment: {
            attach?: string | null;
            choices: components["schemas"]["TriageChoice"][];
            label: string;
            question?: number | null;
        };
        TriageSkip: "empty" | "notice" | "own" | "sealed";
        TriageSortRequest: {
            layout: components["schemas"]["TriageLayout"];
            min_confidence: number;
            response: string;
            threshold: number;
        };
        TriageSortResponse: {
            placements: components["schemas"]["TriagePlacement"][];
            topics: components["schemas"]["TriageTouched"][];
        };
        TriageStart: "fresh" | "resume";
        TriageSubject: {
            conversation_id: string;
            sender_id: string;
            sender_name: string;
            text: string;
        };
        TriageTouched: {
            label: string;
            last_ms: number;
            probability?: number | null;
            topic: string;
            urgent: components["schemas"]["TriageVerdict"];
        };
        TriageVerdict: "normal" | "urgent";
    };
    responses: never;
    parameters: never;
    requestBodies: never;
    headers: never;
    pathItems: never;
}
export type $defs = Record<string, never>;
export type operations = Record<string, never>;
