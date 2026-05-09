# API Documentation

Base URLs:
- Invoice API: `http://localhost:8080`
- Mock PSP: `http://localhost:8081`

## Authentication
Protected Invoice API endpoints require:
- Header: `Authorization: Bearer <api_key>`
- Scope: all endpoints except `GET /health`

Seed test key (local):
- `dodo_test_live_key_1234567890`

---

## Error Format
All Invoice API errors use this shape:

```json
{
  "error": {
    "code": "string_code",
    "message": "human readable message"
  }
}
```

Common HTTP statuses:
- `400` bad request / validation
- `401` unauthorized
- `404` not found
- `409` conflict (state transition/idempotency)
- `500` internal error

---

## Invoice API Endpoints

### `GET /health`
Public health check.

Response `200`:
```text
ok
```

### `POST /customers`
Create customer.
Auth required.

Request body:
```json
{
  "name": "Alice",
  "email": "alice@example.com"
}
```

Response `200`:
```json
{
  "id": "uuid",
  "name": "Alice",
  "email": "alice@example.com",
  "created_at": "2026-05-09T18:00:00Z"
}
```

### `GET /customers`
List customers for authenticated business.
Auth required.

Response `200`:
```json
[
  {
    "id": "uuid",
    "name": "Alice",
    "email": "alice@example.com",
    "created_at": "2026-05-09T18:00:00Z"
  }
]
```

### `GET /customers/:id`
Get customer by id (business scoped).
Auth required.

Response `200`:
```json
{
  "id": "uuid",
  "name": "Alice",
  "email": "alice@example.com",
  "created_at": "2026-05-09T18:00:00Z"
}
```

### `POST /invoices`
Create invoice. Server computes `total_amount_cents` from line items.
Auth required.

Request body:
```json
{
  "customer_id": "uuid",
  "due_date": "2026-06-10",
  "line_items": [
    {
      "description": "Item A",
      "quantity": 2,
      "unit_amount_cents": 500
    }
  ]
}
```

Response `200`:
```json
{
  "id": "uuid",
  "customer_id": "uuid",
  "state": "open",
  "total_amount_cents": 1000,
  "due_date": "2026-06-10",
  "line_items": [
    {
      "description": "Item A",
      "quantity": 2,
      "unit_amount_cents": 500
    }
  ],
  "created_at": "2026-05-09T18:00:00Z",
  "updated_at": "2026-05-09T18:00:00Z"
}
```

### `GET /invoices`
List invoices for authenticated business.
Auth required.

Query params:
- `state` (optional): one of `draft`, `open`, `processing`, `paid`, `void`, `uncollectible`

Response `200`:
```json
[
  {
    "id": "uuid",
    "customer_id": "uuid",
    "state": "paid",
    "total_amount_cents": 1000,
    "due_date": "2026-06-10",
    "created_at": "2026-05-09T18:00:00Z",
    "updated_at": "2026-05-09T18:00:00Z"
  }
]
```

### `GET /invoices/:id`
Get invoice by id, including line items.
Auth required.

Response `200`:
```json
{
  "id": "uuid",
  "customer_id": "uuid",
  "state": "open",
  "total_amount_cents": 1000,
  "due_date": "2026-06-10",
  "line_items": [
    {
      "description": "Item A",
      "quantity": 2,
      "unit_amount_cents": 500
    }
  ],
  "created_at": "2026-05-09T18:00:00Z",
  "updated_at": "2026-05-09T18:00:00Z"
}
```

### `POST /invoices/:id/pay`
Attempt payment for invoice.
Auth required.

Headers:
- `Idempotency-Key: <unique_key>` (required)

Request body:
```json
{
  "card_token": "tok_success"
}
```

Response `200`:
```json
{
  "invoice_id": "uuid",
  "payment_attempt_id": "uuid",
  "status": "succeeded",
  "message": "payment processed by PSP",
  "idempotent_replay": false,
  "failure_code": null,
  "psp_ref": "uuid"
}
```

Failure response example (`tok_timeout`):
```json
{
  "invoice_id": "uuid",
  "payment_attempt_id": "uuid",
  "status": "failed",
  "message": "PSP timeout handled safely",
  "idempotent_replay": false,
  "failure_code": "psp_timeout",
  "psp_ref": null
}
```

Idempotency behavior:
- Same `Idempotency-Key` + same request payload (`invoice_id`, `card_token`) returns same payment attempt result with `idempotent_replay: true`.
- Same `Idempotency-Key` + different payload returns `409` conflict (`idempotency_conflict`).

Replay response example:
```json
{
  "invoice_id": "uuid",
  "payment_attempt_id": "uuid",
  "status": "succeeded",
  "message": "idempotent replay; existing payment attempt returned",
  "idempotent_replay": true,
  "failure_code": null,
  "psp_ref": "uuid"
}
```

### `POST /webhook-endpoints`
Register webhook endpoint for authenticated business.
Auth required.

Request body:
```json
{
  "url": "https://example.com/webhooks/invoices"
}
```

Response `200`:
```json
{
  "id": "uuid",
  "url": "https://example.com/webhooks/invoices",
  "is_active": true,
  "created_at": "2026-05-09T18:00:00Z"
}
```

### `GET /webhook-endpoints`
List webhook endpoints for authenticated business.
Auth required.

Response `200`:
```json
[
  {
    "id": "uuid",
    "url": "https://example.com/webhooks/invoices",
    "is_active": true,
    "created_at": "2026-05-09T18:00:00Z"
  }
]
```

---

## Webhook Delivery
Events currently emitted:
- `invoice.created`
- `invoice.paid`
- `invoice.payment_failed`

Event payload shapes:

`invoice.created`
```json
{
  "invoice_id": "uuid",
  "status": "open"
}
```

`invoice.paid` and `invoice.payment_failed`
```json
{
  "invoice_id": "uuid",
  "payment_attempt_id": "uuid",
  "status": "succeeded|failed",
  "failure_code": "string|null",
  "psp_ref": "string|null"
}
```

Delivery characteristics:
- Asynchronous background worker (does not block API response)
- Signed with HMAC-SHA256 using endpoint `signing_secret`
- Signature input: `<timestamp>.<event_id>.<payload_json>`
- Headers sent:
  - `X-Dodo-Event-Type`
  - `X-Dodo-Event-Id`
  - `X-Dodo-Timestamp`
  - `X-Dodo-Signature`

Retry/backoff policy:
- Attempt 1 retry after `5s`
- Attempt 2 retry after `30s`
- Attempt 3 retry after `120s`
- Attempt 4 retry after `600s`
- Attempt 5 -> mark delivery `exhausted`

---

## Mock PSP Endpoints

### `GET /health`
Response:
```text
ok
```

### `POST /charges`
Request body:
```json
{
  "card_token": "tok_success",
  "amount_cents": 1000
}
```

Token behavior:
- `tok_success`: ~100ms, returns succeeded + `psp_ref`
- `tok_insufficient_funds`: ~100ms, returns failed + `insufficient_funds`
- `tok_card_declined`: ~100ms, returns failed + `card_declined`
- `tok_timeout`: sleeps ~30s then success
- `tok_network_error`: returns HTTP `500`
